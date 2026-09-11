//! ACPI Root System Description Pointer (RSDP)

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct RsdpDescriptorV1 {
    pub signature: [u8; 8],
    pub checksum: u8,
    pub oem_id: [u8; 6],
    pub revision: u8,
    pub rsdt_address: u32,
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct RsdpDescriptorV2 {
    pub v1: RsdpDescriptorV1,
    pub length: u32,
    pub xsdt_address: u64,
    pub extended_checksum: u8,
    pub reserved: [u8; 3],
}

impl RsdpDescriptorV1 {
    pub fn is_valid(&self) -> bool {
        if &self.signature != b"RSD PTR " {
            return false;
        }
        let ptr = self as *const _ as *const u8;
        let mut sum: u8 = 0;
        for i in 0..core::mem::size_of::<RsdpDescriptorV1>() {
            unsafe {
                sum = sum.wrapping_add(*ptr.add(i));
            }
        }
        sum == 0
    }
}

impl RsdpDescriptorV2 {
    pub fn is_valid(&self) -> bool {
        if !self.v1.is_valid() {
            return false;
        }
        if self.v1.revision < 2 {
            return true;
        }
        let ptr = self as *const _ as *const u8;
        let mut sum: u8 = 0;
        for i in 0..self.length as usize {
            unsafe {
                sum = sum.wrapping_add(*ptr.add(i));
            }
        }
        sum == 0
    }
}

/// Locates RSDP by checking BootInfo pointer or scanning BIOS / EBDA memory areas
pub unsafe fn find_rsdp(hint_addr: Option<u64>) -> Option<u64> {
    if let Some(addr) = hint_addr {
        let rsdp_v1 = &*(addr as *const RsdpDescriptorV1);
        if rsdp_v1.is_valid() {
            return Some(addr);
        }
    }

    // 1. Scan EBDA (Extended BIOS Data Area)
    let ebda_ptr = *(0x40E as *const u16) as u64 * 16;
    if ebda_ptr >= 0x80000 && ebda_ptr <= 0x9FFFF {
        if let Some(found) = scan_memory(ebda_ptr, 1024) {
            return Some(found);
        }
    }

    // 2. Scan Main BIOS memory area: 0xE0000..0x100000
    scan_memory(0xE0000, 0x20000)
}

unsafe fn scan_memory(base: u64, length: usize) -> Option<u64> {
    let mut curr = base;
    let end = base + length as u64;

    while curr < end {
        let sig = *(curr as *const [u8; 8]);
        if &sig == b"RSD PTR " {
            let rsdp_v1 = &*(curr as *const RsdpDescriptorV1);
            if rsdp_v1.is_valid() {
                return Some(curr);
            }
        }
        curr += 16; // 16-byte boundary aligned
    }

    None
}
