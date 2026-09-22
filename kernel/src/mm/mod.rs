pub mod cow;
pub mod heap;
pub mod pmm;
pub mod vma;
pub mod vmm;

use lunix_common::BootInfo;


pub fn init(boot_info: &'static BootInfo) {
    use crate::arch::x86_64::serial::{write_dec, write_hex, write_str};

    write_str("[mm::init] BootInfo addr: ");
    write_hex(boot_info as *const BootInfo as u64);
    write_str(", magic: ");
    write_hex(boot_info.magic);
    write_str(", regions: ");
    write_dec(boot_info.memory_map.entry_count);
    write_str("\n");

    write_str("[mm::init] calling pmm::init...\n");
    pmm::init(boot_info);
    write_str("[mm::init] pmm::init done\n");

    write_str("[mm::init] calling vmm::init...\n");
    vmm::init(0);
    write_str("[mm::init] vmm::init done\n");

    write_str("[mm::init] calling heap::init...\n");
    heap::init();
    write_str("[mm::init] heap::init done\n");

    write_str("[mm] Memory Subsystem ready.\n");
}
