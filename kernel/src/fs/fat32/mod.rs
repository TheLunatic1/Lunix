//! FAT32 Filesystem Implementation for Lunix VFS

pub mod bpb;
pub mod cluster;
pub mod dir;
pub mod file;

use alloc::boxed::Box;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use bpb::Fat32Layout;
use cluster::ClusterManager;
use dir::{parse_directory, ParsedFatEntry};
use file::Fat32File;
use crate::fs::block::BlockDevice;
use crate::fs::file::{DirectoryEntry, FileHandle};
use crate::fs::inode::{INode, INodeType};
use crate::fs::path::Path;
use crate::fs::vfs::{FileSystem, VfsError};
use spin::Mutex;

pub struct Fat32FileSystem {
    device: Arc<dyn BlockDevice>,
    layout: Fat32Layout,
    cluster_mgr: Arc<ClusterManager>,
    lock: Mutex<()>,
}

impl Fat32FileSystem {
    pub fn new(device: Arc<dyn BlockDevice>) -> Option<Self> {
        let mut sector0 = [0u8; 512];
        if device.read_blocks(0, 1, &mut sector0).is_err() {
            return None;
        }

        // 1. Check if disk has a GPT (GUID Partition Table) header at Sector 1
        let mut sector1 = [0u8; 512];
        if device.read_blocks(1, 1, &mut sector1).is_ok() && &sector1[0..8] == b"EFI PART" {
            let part_entry_lba = u64::from_le_bytes([
                sector1[72], sector1[73], sector1[74], sector1[75],
                sector1[76], sector1[77], sector1[78], sector1[79],
            ]);
            let num_entries = u32::from_le_bytes([
                sector1[80], sector1[81], sector1[82], sector1[83],
            ]);

            if part_entry_lba > 0 && num_entries > 0 {
                let mut part_table_sector = [0u8; 512];
                if device.read_blocks(part_entry_lba, 1, &mut part_table_sector).is_ok() {
                    for entry_idx in 0..core::cmp::min(4, num_entries as usize) {
                        let offset = entry_idx * 128;
                        let start_lba = u64::from_le_bytes([
                            part_table_sector[offset + 32], part_table_sector[offset + 33],
                            part_table_sector[offset + 34], part_table_sector[offset + 35],
                            part_table_sector[offset + 36], part_table_sector[offset + 37],
                            part_table_sector[offset + 38], part_table_sector[offset + 39],
                        ]);

                        if start_lba > 0 {
                            let mut part_sector = [0u8; 512];
                            if device.read_blocks(start_lba, 1, &mut part_sector).is_ok() {
                                if let Some(layout) = Fat32Layout::parse(&part_sector, start_lba) {
                                    let cluster_mgr = Arc::new(ClusterManager::new(device.clone(), layout));
                                    lunix_serial_println!("  [FAT32] Initialized GPT Partition {} (LBA {}) on device '{}' (Sector: {} B, Cluster: {} Sec, Root Cluster: {})",
                                        entry_idx + 1, start_lba, device.name(), layout.bytes_per_sector, layout.sectors_per_cluster, layout.root_cluster
                                    );
                                    return Some(Self {
                                        device,
                                        layout,
                                        cluster_mgr,
                                        lock: Mutex::new(()),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        // 2. If Sector 0 is an MBR, check MBR partition table (offsets 0x1BE, 0x1CE, 0x1DE, 0x1EE)
        if sector0[510] == 0x55 && sector0[511] == 0xAA {
            for i in 0..4 {
                let entry_offset = 446 + i * 16;
                let part_type = sector0[entry_offset + 4];
                let start_lba = u32::from_le_bytes([
                    sector0[entry_offset + 8],
                    sector0[entry_offset + 9],
                    sector0[entry_offset + 10],
                    sector0[entry_offset + 11],
                ]) as u64;

                if start_lba > 0 && (part_type == 0xEF || part_type == 0x0C || part_type == 0x0B || part_type == 0x07 || part_type == 0x83 || part_type == 0x06) {
                    let mut part_sector = [0u8; 512];
                    if device.read_blocks(start_lba, 1, &mut part_sector).is_ok() {
                        if let Some(layout) = Fat32Layout::parse(&part_sector, start_lba) {
                            let cluster_mgr = Arc::new(ClusterManager::new(device.clone(), layout));
                            lunix_serial_println!("  [FAT32] Initialized MBR Partition {} (LBA {}) on device '{}' (Sector: {} B, Cluster: {} Sec, Root Cluster: {})",
                                i + 1, start_lba, device.name(), layout.bytes_per_sector, layout.sectors_per_cluster, layout.root_cluster
                            );
                            return Some(Self {
                                device,
                                layout,
                                cluster_mgr,
                                lock: Mutex::new(()),
                            });
                        }
                    }
                }
            }
        }

        // 3. Fallback: try parsing Sector 0 directly as an unpartitioned FAT32 BPB
        if let Some(layout) = Fat32Layout::parse(&sector0, 0) {
            let cluster_mgr = Arc::new(ClusterManager::new(device.clone(), layout));
            lunix_serial_println!("  [FAT32] Initialized unpartitioned on device '{}' (Sector: {} B, Cluster: {} Sec, Root Cluster: {})",
                device.name(), layout.bytes_per_sector, layout.sectors_per_cluster, layout.root_cluster
            );
            return Some(Self {
                device,
                layout,
                cluster_mgr,
                lock: Mutex::new(()),
            });
        }

        None
    }

    fn traverse_path(&self, path_str: &str) -> Result<ParsedFatEntry, VfsError> {
        let path = Path::new(path_str);
        if path.is_root() {
            return Ok(ParsedFatEntry {
                name: String::from("/"),
                is_dir: true,
                start_cluster: self.layout.root_cluster,
                file_size: 0,
            });
        }

        let mut current_cluster = self.layout.root_cluster;
        let mut current_entry = ParsedFatEntry {
            name: String::from("/"),
            is_dir: true,
            start_cluster: self.layout.root_cluster,
            file_size: 0,
        };

        for component in path.components() {
            if !current_entry.is_dir {
                return Err(VfsError::NotADirectory);
            }

            let dir_data = self
                .cluster_mgr
                .read_all_clusters(current_cluster)
                .map_err(|_| VfsError::DeviceError)?;
            let entries = parse_directory(&dir_data);

            let mut found = false;
            for entry in entries {
                if entry.name.eq_ignore_ascii_case(component) {
                    current_cluster = entry.start_cluster;
                    current_entry = entry;
                    found = true;
                    break;
                }
            }

            if !found {
                return Err(VfsError::NotFound);
            }
        }

        Ok(current_entry)
    }
}

impl FileSystem for Fat32FileSystem {
    fn open(&self, path: &str) -> Result<Box<dyn FileHandle>, VfsError> {
        let _guard = self.lock.lock();
        let entry = self.traverse_path(path)?;

        if entry.is_dir {
            return Err(VfsError::NotAFile);
        }

        Ok(Box::new(Fat32File::new(
            self.cluster_mgr.clone(),
            entry.start_cluster,
            entry.file_size as u64,
        )))
    }

    fn read_dir(&self, path: &str) -> Result<Vec<DirectoryEntry>, VfsError> {
        let _guard = self.lock.lock();
        let entry = self.traverse_path(path)?;

        if !entry.is_dir {
            return Err(VfsError::NotADirectory);
        }

        let dir_data = self
            .cluster_mgr
            .read_all_clusters(entry.start_cluster)
            .map_err(|_| VfsError::DeviceError)?;
        let fat_entries = parse_directory(&dir_data);

        let mut vfs_entries = Vec::new();
        for e in fat_entries {
            vfs_entries.push(DirectoryEntry {
                name: e.name,
                node_type: if e.is_dir {
                    INodeType::Directory
                } else {
                    INodeType::File
                },
                size: e.file_size as u64,
            });
        }

        Ok(vfs_entries)
    }

    fn stat(&self, path: &str) -> Result<INode, VfsError> {
        let _guard = self.lock.lock();
        let entry = self.traverse_path(path)?;

        Ok(INode {
            id: entry.start_cluster as u64,
            size: entry.file_size as u64,
            node_type: if entry.is_dir {
                INodeType::Directory
            } else {
                INodeType::File
            },
            permissions: 0o755,
            name: entry.name,
        })
    }
}
