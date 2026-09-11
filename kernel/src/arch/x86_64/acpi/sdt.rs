//! ACPI System Description Table (SDT) Headers and Parsing

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct SdtHeader {
    pub signature: [u8; 4],
    pub length: u32,
    pub revision: u8,
    pub checksum: u8,
    pub oem_id: [u8; 6],
    pub oem_table_id: [u8; 8],
    pub oem_revision: u32,
    pub creator_id: u32,
    pub creator_revision: u32,
}

impl SdtHeader {
    /// Validates the checksum of the table.
    pub fn is_valid(&self) -> bool {
        let ptr = self as *const _ as *const u8;
        let mut sum: u8 = 0;
        for i in 0..self.length as usize {
            unsafe {
                sum = sum.wrapping_add(*ptr.add(i));
            }
        }
        sum == 0
    }

    pub fn signature_str(&self) -> &str {
        core::str::from_utf8(&self.signature).unwrap_or("????")
    }

    pub fn oem_id_str(&self) -> &str {
        core::str::from_utf8(&self.oem_id).unwrap_or("??????")
    }
}
