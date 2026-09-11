//! Storage Drivers Subsystem

pub mod ata;

pub fn init() {
    ata::init();
}
