//! Lunix Virtual Filesystem (VFS) and Storage Subsystems

pub mod block;
pub mod devfs;
pub mod fat32;
pub mod file;
pub mod inode;
pub mod path;
pub mod procfs;
pub mod vfs;

use crate::lunix_println;
use alloc::sync::Arc;
use block::get_block_device;
use devfs::DevFs;
use fat32::Fat32FileSystem;
use procfs::ProcFs;
use vfs::mount;

pub fn init() {
    lunix_serial_println!("[fs] Initializing VFS and Mounting Root Filesystem...");

    // Mount virtual pseudo-filesystems
    mount("/dev", Arc::new(DevFs::new()));
    mount("/proc", Arc::new(ProcFs::new()));

    // Try mounting root filesystem from /dev/sda
    if let Some(dev) = get_block_device("sda") {
        if let Some(fat_fs) = Fat32FileSystem::new(dev) {
            mount("/", Arc::new(fat_fs));
            lunix_println!("[+] Root filesystem (FAT32) mounted successfully from /dev/sda.");
            lunix_println!("[+] Virtual pseudo-filesystems (/dev, /proc) mounted.");
        } else {
            lunix_serial_println!("[-] /dev/sda is not a valid FAT32 filesystem.");
        }
    } else {
        lunix_serial_println!("[-] No block device 'sda' found for root mount.");
    }
}
