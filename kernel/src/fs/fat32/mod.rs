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
        let mut boot_sector = [0u8; 512];
        if device.read_blocks(0, 1, &mut boot_sector).is_err() {
            return None;
        }

        let layout = Fat32Layout::parse(&boot_sector)?;
        let cluster_mgr = Arc::new(ClusterManager::new(device.clone(), layout));

        lunix_serial_println!("  [FAT32] Initialized on device '{}' (Sector: {} B, Cluster: {} Sec, Root Cluster: {})",
            device.name(), layout.bytes_per_sector, layout.sectors_per_cluster, layout.root_cluster
        );

        Some(Self {
            device,
            layout,
            cluster_mgr,
            lock: Mutex::new(()),
        })
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
