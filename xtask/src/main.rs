mod disk;
mod ext2;
mod rootfs;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Parser)]
#[command(name = "xtask")]
#[command(about = "Lunix OS Build & Run Orchestrator", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Build the UEFI bootloader, kernel, and boot disk image (GPT: FAT32 ESP + ext2 root)
    Build {
        #[arg(long, default_value = "true")]
        release: bool,
    },
    /// Force a rebuild of the ext2 root filesystem (normally cached), then build the disk
    Rootfs {
        #[arg(long, default_value = "true")]
        release: bool,
    },
    /// Build and run Lunix inside QEMU with OVMF UEFI firmware
    Run {
        #[arg(long, default_value = "true")]
        release: bool,
    },
    /// Build and produce VMware Workstation .vmdk disk image
    Vmdk {
        #[arg(long, default_value = "true")]
        release: bool,
    },
    /// Build and produce VirtualBox .vdi disk image
    Vbox {
        #[arg(long, default_value = "true")]
        release: bool,
    },
    /// Headless QEMU test runner
    Test {
        #[arg(long, default_value = "true")]
        release: bool,
        #[arg(default_value = "tinycore")]
        cmd: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Test { release, cmd } => {
            let img_path = build_all(release, false, false)?;
            test_qemu(&img_path, &cmd)?;
        }

        Commands::Build { release } => {
            build_all(release, false, false)?;
        }
        Commands::Rootfs { release } => {
            build_all(release, false, true)?;
        }
        Commands::Run { release } => {
            let img_path = build_all(release, false, false)?;
            run_qemu(&img_path)?;
        }
        Commands::Vmdk { release } => {
            let img_path = build_all(release, true, false)?;
            println!("\n=======================================================");
            println!("  VMWARE WORKSTATION SETUP GUIDE FOR LUNIX OS          ");
            println!("=======================================================");
            println!("1. Open VMware Workstation / Player -> New Virtual Machine");
            println!("2. Select 'Custom (advanced)' -> Guest OS: 'Linux' -> 'Other Linux 64-bit'");
            println!("3. In VM Settings -> 'Options' tab -> 'Advanced' -> Firmware type: 'UEFI'");
            println!("4. In 'Hardware' tab -> 'Hard Disk' -> 'Use an existing virtual disk'");
            println!("   Select: {}", img_path.parent().unwrap().join("lunix.vmdk").display());
            println!("5. Power on the Virtual Machine! Lunix boots via UEFI GOP.");
            println!("=======================================================\n");
        }
        Commands::Vbox { release } => {
            let img_path = build_all(release, true, false)?;
            println!("\n=======================================================");
            println!("  VIRTUALBOX SETUP GUIDE FOR LUNIX OS                  ");
            println!("=======================================================");
            println!("1. Open VirtualBox -> Click 'New' VM");
            println!("2. Name: Lunix OS, Type: Linux, Version: Other Linux (64-bit)");
            println!("3. In VM Settings -> 'System' tab -> Motherboard -> Check 'Enable EFI'");
            println!("4. In 'Storage' tab -> Add Hard Disk -> Choose existing disk");
            println!("   Select: {}", img_path.parent().unwrap().join("lunix.vdi").display());
            println!("5. Power on the Virtual Machine!");
            println!("=======================================================\n");
        }
    }

    Ok(())
}

fn get_workspace_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir.parent().unwrap().to_path_buf()
}

fn build_all(release: bool, convert: bool, force_rootfs: bool) -> Result<PathBuf> {
    let root = get_workspace_root();
    println!("=======================================================");
    println!("   BUILDING LUNIX OS (Bootloader + Kernel + Image)     ");
    println!("=======================================================");

    // 1. Build Custom Rust UEFI Bootloader
    println!("[1/3] Compiling custom Rust UEFI Bootloader (lunix-bootloader)...");
    let mut bootloader_cmd = Command::new("cargo");
    bootloader_cmd
        .current_dir(&root)
        .arg("build")
        .arg("--package")
        .arg("lunix-bootloader")
        .arg("--target")
        .arg("x86_64-unknown-uefi");

    if release {
        bootloader_cmd.arg("--release");
    }

    let status = bootloader_cmd.status().context("Failed to run cargo build for bootloader")?;
    if !status.success() {
        bail!("Failed to compile bootloader!");
    }

    // 2. Build Lunix Core Kernel
    println!("[2/3] Compiling Lunix Core Kernel (lunix-kernel)...");
    let mut kernel_cmd = Command::new("cargo");
    kernel_cmd
        .current_dir(&root)
        .arg("build")
        .arg("--package")
        .arg("lunix-kernel")
        .arg("--target")
        .arg("x86_64-unknown-none");

    if release {
        kernel_cmd.arg("--release");
    }
    // e.g. LUNIX_KERNEL_FEATURES=strace enables verbose syscall tracing.
    if let Ok(features) = std::env::var("LUNIX_KERNEL_FEATURES") {
        kernel_cmd.arg("--features").arg(features);
    }

    let status = kernel_cmd.status().context("Failed to run cargo build for kernel")?;
    if !status.success() {
        bail!("Failed to compile kernel!");
    }

    // 3. Assemble the disk: GPT + FAT32 ESP (bootloader, kernel) + ext2 root from Arch packages
    println!("[3/3] Creating boot disk image (target/lunix.img)...");
    let profile = if release { "release" } else { "debug" };

    let bootloader_bin = root
        .join("target")
        .join("x86_64-unknown-uefi")
        .join(profile)
        .join("lunix-bootloader.efi");

    let kernel_bin = root
        .join("target")
        .join("x86_64-unknown-none")
        .join(profile)
        .join("lunix-kernel");

    let target_dir = root.join("target");
    fs::create_dir_all(&target_dir)?;
    let img_path = target_dir.join("lunix.img");

    let rootfs_img = disk::ensure_rootfs(&root, force_rootfs)?;
    disk::create_disk_image(&img_path, &bootloader_bin, &kernel_bin, &rootfs_img)?;
    println!("[+] Build complete! Boot disk image: {}", img_path.display());

    if convert {
        // VMware (.vmdk) and VirtualBox (.vdi) images are only made on request.
        convert_disk_images(&img_path)?;
    }

    Ok(img_path)
}

fn calc_crc32(data: &[u8]) -> u32 {
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


fn find_qemu() -> Option<PathBuf> {
    let root = get_workspace_root();
    // Check standard locations
    let candidates = [
        root.join("tools").join("qemu").join("qemu-system-x86_64.exe"),
        PathBuf::from("qemu-system-x86_64"),
        PathBuf::from(r"C:\Program Files\qemu\qemu-system-x86_64.exe"),
        PathBuf::from(r"C:\Program Files (x86)\qemu\qemu-system-x86_64.exe"),
        PathBuf::from(r"C:\tools\qemu\qemu-system-x86_64.exe"),
    ];

    for c in &candidates {
        if c.is_file() {
            return Some(c.clone());
        }
        if let Ok(output) = Command::new(c).arg("--version").output() {
            if output.status.success() {
                return Some(c.clone());
            }
        }
    }

    None
}

fn ensure_ovmf(root: &Path) -> Result<PathBuf> {
    let edk2_local = root.join("tools").join("qemu").join("share").join("edk2-x86_64-code.fd");
    if edk2_local.is_file() {
        return Ok(edk2_local);
    }

    let ovmf_path = root.join("OVMF.fd");
    if ovmf_path.is_file() {
        return Ok(ovmf_path);
    }

    // Check if QEMU directory has OVMF or edk2
    let qemu_ovmf_candidates = [
        root.join("tools").join("qemu").join("share").join("edk2-x86_64-code.fd"),
        root.join("tools").join("qemu").join("share").join("qemu").join("edk2-x86_64-code.fd"),
        root.join("tools").join("qemu").join("edk2-x86_64-code.fd"),
        PathBuf::from(r"C:\Program Files\qemu\share\edk2-x86_64-code.fd"),
        PathBuf::from(r"C:\Program Files\qemu\share\OVMF.fd"),
        PathBuf::from(r"C:\Program Files\qemu\share\qemu\edk2-x86_64-code.fd"),
    ];

    for c in &qemu_ovmf_candidates {
        if c.is_file() {
            return Ok(c.clone());
        }
    }

    // Fetch OVMF.fd prebuilt if not present
    println!("[+] Fetching standard OVMF UEFI firmware...");
    let url = "https://github.com/rust-osdev/ovmf-prebuilt/releases/latest/download/OVMF-X64.fd";
    
    // Use powershell curl / Invoke-WebRequest to fetch OVMF.fd
    let fetch_status = Command::new("powershell")
        .args(["-Command", &format!("Invoke-WebRequest -Uri '{}' -OutFile '{}'", url, ovmf_path.display())])
        .status()?;

    if fetch_status.success() && ovmf_path.is_file() {
        Ok(ovmf_path)
    } else {
        bail!("Could not find or download OVMF UEFI firmware (OVMF.fd)");
    }
}

fn run_qemu(img_path: &Path) -> Result<()> {
    let root = get_workspace_root();
    let qemu = find_qemu().context("Could not find qemu-system-x86_64. Please ensure QEMU is installed.")?;
    let ovmf = ensure_ovmf(&root)?;

    println!("=======================================================");
    println!("   LAUNCHING LUNIX OS IN QEMU (UEFI x86_64)            ");
    println!("=======================================================");
    println!("QEMU Binary: {}", qemu.display());
    println!("OVMF Firmware: {}", ovmf.display());
    println!("Disk Image: {}", img_path.display());
    println!("-------------------------------------------------------");

    let mut qemu_cmd = Command::new(qemu);

    let share_dir = root.join("tools").join("qemu").join("share");
    if share_dir.is_dir() {
        qemu_cmd.arg("-L").arg(share_dir);
    }

    qemu_cmd
        .arg("-drive")
        .arg(format!("if=pflash,format=raw,readonly=on,file={}", ovmf.display()))
        // IDE drive: OVMF's own boot-time firmware only reliably discovers and chainloads our
        // bootloader (\EFI\BOOT\BOOTX64.EFI on the ESP) through this controller — tried
        // VirtIO-only and OVMF's default-boot-file discovery failed to find it, dropping to
        // the UEFI Shell instead of booting. Keep this for firmware boot; the kernel itself
        // does not read from it once /dev/vda is available (see below).
        .arg("-drive")
        .arg(format!("format=raw,file={},if=ide", img_path.display()))
        // Same disk image, exposed a second time over VirtIO-Block (legacy transport: the
        // in-kernel driver speaks the I/O-port PFN-based legacy VirtIO-blk protocol, not the
        // modern MMIO capability layout). The kernel prefers this DMA-capable /dev/vda over
        // /dev/sda's PIO ATA path for the root filesystem once booted, which is most of the
        // boot-time win: real Arch bash reachable in ~4.5s vs ~7.5s IDE-only, measured with
        // this same image. (Two controllers on the same backing file were also observed to
        // slow later disk-heavy work like X server font loading — if that turns out to be
        // reproducible contention rather than incidental timing, revisit with the ESP and
        // root filesystem split across two separate backing files instead of one shared one.)
        .arg("-drive")
        .arg(format!("if=none,format=raw,file={},id=lunixvirtio", img_path.display()))
        .arg("-device")
        .arg("virtio-blk-pci,drive=lunixvirtio,disable-modern=on,disable-legacy=off")
        .arg("-netdev")
        .arg("user,id=net0")
        .arg("-device")
        .arg("e1000,netdev=net0")
        .arg("-smp")
        .arg("2")
        .arg("-chardev")
        .arg("stdio,id=char0,mux=off")
        .arg("-serial")
        .arg("chardev:char0")
        .arg("-m")
        .arg("512M")
        .arg("-vga")
        .arg("std")
        .stdin(std::process::Stdio::piped());

    let mut child = qemu_cmd.spawn().context("Failed to launch QEMU process")?;
    let mut child_stdin = child.stdin.take().expect("Failed to open child stdin");

    // Host-to-QEMU stdin forwarder with microsecond pacing
    // Completely eliminates Windows Console paste truncation & 16550 UART FIFO overruns
    std::thread::spawn(move || {
        let mut stdin = std::io::stdin().lock();
        let mut buf = [0u8; 1024];
        while let Ok(n) = stdin.read(&mut buf) {
            if n == 0 {
                break;
            }
            for &byte in &buf[..n] {
                if child_stdin.write_all(&[byte]).is_err() {
                    return;
                }
                let _ = child_stdin.flush();
                std::thread::sleep(std::time::Duration::from_millis(3));
            }
        }
    });

    let status = child.wait()?;

    println!("QEMU exited with status: {:?}", status);
    Ok(())
}

fn test_qemu(img_path: &Path, input_cmd: &str) -> Result<()> {
    let root = get_workspace_root();
    let qemu = find_qemu().context("Could not find qemu-system-x86_64.")?;
    let ovmf = ensure_ovmf(&root)?;

    println!("=======================================================");
    println!("   RUNNING HEADLESS QEMU TEST WITH INPUT: '{}'         ", input_cmd);
    println!("=======================================================");

    let mut qemu_cmd = Command::new(qemu);
    let share_dir = root.join("tools").join("qemu").join("share");
    if share_dir.is_dir() {
        qemu_cmd.arg("-L").arg(share_dir);
    }

    qemu_cmd
        .arg("-drive")
        .arg(format!("if=pflash,format=raw,readonly=on,file={}", ovmf.display()))
        // IDE drive: OVMF's own boot-time firmware only reliably discovers and chainloads our
        // bootloader (\EFI\BOOT\BOOTX64.EFI on the ESP) through this controller — tried
        // VirtIO-only and OVMF's default-boot-file discovery failed to find it, dropping to
        // the UEFI Shell instead of booting. Keep this for firmware boot; the kernel itself
        // does not read from it once /dev/vda is available (see below).
        .arg("-drive")
        .arg(format!("format=raw,file={},if=ide", img_path.display()))
        // Same disk image, exposed a second time over VirtIO-Block (legacy transport: the
        // in-kernel driver speaks the I/O-port PFN-based legacy VirtIO-blk protocol, not the
        // modern MMIO capability layout). The kernel prefers this DMA-capable /dev/vda over
        // /dev/sda's PIO ATA path for the root filesystem once booted, which is most of the
        // boot-time win: real Arch bash reachable in ~4.5s vs ~7.5s IDE-only, measured with
        // this same image. (Two controllers on the same backing file were also observed to
        // slow later disk-heavy work like X server font loading — if that turns out to be
        // reproducible contention rather than incidental timing, revisit with the ESP and
        // root filesystem split across two separate backing files instead of one shared one.)
        .arg("-drive")
        .arg(format!("if=none,format=raw,file={},id=lunixvirtio", img_path.display()))
        .arg("-device")
        .arg("virtio-blk-pci,drive=lunixvirtio,disable-modern=on,disable-legacy=off")
        .arg("-netdev")
        .arg("user,id=net0")
        .arg("-device")
        .arg("e1000,netdev=net0")
        .arg("-smp")
        .arg("2")
        .arg("-chardev")
        .arg("stdio,id=char0,mux=off")
        .arg("-serial")
        .arg("chardev:char0")
        .arg("-display")
        .arg("none")
        .arg("-m")
        .arg("512M")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit());

    let mut child = qemu_cmd.spawn().context("Failed to launch QEMU process")?;
    let mut child_stdin = child.stdin.take().expect("Failed to open child stdin");

    let cmd_to_send = format!("{}\n", input_cmd);
    std::thread::spawn(move || {
        // Wait 5.0s for kernel boot to reach shell prompt
        std::thread::sleep(std::time::Duration::from_millis(5000));
        for &byte in cmd_to_send.as_bytes() {
            let _ = child_stdin.write_all(&[byte]);
            let _ = child_stdin.flush();
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        // Let it run for 25 seconds then exit
        std::thread::sleep(std::time::Duration::from_millis(25000));
    });

    // Wait with timeout
    let start = std::time::Instant::now();
    loop {
        if let Ok(Some(status)) = child.try_wait() {
            println!("\n[+] QEMU process completed with status: {:?}", status);
            break;
        }
        if start.elapsed() > std::time::Duration::from_secs(35) {
            println!("\n[+] Test complete, terminating QEMU...");
            let _ = child.kill();
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }


    Ok(())
}


fn convert_disk_images(img_path: &Path) -> Result<()> {
    let target_dir = img_path.parent().unwrap();
    let root = get_workspace_root();
    let qemu_img = root.join("tools").join("qemu").join("qemu-img.exe");

    if qemu_img.exists() {
        // 1. Convert to VMDK for VMware Workstation
        let vmdk_path = target_dir.join("lunix.vmdk");
        println!("[+] Generating VMware Workstation disk image (target/lunix.vmdk)...");
        let status = Command::new(&qemu_img)
            .arg("convert")
            .arg("-f")
            .arg("raw")
            .arg("-O")
            .arg("vmdk")
            .arg("-o")
            .arg("subformat=monolithicFlat,adapter_type=ide")
            .arg(img_path)
            .arg(&vmdk_path)
            .status();
        if let Ok(st) = status {
            if st.success() {
                println!("    -> Created VMware disk: {}", vmdk_path.display());
            }
        }

        // 2. Convert to VDI for VirtualBox
        let vdi_path = target_dir.join("lunix.vdi");
        println!("[+] Generating VirtualBox disk image (target/lunix.vdi)...");
        let status = Command::new(&qemu_img)
            .arg("convert")
            .arg("-f")
            .arg("raw")
            .arg("-O")
            .arg("vdi")
            .arg(img_path)
            .arg(&vdi_path)
            .status();
        if let Ok(st) = status {
            if st.success() {
                println!("    -> Created VirtualBox disk: {}", vdi_path.display());
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn test_symbolicate_kernel() {
        let root = get_workspace_root();
        let kernel_elf = root.join("target").join("x86_64-unknown-none").join("release").join("lunix-kernel");
        if !kernel_elf.is_file() { return; }
        let data = std::fs::read(&kernel_elf).unwrap();
        
        // Find symbol table (.symtab) and string table (.strtab)
        let shoff = u64::from_le_bytes(data[40..48].try_into().unwrap()) as usize;
        let shentsize = u16::from_le_bytes(data[58..60].try_into().unwrap()) as usize;
        let shnum = u16::from_le_bytes(data[60..62].try_into().unwrap()) as usize;
        let shstrndx = u16::from_le_bytes(data[62..64].try_into().unwrap()) as usize;
        
        let shstr_off = shoff + shstrndx * shentsize;
        let shstr_data_off = u64::from_le_bytes(data[shstr_off+24..shstr_off+32].try_into().unwrap()) as usize;
        
        let mut symtab_off = 0;
        let mut symtab_sz = 0;
        let mut symtab_entsz = 0;
        let mut strtab_off = 0;
        
        for i in 0..shnum {
            let off = shoff + i * shentsize;
            let name_idx = u32::from_le_bytes(data[off..off+4].try_into().unwrap()) as usize;
            let name_str = std::ffi::CStr::from_bytes_until_nul(&data[shstr_data_off+name_idx..]).unwrap().to_str().unwrap();
            let sec_off = u64::from_le_bytes(data[off+24..off+32].try_into().unwrap()) as usize;
            let sec_sz = u64::from_le_bytes(data[off+32..off+40].try_into().unwrap()) as usize;
            let sec_entsz = u64::from_le_bytes(data[off+56..off+64].try_into().unwrap()) as usize;
            
            if name_str == ".symtab" {
                symtab_off = sec_off;
                symtab_sz = sec_sz;
                symtab_entsz = if sec_entsz > 0 { sec_entsz } else { 24 };
            } else if name_str == ".strtab" {
                strtab_off = sec_off;
            }
        }
        
        if symtab_off > 0 && strtab_off > 0 {
            let target_rip = 0x0204E2E9u64;
            let mut best_sym = "";
            let mut best_val = 0u64;
            let num_syms = symtab_sz / symtab_entsz;
            for i in 0..num_syms {
                let off = symtab_off + i * symtab_entsz;
                let name_idx = u32::from_le_bytes(data[off..off+4].try_into().unwrap()) as usize;
                let val = u64::from_le_bytes(data[off+8..off+16].try_into().unwrap());
                let sz = u64::from_le_bytes(data[off+16..off+24].try_into().unwrap());
                let sym_name = if name_idx > 0 && strtab_off + name_idx < data.len() {
                    std::ffi::CStr::from_bytes_until_nul(&data[strtab_off+name_idx..]).map(|s| s.to_str().unwrap_or("")).unwrap_or("")
                } else { "" };
                
                if val <= target_rip && target_rip < val + sz.max(1) {
                    println!("MATCH: RIP 0x{:X} is in {} (0x{:X}..0x{:X})", target_rip, sym_name, val, val + sz);
                }
                if val <= target_rip && val > best_val {
                    best_val = val;
                    best_sym = sym_name;
                }
            }
            println!("CLOSEST: RIP 0x{:X} -> {} + 0x{:X} (sym base 0x{:X})", target_rip, best_sym, target_rip - best_val, best_val);
        }
    }


    #[test]
    fn test_inspect_tinycore_elfs() {
        let root = get_workspace_root();
        let core_gz_path = root.join("target").join("corepure64.gz");
        let mut out = String::new();
        out.push_str(&format!("Checking core_gz_path: {}\n", core_gz_path.display()));
        if !core_gz_path.is_file() {
            out.push_str("corepure64.gz not found\n");
            let _ = std::fs::write("target/test_out.txt", out);
            return;
        }
        let mut gz_bytes = Vec::new();
        std::fs::File::open(&core_gz_path).unwrap().read_to_end(&mut gz_bytes).unwrap();
        let entries = unpack_cpio_gz(&gz_bytes).unwrap();
        out.push_str(&format!("Unpacked {} entries\n", entries.len()));
        
        for entry in &entries {
            if entry.name.contains("libc.so") {
                out.push_str(&format!("Found libc: {}\n", entry.name));
                let fork_addr = 0x0A927Au64;
                for i in 0..entry.data.len().saturating_sub(5) {
                    if entry.data[i] == 0xE8 { // call rel32
                        let rel = i32::from_le_bytes(entry.data[i+1..i+5].try_into().unwrap());
                        let target = (i as i64 + 5 + rel as i64) as u64;
                        if target == fork_addr {
                            out.push_str(&format!("Call to _Fork at 0x{:X}\n", i));
                            let start = i.saturating_sub(16);
                            let end = (i + 32).min(entry.data.len());
                            out.push_str(&format!("Bytes around caller 0x{:X}: {:02X?}\n", i, &entry.data[start..end]));
                        }
                    }
                }
            }
        }
        let _ = std::fs::write(root.join("target").join("test_out.txt"), out);
    }
}

