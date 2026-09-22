//! Disk image assembly: GPT with a small FAT32 EFI System Partition (bootloader + kernel)
//! and an ext2 root filesystem built from real Arch packages.

use crate::{ext2, rootfs};
use anyhow::{bail, Context, Result};
use std::fs::{self, File, OpenOptions};
use std::io::{Cursor, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

pub const DISK_SIZE: u64 = 2048 * 1024 * 1024;
const SECTOR: u64 = 512;
const ESP_START_LBA: u64 = 2048;
const ESP_SIZE: u64 = 128 * 1024 * 1024;
const ESP_SECTORS: u64 = ESP_SIZE / SECTOR;
const ROOT_START_LBA: u64 = ESP_START_LBA + ESP_SECTORS;

fn total_sectors() -> u64 {
    DISK_SIZE / SECTOR
}

fn last_usable_lba() -> u64 {
    total_sectors() - 34
}

/// Size in bytes of the root partition (whole 4 KiB blocks).
pub fn root_partition_bytes() -> u64 {
    let sectors = last_usable_lba() - ROOT_START_LBA + 1;
    (sectors * SECTOR) / 4096 * 4096
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            let mask = -((crc & 1) as i32) as u32;
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

fn utf16_name(dst: &mut [u8], name: &str) {
    for (i, ch) in name.encode_utf16().take(36).enumerate() {
        dst[i * 2..i * 2 + 2].copy_from_slice(&ch.to_le_bytes());
    }
}

/// Build the FAT32 EFI System Partition contents (kernel and bootloader only).
fn build_esp(bootloader: &[u8], kernel: &[u8]) -> Result<Vec<u8>> {
    let mut buf = Cursor::new(vec![0u8; ESP_SIZE as usize]);
    fatfs::format_volume(
        &mut buf,
        fatfs::FormatVolumeOptions::new().fat_type(fatfs::FatType::Fat32).bytes_per_cluster(1024),
    )?;
    {
        let fs = fatfs::FileSystem::new(&mut buf, fatfs::FsOptions::new())?;
        let root = fs.root_dir();
        let efi = root.create_dir("EFI")?;
        let boot = efi.create_dir("BOOT")?;
        boot.create_file("BOOTX64.EFI")?.write_all(bootloader)?;
        let lunix = root.create_dir("LUNIX")?;
        lunix.create_file("KERNEL.BIN")?.write_all(kernel)?;
        // OVMF's own default-boot-file discovery (auto-launching \EFI\BOOT\BOOTX64.EFI with no
        // NVRAM boot entry) has been observed to race and lose occasionally — more PCI/block
        // devices connected at boot (e.g. adding a VirtIO-Block controller) makes it drop to
        // the interactive UEFI Shell instead, even though the ESP and BOOTX64.EFI are both
        // right there and reachable by hand at that same shell prompt. A startup.nsh makes the
        // Shell's own (short, fixed) auto-run path launch the bootloader deterministically
        // instead of relying on the boot manager's racier default-boot logic. The ESP can be
        // whichever fsN the shell mapped this boot (fs0 in the common case, but not always
        // once more disks are attached), so try a few.
        root.create_file("startup.nsh")?.write_all(
            b"@echo -off\r\nfs0:\\EFI\\BOOT\\BOOTX64.EFI\r\nfs1:\\EFI\\BOOT\\BOOTX64.EFI\r\nfs2:\\EFI\\BOOT\\BOOTX64.EFI\r\n",
        )?;
    }
    let mut bytes = buf.into_inner();
    // BPB_HiddSec = partition start LBA (primary and backup boot sector).
    let hidden = (ESP_START_LBA as u32).to_le_bytes();
    bytes[28..32].copy_from_slice(&hidden);
    if bytes.len() >= 6 * 512 + 32 {
        bytes[6 * 512 + 28..6 * 512 + 32].copy_from_slice(&hidden);
    }
    Ok(bytes)
}

fn gpt_entries() -> Vec<u8> {
    let mut e = vec![0u8; 128 * 128];
    // Entry 0: EFI System Partition
    e[0..16].copy_from_slice(&[0x28, 0x73, 0x2A, 0xC1, 0x1F, 0xF8, 0xD2, 0x11, 0xBA, 0x4B, 0x00, 0xA0, 0xC9, 0x3E, 0xC9, 0x3B]);
    e[16..32].copy_from_slice(&[0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x00]);
    e[32..40].copy_from_slice(&ESP_START_LBA.to_le_bytes());
    e[40..48].copy_from_slice(&(ROOT_START_LBA - 1).to_le_bytes());
    utf16_name(&mut e[56..128], "EFI System Partition");
    // Entry 1: Linux filesystem (0FC63DAF-8483-4772-8E79-3D69D8477DE4) - the ext2 root
    let o = 128;
    e[o..o + 16].copy_from_slice(&[0xAF, 0x3D, 0xC6, 0x0F, 0x83, 0x84, 0x72, 0x47, 0x8E, 0x79, 0x3D, 0x69, 0xD8, 0x47, 0x7D, 0xE4]);
    e[o + 16..o + 32].copy_from_slice(&[0x21, 0x32, 0x43, 0x54, 0x65, 0x76, 0x87, 0x98, 0xA9, 0xBA, 0xCB, 0xDC, 0xED, 0xFE, 0x0F, 0x10]);
    e[o + 32..o + 40].copy_from_slice(&ROOT_START_LBA.to_le_bytes());
    e[o + 40..o + 48].copy_from_slice(&last_usable_lba().to_le_bytes());
    utf16_name(&mut e[o + 56..o + 128], "Lunix Root");
    e
}

fn gpt_header(my_lba: u64, alt_lba: u64, entries_lba: u64, entries_crc: u32, disk_guid: &[u8; 16]) -> Vec<u8> {
    let mut h = vec![0u8; 92];
    h[0..8].copy_from_slice(b"EFI PART");
    h[8..12].copy_from_slice(&0x0001_0000u32.to_le_bytes());
    h[12..16].copy_from_slice(&92u32.to_le_bytes());
    h[24..32].copy_from_slice(&my_lba.to_le_bytes());
    h[32..40].copy_from_slice(&alt_lba.to_le_bytes());
    h[40..48].copy_from_slice(&ESP_START_LBA.to_le_bytes());
    h[48..56].copy_from_slice(&last_usable_lba().to_le_bytes());
    h[56..72].copy_from_slice(disk_guid);
    h[72..80].copy_from_slice(&entries_lba.to_le_bytes());
    h[80..84].copy_from_slice(&128u32.to_le_bytes());
    h[84..88].copy_from_slice(&128u32.to_le_bytes());
    h[88..92].copy_from_slice(&entries_crc.to_le_bytes());
    let c = crc32(&h);
    h[16..20].copy_from_slice(&c.to_le_bytes());
    h
}

/// Write the whole disk: protective MBR, GPT (primary + backup), ESP, and root partition.
pub fn create_disk_image(img_path: &Path, bootloader_bin: &Path, kernel_bin: &Path, rootfs_img: &Path) -> Result<()> {
    let bootloader = fs::read(bootloader_bin).with_context(|| format!("read {}", bootloader_bin.display()))?;
    let kernel = fs::read(kernel_bin).with_context(|| format!("read {}", kernel_bin.display()))?;
    let esp = build_esp(&bootloader, &kernel)?;

    let mut img = OpenOptions::new().read(true).write(true).create(true).truncate(true).open(img_path)?;
    img.set_len(DISK_SIZE)?;

    // Protective MBR
    let mut mbr = vec![0u8; 512];
    mbr[446 + 2] = 0x02;
    mbr[446 + 4] = 0xEE;
    mbr[446 + 5] = 0xFF;
    mbr[446 + 6] = 0xFF;
    mbr[446 + 7] = 0xFF;
    mbr[446 + 8..446 + 12].copy_from_slice(&1u32.to_le_bytes());
    mbr[446 + 12..446 + 16].copy_from_slice(&((total_sectors() - 1) as u32).to_le_bytes());
    mbr[510] = 0x55;
    mbr[511] = 0xAA;
    img.write_all(&mbr)?;

    let entries = gpt_entries();
    let entries_crc = crc32(&entries);
    let disk_guid: [u8; 16] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];

    let primary = gpt_header(1, total_sectors() - 1, 2, entries_crc, &disk_guid);
    img.seek(SeekFrom::Start(SECTOR))?;
    img.write_all(&primary)?;
    img.seek(SeekFrom::Start(2 * SECTOR))?;
    img.write_all(&entries)?;

    let backup_entries_lba = total_sectors() - 33;
    let backup = gpt_header(total_sectors() - 1, 1, backup_entries_lba, entries_crc, &disk_guid);
    img.seek(SeekFrom::Start(backup_entries_lba * SECTOR))?;
    img.write_all(&entries)?;
    img.seek(SeekFrom::Start((total_sectors() - 1) * SECTOR))?;
    img.write_all(&backup)?;

    // ESP
    img.seek(SeekFrom::Start(ESP_START_LBA * SECTOR))?;
    img.write_all(&esp)?;

    // Root partition: copy the cached ext2 image.
    let mut root = File::open(rootfs_img).with_context(|| format!("open {}", rootfs_img.display()))?;
    img.seek(SeekFrom::Start(ROOT_START_LBA * SECTOR))?;
    let mut buf = vec![0u8; 8 * 1024 * 1024];
    loop {
        let n = root.read(&mut buf)?;
        if n == 0 {
            break;
        }
        img.write_all(&buf[..n])?;
    }
    img.flush()?;
    Ok(())
}

/// Build `target/rootfs.ext2` from the packages listed in `target/arch_rootfs_manifest.txt`
/// (written by `tools/resolve_arch_deps.py`). Cached: rebuilt only when the manifest, the
/// package archives or `LUNIX_REBUILD_ROOTFS` say so.
pub fn ensure_rootfs(root: &Path, force: bool) -> Result<PathBuf> {
    let target = root.join("target");
    let manifest = target.join("arch_rootfs_manifest.txt");
    let out_path = target.join("rootfs.ext2");
    if !manifest.is_file() {
        bail!(
            "{} not found. Fetch Arch packages first, e.g.:\n  python tools/resolve_arch_deps.py --fetch bash coreutils pacman filesystem",
            manifest.display()
        );
    }
    let stamp = |p: &Path| p.metadata().and_then(|m| m.modified()).ok();
    let force = force || std::env::var_os("LUNIX_REBUILD_ROOTFS").is_some();
    // Also rebuild when the builder itself changed (it decides what goes into the image).
    let src_dir = root.join("xtask").join("src");
    let newest_src = ["disk.rs", "ext2.rs", "rootfs.rs"].iter().filter_map(|f| stamp(&src_dir.join(f))).max();
    let up_to_date = stamp(&out_path) >= stamp(&manifest) && stamp(&out_path) >= newest_src;
    if !force && out_path.is_file() && up_to_date {
        println!("[+] Reusing cached root filesystem ({})", out_path.display());
        return Ok(out_path);
    }

    println!("[+] Building ext2 root filesystem from Arch packages...");
    let mut tree = rootfs::Tree::new();
    let mut infos = Vec::new();
    for line in fs::read_to_string(&manifest)?.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let tar_path = root.join(line);
        let info = tree
            .add_package(&tar_path)
            .with_context(|| format!("reading package {}", tar_path.display()))?;
        infos.push(info);
    }
    println!("    -> {} packages, {} filesystem nodes", infos.len(), tree.nodes.len());

    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs();
    tree.write_pacman_db(&infos, now);

    // System configuration that a fresh Arch install gets from its administrator, not from packages.
    tree.add_file("etc/hostname", b"arch\n".to_vec(), 0o644);
    // Xorg on the Linux framebuffer: no GPU driver, udev or evdev auto-detection here.
    tree.add_file("etc/X11/xorg.conf.d/10-lunix.conf", XORG_CONF.as_bytes().to_vec(), 0o644);
    tree.add_file("root/.xinitrc", XINITRC.as_bytes().to_vec(), 0o755);
    for dir in ["proc", "sys", "dev", "run", "tmp", "mnt", "boot", "srv", "opt"] {
        if tree.lookup(dir).is_none() {
            tree.insert(
                dir,
                rootfs::Node { kind: rootfs::Kind::Dir(Default::default()), mode: 0o755, uid: 0, gid: 0, mtime: 0 },
            );
        }
    }
    if let Some(i) = tree.lookup("tmp") {
        tree.nodes[i].mode = 0o1777;
    }

    let size = root_partition_bytes();
    let mut out = OpenOptions::new().read(true).write(true).create(true).truncate(true).open(&out_path)?;
    out.set_len(size)?;
    ext2::write_ext2(&mut out, 0, size, &tree, "lunix-root")?;
    println!("    -> Wrote {} ({} MiB)", out_path.display(), size / (1024 * 1024));
    Ok(out_path)
}

const XORG_CONF: &str = r#"Section "ServerFlags"
    Option "AutoAddDevices" "false"
    Option "AutoEnableDevices" "false"
    Option "AllowEmptyInput" "true"
    Option "DontVTSwitch" "true"
EndSection

Section "Device"
    Identifier "fb0"
    Driver "fbdev"
    Option "fbdev" "/dev/fb0"
EndSection

Section "Screen"
    Identifier "screen0"
    Device "fb0"
EndSection

Section "InputDevice"
    Identifier "kbd0"
    Driver "evdev"
    Option "Device" "/dev/input/event0"
EndSection

Section "InputDevice"
    Identifier "mouse0"
    Driver "evdev"
    Option "Device" "/dev/input/event1"
EndSection

Section "ServerLayout"
    Identifier "layout0"
    Screen 0 "screen0"
    InputDevice "kbd0" "CoreKeyboard"
    InputDevice "mouse0" "CorePointer"
EndSection
"#;

/// The user's X session (what an Arch user puts in ~/.xinitrc): a terminal, then the
/// window manager and its panel.
const XINITRC: &str = r#"#!/bin/sh
xsetroot -solid '#1793d1' &
xterm &
exec jwm
"#;
