//! FAT32 BIOS Parameter Block (BPB) Parser

#[derive(Debug, Clone, Copy)]
pub struct Fat32Layout {
    pub partition_start_lba: u64,
    pub bytes_per_sector: u32,
    pub sectors_per_cluster: u32,
    pub reserved_sectors: u32,
    pub num_fats: u32,
    pub fat_size: u32,
    pub root_cluster: u32,
    pub first_data_sector: u32,
    pub total_sectors: u32,
}

impl Fat32Layout {
    pub fn parse(buf: &[u8], partition_start_lba: u64) -> Option<Self> {
        if buf.len() < 512 {
            return None;
        }

        // Verify boot signature 0x55, 0xAA at end of boot sector
        if buf[510] != 0x55 || buf[511] != 0xAA {
            return None;
        }

        unsafe {
            let ptr = buf.as_ptr();

            let bytes_per_sector = core::ptr::read_unaligned(ptr.add(11) as *const u16) as u32;
            let sectors_per_cluster = *ptr.add(13) as u32;
            let reserved_sectors = core::ptr::read_unaligned(ptr.add(14) as *const u16) as u32;
            let num_fats = *ptr.add(16) as u32;

            let fat_size_16 = core::ptr::read_unaligned(ptr.add(22) as *const u16) as u32;
            let fat_size_32 = core::ptr::read_unaligned(ptr.add(36) as *const u32);

            let fat_size = if fat_size_16 != 0 {
                fat_size_16
            } else {
                fat_size_32
            };

            if bytes_per_sector == 0 || sectors_per_cluster == 0 || fat_size == 0 {
                return None;
            }

            let total_sectors_16 = core::ptr::read_unaligned(ptr.add(19) as *const u16) as u32;
            let total_sectors_32 = core::ptr::read_unaligned(ptr.add(32) as *const u32);

            let total_sectors = if total_sectors_16 != 0 {
                total_sectors_16
            } else {
                total_sectors_32
            };

            let root_cluster = core::ptr::read_unaligned(ptr.add(44) as *const u32);
            let first_data_sector = reserved_sectors + (num_fats * fat_size);

            Some(Self {
                partition_start_lba,
                bytes_per_sector,
                sectors_per_cluster,
                reserved_sectors,
                num_fats,
                fat_size,
                root_cluster: if root_cluster == 0 { 2 } else { root_cluster },
                first_data_sector,
                total_sectors,
            })
        }
    }

    pub fn cluster_to_lba(&self, cluster: u32) -> u64 {
        let base_data_lba = self.partition_start_lba + self.first_data_sector as u64;
        if cluster < 2 {
            base_data_lba
        } else {
            base_data_lba + ((cluster - 2) * self.sectors_per_cluster) as u64
        }
    }

    pub fn cluster_bytes(&self) -> usize {
        (self.bytes_per_sector * self.sectors_per_cluster) as usize
    }
}
