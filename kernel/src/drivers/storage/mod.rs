//! Storage Drivers Subsystem

pub mod ahci;
pub mod ata;
pub mod nvme;

pub fn init() {
    ata::init();
    ahci::init();
    nvme::init();
}

