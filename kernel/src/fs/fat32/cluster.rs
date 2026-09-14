//! FAT32 Cluster Chain Traversal

use super::bpb::Fat32Layout;
use crate::fs::block::BlockDevice;
use alloc::sync::Arc;
use alloc::vec::Vec;

pub const FAT32_EOF: u32 = 0x0FFF_FFF8;
pub const FAT32_BAD: u32 = 0x0FFF_FFF7;

pub struct ClusterManager {
    device: Arc<dyn BlockDevice>,
    layout: Fat32Layout,
}

impl ClusterManager {
    pub fn new(device: Arc<dyn BlockDevice>, layout: Fat32Layout) -> Self {
        Self { device, layout }
    }

    pub fn next_cluster(&self, current_cluster: u32) -> Option<u32> {
        if current_cluster < 2 || current_cluster >= FAT32_EOF {
            return None;
        }

        let fat_offset = current_cluster * 4;
        let fat_sector = self.layout.partition_start_lba
            + self.layout.reserved_sectors as u64
            + (fat_offset / self.layout.bytes_per_sector) as u64;
        let sector_offset = (fat_offset % self.layout.bytes_per_sector) as usize;

        let mut sector_buf = [0u8; 512];
        if self.device.read_blocks(fat_sector, 1, &mut sector_buf).is_err() {
            return None;
        }

        let raw_val = unsafe {
            core::ptr::read_unaligned(sector_buf.as_ptr().add(sector_offset) as *const u32)
        };

        let next = raw_val & 0x0FFF_FFFF;
        if next >= FAT32_EOF || next == 0 {
            None
        } else {
            Some(next)
        }
    }

    pub fn read_cluster(&self, cluster: u32, buf: &mut [u8]) -> Result<(), &'static str> {
        let lba = self.layout.cluster_to_lba(cluster);
        self.device.read_blocks(lba, self.layout.sectors_per_cluster as usize, buf)
    }

    pub fn read_all_clusters(&self, start_cluster: u32) -> Result<Vec<u8>, &'static str> {
        let mut data = Vec::new();
        let cluster_size = self.layout.cluster_bytes();
        let mut cluster_buf = alloc::vec![0u8; cluster_size];

        let mut curr = Some(start_cluster);
        while let Some(c) = curr {
            self.read_cluster(c, &mut cluster_buf)?;
            data.extend_from_slice(&cluster_buf);
            curr = self.next_cluster(c);
        }

        Ok(data)
    }
}
