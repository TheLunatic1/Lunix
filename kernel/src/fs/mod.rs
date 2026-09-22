//! Lunix Virtual Filesystem (VFS) and Storage Subsystems

pub mod block;
pub mod devfs;
pub mod ext2;
pub mod fat32;
pub mod file;
pub mod inode;
pub mod path;
pub mod procfs;
pub mod sysfs;
pub mod vfs;

use crate::lunix_println;
use alloc::sync::Arc;
use block::get_block_device;
use devfs::DevFs;
use fat32::Fat32FileSystem;
use procfs::ProcFs;
use sysfs::SysFs;
use vfs::mount;

pub fn init() {
    lunix_serial_println!("[fs] Initializing VFS and Mounting Root Filesystem...");

    // Mount virtual pseudo-filesystems
    mount("/dev", Arc::new(DevFs::new()));
    mount("/proc", Arc::new(ProcFs::new()));
    mount("/sys", Arc::new(SysFs::new()));

    // /dev/vda (paravirtualized VirtIO-Block) is DMA-backed and boots noticeably faster than
    // /dev/sda's per-sector PIO ATA path when both are present (measured ~4-6s vs ~7.5s to a
    // shell prompt with an otherwise identical image) — see docs/KERNEL_ROADMAP.md Phase 1.
    // It is NOT preferred as root by default yet: under the X/JWM/xterm desktop's sustained
    // concurrent read load (as opposed to boot's light, mostly-sequential reads) something
    // downstream still stalls indefinitely with vda as root. Three real bugs in the VirtIO-Block
    // driver were found and fixed chasing this (queue-size/ring-offset mismatch corrupting
    // reads, a DMA data buffer overflow past its single allocated frame for transfers over 4
    // KiB, a physical-frame leak on every request), and each was independently worth fixing,
    // but the stall persisted after all three, so at least one more cause remains unidentified.
    // Prefer sda until that's root-caused, so the (working, verified) GUI desktop path isn't
    // silently regressed; vda stays registered and usable (e.g. `mount`, `/dev/vda`) either way.
    let (root_dev_name, dev) = match get_block_device("sda") {
        Some(d) => ("sda", d),
        None => match get_block_device("vda") {
            Some(d) => ("vda", d),
            None => {
                lunix_serial_println!("[-] No block device ('sda'/'vda') found for root mount.");
                return;
            }
        },
    };

    // Root: the ext2 partition (real Linux filesystem: symlinks, case-sensitive, permissions).
    let mut root_mounted = false;
    if let Some(start) = ext2::find_ext2(&dev) {
        if let Some(fs) = ext2::Ext2FileSystem::new(dev.clone(), start) {
            mount("/", Arc::new(fs));
            lunix_println!("[+] Root filesystem (ext2) mounted from /dev/{}.", root_dev_name);
            root_mounted = true;
        }
    }
    // The EFI System Partition (FAT32) holds the bootloader and kernel.
    if let Some(fat_fs) = Fat32FileSystem::new(dev) {
        if root_mounted {
            mount("/boot", Arc::new(fat_fs));
            lunix_println!("[+] EFI System Partition (FAT32) mounted at /boot.");
        } else {
            mount("/", Arc::new(fat_fs));
            lunix_println!("[!] No ext2 root found: falling back to the FAT32 partition as root.");
            root_mounted = true;
        }
    }
    if !root_mounted {
        lunix_serial_println!("[-] No usable root filesystem on /dev/{}.", root_dev_name);
    }
    lunix_println!("[+] Virtual pseudo-filesystems (/dev, /proc, /sys) mounted.");
}
