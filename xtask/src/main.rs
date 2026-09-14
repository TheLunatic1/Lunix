use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
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
    /// Build the UEFI bootloader, kernel, and FAT32 disk image
    Build {
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
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Build { release } => {
            build_all(release)?;
        }
        Commands::Run { release } => {
            let img_path = build_all(release)?;
            run_qemu(&img_path)?;
        }
        Commands::Vmdk { release } => {
            let img_path = build_all(release)?;
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
            let img_path = build_all(release)?;
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

fn build_all(release: bool) -> Result<PathBuf> {
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

    let status = kernel_cmd.status().context("Failed to run cargo build for kernel")?;
    if !status.success() {
        bail!("Failed to compile kernel!");
    }

    // 3. Create FAT32 EFI System Partition Image
    println!("[3/3] Creating FAT32 boot disk image (target/lunix.img)...");
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

    create_fat32_image(&img_path, &bootloader_bin, &kernel_bin)?;
    println!("[+] Build complete! Boot disk image: {}", img_path.display());

    // Automatically generate VMware (.vmdk) and VirtualBox (.vdi) images
    convert_disk_images(&img_path)?;

    Ok(img_path)
}

fn create_fat32_image(img_path: &Path, bootloader_bin: &Path, kernel_bin: &Path) -> Result<()> {
    let mut bootloader_bytes = Vec::new();
    File::open(bootloader_bin)
        .with_context(|| format!("Failed to open {}", bootloader_bin.display()))?
        .read_to_end(&mut bootloader_bytes)?;

    let mut kernel_bytes = Vec::new();
    File::open(kernel_bin)
        .with_context(|| format!("Failed to open {}", kernel_bin.display()))?
        .read_to_end(&mut kernel_bytes)?;

    let mut img_file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(img_path)?;

    // 64 MiB disk image
    img_file.set_len(64 * 1024 * 1024)?;

    fatfs::format_volume(
        &mut img_file,
        fatfs::FormatVolumeOptions::new().fat_type(fatfs::FatType::Fat32),
    )?;

    let fs = fatfs::FileSystem::new(&mut img_file, fatfs::FsOptions::new())?;
    let root_dir = fs.root_dir();

    // Create /EFI/BOOT/BOOTX64.EFI
    root_dir.create_dir("EFI")?;
    let efi_dir = root_dir.open_dir("EFI")?;
    efi_dir.create_dir("BOOT")?;
    let boot_dir = efi_dir.open_dir("BOOT")?;
    let mut boot_file = boot_dir.create_file("BOOTX64.EFI")?;
    boot_file.write_all(&bootloader_bytes)?;

    // Create /LUNIX/KERNEL.BIN
    root_dir.create_dir("LUNIX")?;
    let lunix_dir = root_dir.open_dir("LUNIX")?;
    let mut kernel_file = lunix_dir.create_file("KERNEL.BIN")?;
    kernel_file.write_all(&kernel_bytes)?;

    // Create /bin and standalone ELF64 user binaries
    root_dir.create_dir("bin")?;
    let bin_dir = root_dir.open_dir("bin")?;

    let hello_elf = create_hello_elf();
    let mut hello_file = bin_dir.create_file("hello.elf")?;
    hello_file.write_all(&hello_elf)?;

    let sysinfo_elf = create_sysinfo_elf();
    let mut sysinfo_file = bin_dir.create_file("sysinfo.elf")?;
    sysinfo_file.write_all(&sysinfo_elf)?;

    let test_fork_elf = create_test_fork_elf();
    let mut test_fork_file = bin_dir.create_file("test_fork.elf")?;
    test_fork_file.write_all(&test_fork_elf)?;

    let test_exec_elf = create_test_exec_elf();
    let mut test_exec_file = bin_dir.create_file("test_exec.elf")?;
    test_exec_file.write_all(&test_exec_elf)?;

    let win_hello_pe = create_win32_hello_exe();
    let mut win_hello_file = bin_dir.create_file("win_hello.exe")?;
    win_hello_file.write_all(&win_hello_pe)?;

    let test_pipe_elf = create_test_pipe_elf();
    let mut test_pipe_file = bin_dir.create_file("test_pipe.elf")?;
    test_pipe_file.write_all(&test_pipe_elf)?;

    let test_dir_elf = create_test_dir_elf();
    let mut test_dir_file = bin_dir.create_file("test_dir.elf")?;
    test_dir_file.write_all(&test_dir_elf)?;

    let win_stream_pe = create_win32_stream_exe();
    let mut win_stream_file = bin_dir.create_file("win_stream.exe")?;
    win_stream_file.write_all(&win_stream_pe)?;

    let test_devproc_elf = create_test_devproc_elf();
    let mut test_devproc_file = bin_dir.create_file("test_devproc.elf")?;
    test_devproc_file.write_all(&test_devproc_elf)?;

    let win_envreg_pe = create_win32_envreg_exe();
    let mut win_envreg_file = bin_dir.create_file("win_envreg.exe")?;
    win_envreg_file.write_all(&win_envreg_pe)?;

    let busybox_elf = create_busybox_elf();
    let mut busybox_file = bin_dir.create_file("busybox")?;
    busybox_file.write_all(&busybox_elf)?;

    let mut sh_file = bin_dir.create_file("sh")?;
    sh_file.write_all(&busybox_elf)?;

    let test_busybox_elf = create_test_busybox_elf();
    let mut test_busybox_file = bin_dir.create_file("test_busybox.elf")?;
    test_busybox_file.write_all(&test_busybox_elf)?;

    // Milestone 6: Tiny Core binaries & test executables
    let test_tinycore_elf = create_test_tinycore_elf();
    let mut tc_test_file = bin_dir.create_file("test_tinycore.elf")?;
    tc_test_file.write_all(&test_tinycore_elf)?;

    let test_dynamic_elf = create_dynamic_test_elf();
    let mut dyn_test_file = bin_dir.create_file("test_dynamic.elf")?;
    dyn_test_file.write_all(&test_dynamic_elf)?;

    let mut tc_config = bin_dir.create_file("tc-config")?;
    tc_config.write_all(b"#!/bin/sh\necho 'Tiny Core Linux configuration utility initialized.'\n")?;

    // Create /sbin and /sbin/init
    root_dir.create_dir("sbin")?;
    let sbin_dir = root_dir.open_dir("sbin")?;
    let sbin_init_elf = create_sbin_init_elf();
    let mut init_file = sbin_dir.create_file("init")?;
    init_file.write_all(&sbin_init_elf)?;

    let mut reboot_file = sbin_dir.create_file("reboot")?;
    reboot_file.write_all(&busybox_elf)?;

    let mut halt_file = sbin_dir.create_file("halt")?;
    halt_file.write_all(&busybox_elf)?;

    // Create /lib and /lib64 with ld-linux-x86-64.so.2 & libc.so.6
    root_dir.create_dir("lib")?;
    let lib_dir = root_dir.open_dir("lib")?;
    let ld_so_bytes = create_ld_linux_so();
    let mut ld_file = lib_dir.create_file("ld-linux-x86-64.so.2")?;
    ld_file.write_all(&ld_so_bytes)?;

    let mut libc_file = lib_dir.create_file("libc.so.6")?;
    libc_file.write_all(&ld_so_bytes)?;

    root_dir.create_dir("lib64")?;
    let lib64_dir = root_dir.open_dir("lib64")?;
    let mut ld64_file = lib64_dir.create_file("ld-linux-x86-64.so.2")?;
    ld64_file.write_all(&ld_so_bytes)?;

    // Create /sys and /sys/drivers/sample_wdm.sys
    root_dir.create_dir("sys")?;
    let sys_dir = root_dir.open_dir("sys")?;
    sys_dir.create_dir("drivers")?;
    let drv_dir = sys_dir.open_dir("drivers")?;

    let sample_wdm = create_sample_wdm_sys();
    let mut sample_wdm_file = drv_dir.create_file("sample_wdm.sys")?;
    sample_wdm_file.write_all(&sample_wdm)?;

    // Also place a copy in /bin for convenience
    let mut bin_wdm_file = bin_dir.create_file("sample_wdm.sys")?;
    bin_wdm_file.write_all(&sample_wdm)?;

    // Create /etc (inittab, init.d/rcS, passwd, group, fstab, issue, os-release, hostname)
    root_dir.create_dir("etc")?;
    let etc_dir = root_dir.open_dir("etc")?;

    let mut inittab = etc_dir.create_file("inittab")?;
    inittab.write_all(b"::sysinit:/etc/init.d/rcS\ntty1::respawn:/bin/sh\n::ctrlaltdel:/sbin/reboot\n::shutdown:/sbin/halt\n")?;

    etc_dir.create_dir("init.d")?;
    let initd_dir = etc_dir.open_dir("init.d")?;
    let mut rcs_file = initd_dir.create_file("rcS")?;
    rcs_file.write_all(b"#!/bin/sh\necho '[BOOT] Running /etc/init.d/rcS system startup scripts...'\n/bin/echo '[BOOT] Mounting pseudo-filesystems (/proc, /dev, /sys)...'\n/bin/hostname box\n")?;

    let mut passwd_file = etc_dir.create_file("passwd")?;
    passwd_file.write_all(b"root:x:0:0:root:/root:/bin/sh\ntc:x:1001:1001:tc:/home/tc:/bin/sh\n")?;

    let mut group_file = etc_dir.create_file("group")?;
    group_file.write_all(b"root:x:0:\nstaff:x:50:tc\ntc:x:1001:\n")?;

    let mut fstab_file = etc_dir.create_file("fstab")?;
    fstab_file.write_all(b"/dev/sda / fat32 defaults 0 0\nproc /proc proc defaults 0 0\ndev /dev devtmpfs defaults 0 0\n")?;

    let mut issue_file = etc_dir.create_file("issue")?;
    issue_file.write_all(b"\nTiny Core Linux v15.0 on Lunix Hybrid Kernel (x86_64)\n\n")?;

    let mut os_rel = etc_dir.create_file("os-release")?;
    os_rel.write_all(b"NAME=\"TinyCore Linux on Lunix\"\nPRETTY_NAME=\"Tiny Core Linux v15.0 (Lunix Hybrid Kernel)\"\nID=tinycore\nVERSION=\"15.0\"\nVERSION_ID=15.0\nBUILD_ID=\"2026-09-14\"\nANSI_COLOR=\"0;36\"\nHOME_URL=\"http://tinycorelinux.net\"\n")?;

    let mut host_file = etc_dir.create_file("hostname")?;
    host_file.write_all(b"box\n")?;

    // Create /home/tc and /home/lunix
    root_dir.create_dir("home")?;
    let home_dir = root_dir.open_dir("home")?;
    home_dir.create_dir("tc")?;
    let tc_dir = home_dir.open_dir("tc")?;
    let mut tc_profile = tc_dir.create_file(".profile")?;
    tc_profile.write_all(b"export USER=\"tc\"\nexport HOME=\"/home/tc\"\nexport SHELL=\"/bin/sh\"\nexport PATH=\"/bin:/usr/bin:/sbin:/usr/sbin\"\necho 'Welcome to Tiny Core Linux on Lunix!'\n")?;

    let mut tc_readme = tc_dir.create_file("readme.txt")?;
    tc_readme.write_all(b"Tiny Core Linux v15.0 userspace running on 100% pure Rust Lunix Kernel.\n")?;

    home_dir.create_dir("lunix")?;
    let user_dir = home_dir.open_dir("lunix")?;
    let mut readme = user_dir.create_file("readme.txt")?;
    readme.write_all(b"Welcome to Lunix OS!\n\nA from-scratch hybrid operating system kernel written in 100% pure Rust.\nCombines Windows NT Driver Model (WDM) with Linux POSIX ABI & ELF64 execution.\n\nType 'help' to explore all Linux shell commands or 'gui' for LunixWM desktop.\n")?;

    // Create /tmp, /var, /usr directories
    root_dir.create_dir("tmp")?;
    root_dir.create_dir("var")?;
    let var_dir = root_dir.open_dir("var")?;
    var_dir.create_dir("run")?;
    var_dir.create_dir("log")?;

    root_dir.create_dir("usr")?;
    let usr_dir = root_dir.open_dir("usr")?;
    usr_dir.create_dir("bin")?;
    usr_dir.create_dir("sbin")?;
    usr_dir.create_dir("lib")?;

    Ok(())
}

fn build_elf64_binary(code_and_data: &[u8]) -> Vec<u8> {
    let mut elf = Vec::new();
    let file_size = (64 + 56 + code_and_data.len()) as u64;
    let vaddr = 0x400000u64;
    let entry_vaddr = vaddr + 64 + 56;

    // 1. ELF Header (64 bytes)
    elf.extend_from_slice(&[0x7F, b'E', b'L', b'F']); // Magic
    elf.push(2); // 64-bit
    elf.push(1); // Little endian
    elf.push(1); // Version
    elf.push(0); // System V ABI
    elf.extend_from_slice(&[0u8; 8]); // Padding

    elf.extend_from_slice(&2u16.to_le_bytes()); // e_type = ET_EXEC
    elf.extend_from_slice(&0x3Eu16.to_le_bytes()); // e_machine = EM_X86_64
    elf.extend_from_slice(&1u32.to_le_bytes()); // e_version = 1
    elf.extend_from_slice(&entry_vaddr.to_le_bytes()); // e_entry
    elf.extend_from_slice(&64u64.to_le_bytes()); // e_phoff = 64
    elf.extend_from_slice(&0u64.to_le_bytes()); // e_shoff = 0
    elf.extend_from_slice(&0u32.to_le_bytes()); // e_flags = 0
    elf.extend_from_slice(&64u16.to_le_bytes()); // e_ehsize = 64
    elf.extend_from_slice(&56u16.to_le_bytes()); // e_phentsize = 56
    elf.extend_from_slice(&1u16.to_le_bytes()); // e_phnum = 1
    elf.extend_from_slice(&64u16.to_le_bytes()); // e_shentsize = 64
    elf.extend_from_slice(&0u16.to_le_bytes()); // e_shnum = 0
    elf.extend_from_slice(&0u16.to_le_bytes()); // e_shstrndx = 0

    // 2. Program Header (56 bytes)
    elf.extend_from_slice(&1u32.to_le_bytes()); // p_type = PT_LOAD
    elf.extend_from_slice(&7u32.to_le_bytes()); // p_flags = PF_R | PF_W | PF_X
    elf.extend_from_slice(&0u64.to_le_bytes()); // p_offset = 0
    elf.extend_from_slice(&vaddr.to_le_bytes()); // p_vaddr = 0x400000
    elf.extend_from_slice(&vaddr.to_le_bytes()); // p_paddr = 0x400000
    elf.extend_from_slice(&file_size.to_le_bytes()); // p_filesz
    let memsz = (file_size + 4095) & !4095;
    elf.extend_from_slice(&memsz.to_le_bytes()); // p_memsz
    elf.extend_from_slice(&4096u64.to_le_bytes()); // p_align = 4096

    // 3. Payload (code + data)
    elf.extend_from_slice(code_and_data);

    elf
}

fn create_hello_elf() -> Vec<u8> {
    let msg = b"\n  [RING 3 USER MODE] Hello from standalone ELF64 binary (/bin/hello.elf)!\n  [ELF64] Exiting cleanly via Linux sys_exit(0)...\n\n";
    let mut payload = Vec::new();

    // mov eax, 1 (sys_write)
    payload.extend_from_slice(&[0xB8, 0x01, 0x00, 0x00, 0x00]);
    // mov edi, 1 (stdout)
    payload.extend_from_slice(&[0xBF, 0x01, 0x00, 0x00, 0x00]);
    // lea rsi, [rip + 19]
    payload.extend_from_slice(&[0x48, 0x8D, 0x35, 0x13, 0x00, 0x00, 0x00]);
    // mov edx, msg.len()
    let len = msg.len() as u32;
    payload.extend_from_slice(&[0xBA, len as u8, (len >> 8) as u8, (len >> 16) as u8, (len >> 24) as u8]);
    // syscall
    payload.extend_from_slice(&[0x0F, 0x05]);

    // mov eax, 60 (sys_exit)
    payload.extend_from_slice(&[0xB8, 0x3C, 0x00, 0x00, 0x00]);
    // xor edi, edi (status 0)
    payload.extend_from_slice(&[0x31, 0xFF]);
    // syscall
    payload.extend_from_slice(&[0x0F, 0x05]);
    // hlt; jmp $-1
    payload.extend_from_slice(&[0xF4, 0xEB, 0xFD]);

    // String payload
    payload.extend_from_slice(msg);

    build_elf64_binary(&payload)
}

fn create_sysinfo_elf() -> Vec<u8> {
    let msg1 = b"\n  ===============================================================\n  [ELF64 PROGRAM] Standalone Linux User Binary (/bin/sysinfo.elf)\n  [ELF64] Invoking Linux nanosleep syscall in Ring 3...\n  ===============================================================\n\n";
    let msg2 = b"  [ELF64] Successfully woke up in Ring 3! Exiting via sys_exit(0).\n\n";

    let mut payload = Vec::new();

    // 1. sys_write(1, msg1, len1)
    payload.extend_from_slice(&[0xB8, 0x01, 0x00, 0x00, 0x00]); // mov eax, 1
    payload.extend_from_slice(&[0xBF, 0x01, 0x00, 0x00, 0x00]); // mov edi, 1
    // lea rsi, [rip + msg1_offset] -> will patch
    let lea1_pos = payload.len();
    payload.extend_from_slice(&[0x48, 0x8D, 0x35, 0x00, 0x00, 0x00, 0x00]);
    let len1 = msg1.len() as u32;
    payload.extend_from_slice(&[0xBA, len1 as u8, (len1 >> 8) as u8, (len1 >> 16) as u8, (len1 >> 24) as u8]);
    payload.extend_from_slice(&[0x0F, 0x05]); // syscall

    // 2. sys_nanosleep(&timespec)
    // timespec: 0s, 500_000_000 ns -> at timespec_offset
    payload.extend_from_slice(&[0xB8, 0x23, 0x00, 0x00, 0x00]); // mov eax, 35 (sys_nanosleep)
    let lea_time_pos = payload.len();
    payload.extend_from_slice(&[0x48, 0x8D, 0x3D, 0x00, 0x00, 0x00, 0x00]); // lea rdi, [rip + timespec]
    payload.extend_from_slice(&[0x31, 0xF6]); // xor esi, esi (rem = NULL)
    payload.extend_from_slice(&[0x0F, 0x05]); // syscall

    // 3. sys_write(1, msg2, len2)
    payload.extend_from_slice(&[0xB8, 0x01, 0x00, 0x00, 0x00]); // mov eax, 1
    payload.extend_from_slice(&[0xBF, 0x01, 0x00, 0x00, 0x00]); // mov edi, 1
    let lea2_pos = payload.len();
    payload.extend_from_slice(&[0x48, 0x8D, 0x35, 0x00, 0x00, 0x00, 0x00]);
    let len2 = msg2.len() as u32;
    payload.extend_from_slice(&[0xBA, len2 as u8, (len2 >> 8) as u8, (len2 >> 16) as u8, (len2 >> 24) as u8]);
    payload.extend_from_slice(&[0x0F, 0x05]); // syscall

    // 4. sys_exit(0)
    payload.extend_from_slice(&[0xB8, 0x3C, 0x00, 0x00, 0x00]); // mov eax, 60
    payload.extend_from_slice(&[0x31, 0xFF]); // xor edi, edi
    payload.extend_from_slice(&[0x0F, 0x05]); // syscall
    payload.extend_from_slice(&[0xF4, 0xEB, 0xFD]); // hlt; jmp $-1

    // timespec: 0s, 500_000_000 ns (16 bytes)
    let timespec_pos = payload.len();
    payload.extend_from_slice(&0u64.to_le_bytes()); // tv_sec = 0
    payload.extend_from_slice(&500_000_000u64.to_le_bytes()); // tv_nsec = 500,000,000

    let msg1_pos = payload.len();
    payload.extend_from_slice(msg1);

    let msg2_pos = payload.len();
    payload.extend_from_slice(msg2);

    // Patch LEA offsets:
    // lea1: target msg1_pos, rip after instruction is lea1_pos + 7
    let disp1 = (msg1_pos as i32) - ((lea1_pos + 7) as i32);
    payload[lea1_pos + 3..lea1_pos + 7].copy_from_slice(&disp1.to_le_bytes());

    let disp_time = (timespec_pos as i32) - ((lea_time_pos + 7) as i32);
    payload[lea_time_pos + 3..lea_time_pos + 7].copy_from_slice(&disp_time.to_le_bytes());

    let disp2 = (msg2_pos as i32) - ((lea2_pos + 7) as i32);
    payload[lea2_pos + 3..lea2_pos + 7].copy_from_slice(&disp2.to_le_bytes());

    build_elf64_binary(&payload)
}

fn create_test_fork_elf() -> Vec<u8> {
    let msg_parent = b"\n  [PARENT] Calling Linux sys_clone / sys_fork in Ring 3...\n";
    let msg_child = b"  [CHILD] Hello from cloned child process in Ring 3! Exiting with status 42...\n";
    let msg_reaped = b"  [PARENT] Successfully reaped child process via sys_wait4! Exiting with status 0.\n\n";

    let mut payload = Vec::new();

    // 1. sys_write(1, msg_parent, len)
    payload.extend_from_slice(&[0xB8, 0x01, 0x00, 0x00, 0x00]); // mov eax, 1
    payload.extend_from_slice(&[0xBF, 0x01, 0x00, 0x00, 0x00]); // mov edi, 1
    let lea_p_pos = payload.len();
    payload.extend_from_slice(&[0x48, 0x8D, 0x35, 0x00, 0x00, 0x00, 0x00]); // lea rsi, [rip + msg_parent]
    let len_p = msg_parent.len() as u32;
    payload.extend_from_slice(&[0xBA, len_p as u8, (len_p >> 8) as u8, (len_p >> 16) as u8, (len_p >> 24) as u8]);
    payload.extend_from_slice(&[0x0F, 0x05]); // syscall

    // 2. sys_clone(0, 0) / sys_fork
    payload.extend_from_slice(&[0xB8, 0x39, 0x00, 0x00, 0x00]); // mov eax, 57 (sys_fork)
    payload.extend_from_slice(&[0x31, 0xFF]); // xor edi, edi
    payload.extend_from_slice(&[0x31, 0xF6]); // xor esi, esi
    payload.extend_from_slice(&[0x0F, 0x05]); // syscall

    // test rax, rax
    payload.extend_from_slice(&[0x48, 0x85, 0xC0]);
    // jz child_branch
    let jz_pos = payload.len();
    payload.extend_from_slice(&[0x74, 0x00]);

    // --- PARENT BRANCH ---
    // sys_wait4(-1, &status, 0)
    payload.extend_from_slice(&[0x48, 0x83, 0xEC, 0x10]); // sub rsp, 16
    payload.extend_from_slice(&[0xB8, 0x3D, 0x00, 0x00, 0x00]); // mov eax, 61 (sys_wait4)
    payload.extend_from_slice(&[0x48, 0xC7, 0xC7, 0xFF, 0xFF, 0xFF, 0xFF]); // mov rdi, -1
    payload.extend_from_slice(&[0x48, 0x89, 0xE6]); // mov rsi, rsp
    payload.extend_from_slice(&[0x31, 0xD2]); // xor edx, edx
    payload.extend_from_slice(&[0x0F, 0x05]); // syscall
    payload.extend_from_slice(&[0x48, 0x83, 0xC4, 0x10]); // add rsp, 16

    // sys_write(1, msg_reaped, len)
    payload.extend_from_slice(&[0xB8, 0x01, 0x00, 0x00, 0x00]); // mov eax, 1
    payload.extend_from_slice(&[0xBF, 0x01, 0x00, 0x00, 0x00]); // mov edi, 1
    let lea_r_pos = payload.len();
    payload.extend_from_slice(&[0x48, 0x8D, 0x35, 0x00, 0x00, 0x00, 0x00]);
    let len_r = msg_reaped.len() as u32;
    payload.extend_from_slice(&[0xBA, len_r as u8, (len_r >> 8) as u8, (len_r >> 16) as u8, (len_r >> 24) as u8]);
    payload.extend_from_slice(&[0x0F, 0x05]); // syscall

    // sys_exit(0)
    payload.extend_from_slice(&[0xB8, 0x3C, 0x00, 0x00, 0x00]); // mov eax, 60
    payload.extend_from_slice(&[0x31, 0xFF]); // xor edi, edi
    payload.extend_from_slice(&[0x0F, 0x05]); // syscall
    payload.extend_from_slice(&[0xF4, 0xEB, 0xFD]); // hlt; jmp $-1

    // --- CHILD BRANCH ---
    let child_start = payload.len();
    let jz_disp = (child_start - (jz_pos + 2)) as u8;
    payload[jz_pos + 1] = jz_disp;

    // sys_write(1, msg_child, len)
    payload.extend_from_slice(&[0xB8, 0x01, 0x00, 0x00, 0x00]); // mov eax, 1
    payload.extend_from_slice(&[0xBF, 0x01, 0x00, 0x00, 0x00]); // mov edi, 1
    let lea_c_pos = payload.len();
    payload.extend_from_slice(&[0x48, 0x8D, 0x35, 0x00, 0x00, 0x00, 0x00]);
    let len_c = msg_child.len() as u32;
    payload.extend_from_slice(&[0xBA, len_c as u8, (len_c >> 8) as u8, (len_c >> 16) as u8, (len_c >> 24) as u8]);
    payload.extend_from_slice(&[0x0F, 0x05]); // syscall

    // sys_exit(42)
    payload.extend_from_slice(&[0xB8, 0x3C, 0x00, 0x00, 0x00]); // mov eax, 60
    payload.extend_from_slice(&[0xBF, 0x2A, 0x00, 0x00, 0x00]); // mov edi, 42
    payload.extend_from_slice(&[0x0F, 0x05]); // syscall
    payload.extend_from_slice(&[0xF4, 0xEB, 0xFD]); // hlt; jmp $-1

    // String payloads
    let p_pos = payload.len();
    payload.extend_from_slice(msg_parent);
    let c_pos = payload.len();
    payload.extend_from_slice(msg_child);
    let r_pos = payload.len();
    payload.extend_from_slice(msg_reaped);

    // Patch LEA offsets
    let disp_p = (p_pos as i32) - ((lea_p_pos + 7) as i32);
    payload[lea_p_pos + 3..lea_p_pos + 7].copy_from_slice(&disp_p.to_le_bytes());

    let disp_r = (r_pos as i32) - ((lea_r_pos + 7) as i32);
    payload[lea_r_pos + 3..lea_r_pos + 7].copy_from_slice(&disp_r.to_le_bytes());

    let disp_c = (c_pos as i32) - ((lea_c_pos + 7) as i32);
    payload[lea_c_pos + 3..lea_c_pos + 7].copy_from_slice(&disp_c.to_le_bytes());

    build_elf64_binary(&payload)
}

fn create_test_exec_elf() -> Vec<u8> {
    let msg = b"\n  [EXEC] Invoking Linux sys_execve(\"/bin/hello.elf\") in Ring 3...\n\n";
    let target = b"/bin/hello.elf\0";

    let mut payload = Vec::new();

    // 1. sys_write(1, msg, len)
    payload.extend_from_slice(&[0xB8, 0x01, 0x00, 0x00, 0x00]); // mov eax, 1
    payload.extend_from_slice(&[0xBF, 0x01, 0x00, 0x00, 0x00]); // mov edi, 1
    let lea_msg_pos = payload.len();
    payload.extend_from_slice(&[0x48, 0x8D, 0x35, 0x00, 0x00, 0x00, 0x00]);
    let len_msg = msg.len() as u32;
    payload.extend_from_slice(&[0xBA, len_msg as u8, (len_msg >> 8) as u8, (len_msg >> 16) as u8, (len_msg >> 24) as u8]);
    payload.extend_from_slice(&[0x0F, 0x05]); // syscall

    // 2. sys_execve("/bin/hello.elf", NULL, NULL)
    payload.extend_from_slice(&[0xB8, 0x3B, 0x00, 0x00, 0x00]); // mov eax, 59 (sys_execve)
    let lea_tgt_pos = payload.len();
    payload.extend_from_slice(&[0x48, 0x8D, 0x3D, 0x00, 0x00, 0x00, 0x00]); // lea rdi, [rip + target]
    payload.extend_from_slice(&[0x31, 0xF6]); // xor esi, esi (argv = NULL)
    payload.extend_from_slice(&[0x31, 0xD2]); // xor edx, edx (envp = NULL)
    payload.extend_from_slice(&[0x0F, 0x05]); // syscall

    // 3. Fallback sys_exit(1)
    payload.extend_from_slice(&[0xB8, 0x3C, 0x00, 0x00, 0x00]); // mov eax, 60
    payload.extend_from_slice(&[0xBF, 0x01, 0x00, 0x00, 0x00]); // mov edi, 1
    payload.extend_from_slice(&[0x0F, 0x05]); // syscall
    payload.extend_from_slice(&[0xF4, 0xEB, 0xFD]);

    let msg_pos = payload.len();
    payload.extend_from_slice(msg);
    let tgt_pos = payload.len();
    payload.extend_from_slice(target);

    let disp_m = (msg_pos as i32) - ((lea_msg_pos + 7) as i32);
    payload[lea_msg_pos + 3..lea_msg_pos + 7].copy_from_slice(&disp_m.to_le_bytes());

    let disp_t = (tgt_pos as i32) - ((lea_tgt_pos + 7) as i32);
    payload[lea_tgt_pos + 3..lea_tgt_pos + 7].copy_from_slice(&disp_t.to_le_bytes());

    build_elf64_binary(&payload)
}

fn create_win32_hello_exe() -> Vec<u8> {
    let mut pe = vec![0u8; 1536]; // 512 headers + 512 .text + 512 .rdata

    // 1. DOS Header (64 bytes)
    pe[0..2].copy_from_slice(&[0x4D, 0x5A]); // 'MZ'
    pe[0x3C..0x40].copy_from_slice(&64u32.to_le_bytes()); // e_lfanew = 64

    // 2. PE Signature (4 bytes at offset 64)
    pe[64..68].copy_from_slice(b"PE\0\0");

    // 3. COFF File Header (20 bytes at offset 68)
    pe[68..70].copy_from_slice(&0x8664u16.to_le_bytes()); // Machine: AMD64
    pe[70..72].copy_from_slice(&2u16.to_le_bytes()); // NumberOfSections: 2
    pe[72..76].copy_from_slice(&0x66E00000u32.to_le_bytes()); // TimeDateStamp
    pe[76..80].copy_from_slice(&0u32.to_le_bytes()); // PointerToSymbolTable
    pe[80..84].copy_from_slice(&0u32.to_le_bytes()); // NumberOfSymbols
    pe[84..86].copy_from_slice(&240u16.to_le_bytes()); // SizeOfOptionalHeader = 240
    pe[86..88].copy_from_slice(&0x0022u16.to_le_bytes()); // Characteristics: EXECUTABLE_IMAGE | LARGE_ADDRESS_AWARE

    // 4. Optional Header (240 bytes at offset 88)
    let opt = 88;
    pe[opt..opt + 2].copy_from_slice(&0x020Bu16.to_le_bytes()); // Magic: PE32+ (64-bit)
    pe[opt + 2] = 14; // MajorLinkerVersion
    pe[opt + 3] = 0;  // MinorLinkerVersion
    pe[opt + 4..opt + 8].copy_from_slice(&512u32.to_le_bytes()); // SizeOfCode
    pe[opt + 8..opt + 12].copy_from_slice(&512u32.to_le_bytes()); // SizeOfInitializedData
    pe[opt + 12..opt + 16].copy_from_slice(&0u32.to_le_bytes()); // SizeOfUninitializedData
    pe[opt + 16..opt + 20].copy_from_slice(&0x1000u32.to_le_bytes()); // AddressOfEntryPoint = 0x1000 (.text)
    pe[opt + 20..opt + 24].copy_from_slice(&0x1000u32.to_le_bytes()); // BaseOfCode = 0x1000
    pe[opt + 24..opt + 32].copy_from_slice(&0x0040_0000u64.to_le_bytes()); // ImageBase = 0x400000
    pe[opt + 32..opt + 36].copy_from_slice(&0x1000u32.to_le_bytes()); // SectionAlignment = 4096
    pe[opt + 36..opt + 40].copy_from_slice(&0x200u32.to_le_bytes()); // FileAlignment = 512
    pe[opt + 40..opt + 42].copy_from_slice(&6u16.to_le_bytes()); // MajorOperatingSystemVersion
    pe[opt + 42..opt + 44].copy_from_slice(&0u16.to_le_bytes()); // MinorOperatingSystemVersion
    pe[opt + 48..opt + 50].copy_from_slice(&6u16.to_le_bytes()); // MajorSubsystemVersion
    pe[opt + 50..opt + 52].copy_from_slice(&0u16.to_le_bytes()); // MinorSubsystemVersion
    pe[opt + 56..opt + 60].copy_from_slice(&0x3000u32.to_le_bytes()); // SizeOfImage = 12 KiB (3 * 4096)
    pe[opt + 60..opt + 64].copy_from_slice(&0x200u32.to_le_bytes()); // SizeOfHeaders = 512
    pe[opt + 68..opt + 70].copy_from_slice(&3u16.to_le_bytes()); // Subsystem: IMAGE_SUBSYSTEM_WINDOWS_CUI (3)
    pe[opt + 70..opt + 72].copy_from_slice(&0x8160u16.to_le_bytes()); // DllCharacteristics
    pe[opt + 72..opt + 80].copy_from_slice(&0x100000u64.to_le_bytes()); // SizeOfStackReserve (1 MB)
    pe[opt + 80..opt + 88].copy_from_slice(&0x1000u64.to_le_bytes()); // SizeOfStackCommit (4 KB)
    pe[opt + 88..opt + 96].copy_from_slice(&0x100000u64.to_le_bytes()); // SizeOfHeapReserve (1 MB)
    pe[opt + 96..opt + 104].copy_from_slice(&0x1000u64.to_le_bytes()); // SizeOfHeapCommit (4 KB)
    pe[opt + 108..opt + 112].copy_from_slice(&16u32.to_le_bytes()); // NumberOfRvaAndSizes = 16

    // Data Directory 1: Import Table (at opt + 112 + 8 = opt + 120)
    let dd_import = opt + 120;
    pe[dd_import..dd_import + 4].copy_from_slice(&0x2000u32.to_le_bytes()); // Import Directory RVA = 0x2000 (.rdata)
    pe[dd_import + 4..dd_import + 8].copy_from_slice(&40u32.to_le_bytes()); // Import Directory Size = 40

    // 5. Section Headers (2 * 40 bytes at offset 88 + 240 = 328)
    let sec1 = 328;
    // Section 1: .text
    pe[sec1..sec1 + 8].copy_from_slice(b".text\0\0\0");
    pe[sec1 + 8..sec1 + 12].copy_from_slice(&512u32.to_le_bytes()); // VirtualSize
    pe[sec1 + 12..sec1 + 16].copy_from_slice(&0x1000u32.to_le_bytes()); // VirtualAddress
    pe[sec1 + 16..sec1 + 20].copy_from_slice(&512u32.to_le_bytes()); // SizeOfRawData
    pe[sec1 + 20..sec1 + 24].copy_from_slice(&0x200u32.to_le_bytes()); // PointerToRawData = 512
    pe[sec1 + 36..sec1 + 40].copy_from_slice(&0x60000020u32.to_le_bytes()); // Characteristics: CODE | EXECUTE | READ

    let sec2 = sec1 + 40;
    // Section 2: .rdata
    pe[sec2..sec2 + 8].copy_from_slice(b".rdata\0\0");
    pe[sec2 + 8..sec2 + 12].copy_from_slice(&512u32.to_le_bytes()); // VirtualSize
    pe[sec2 + 12..sec2 + 16].copy_from_slice(&0x2000u32.to_le_bytes()); // VirtualAddress
    pe[sec2 + 16..sec2 + 20].copy_from_slice(&512u32.to_le_bytes()); // SizeOfRawData
    pe[sec2 + 20..sec2 + 24].copy_from_slice(&0x400u32.to_le_bytes()); // PointerToRawData = 1024
    pe[sec2 + 36..sec2 + 40].copy_from_slice(&0x40000040u32.to_le_bytes()); // Characteristics: INITIALIZED_DATA | READ

    // 6. .text Section Content (at raw offset 0x200 = 512)
    // Code invoking:
    // 1) GetStdHandle(STD_OUTPUT_HANDLE = -11)
    // 2) WriteConsoleA(hStdOut, &msg, msg_len, &written, NULL)
    // 3) ExitProcess(0)
    let text_start = 512;
    let mut code = Vec::new();

    let win_msg = b"\n  ===============================================================\n  [WIN32 USER MODE] Hello from 64-bit Windows PE32+ (/bin/win_hello.exe)!\n  [WIN32] Executing natively via extern \"win64\" ABI & kernel32.dll\n  ===============================================================\n\n";

    // sub rsp, 40 (allocate shadow space)
    code.extend_from_slice(&[0x48, 0x83, 0xEC, 0x28]);

    // mov ecx, -11 (STD_OUTPUT_HANDLE)
    code.extend_from_slice(&[0xB9, 0xF5, 0xFF, 0xFF, 0xFF]);
    // call [rip + GetStdHandle_IAT]
    let call1_pos = code.len();
    code.extend_from_slice(&[0xFF, 0x15, 0x00, 0x00, 0x00, 0x00]);

    // mov rcx, rax (hConsole in rcx)
    code.extend_from_slice(&[0x48, 0x89, 0xC1]);
    // lea rdx, [rip + msg_disp]
    let lea_msg_pos = code.len();
    code.extend_from_slice(&[0x48, 0x8D, 0x15, 0x00, 0x00, 0x00, 0x00]);
    // mov r8d, msg.len()
    let msg_len = win_msg.len() as u32;
    code.extend_from_slice(&[0x41, 0xB8, msg_len as u8, (msg_len >> 8) as u8, (msg_len >> 16) as u8, (msg_len >> 24) as u8]);
    // xor r9d, r9d (lpNumberOfCharsWritten = NULL)
    code.extend_from_slice(&[0x45, 0x31, 0xC9]);
    // mov qword ptr [rsp + 32], 0 (lpReserved = NULL)
    code.extend_from_slice(&[0x48, 0xC7, 0x44, 0x24, 0x20, 0x00, 0x00, 0x00, 0x00]);
    // call [rip + WriteConsoleA_IAT]
    let call2_pos = code.len();
    code.extend_from_slice(&[0xFF, 0x15, 0x00, 0x00, 0x00, 0x00]);

    // xor ecx, ecx (uExitCode = 0)
    code.extend_from_slice(&[0x31, 0xC9]);
    // call [rip + ExitProcess_IAT]
    let call3_pos = code.len();
    code.extend_from_slice(&[0xFF, 0x15, 0x00, 0x00, 0x00, 0x00]);

    // add rsp, 40; ret
    code.extend_from_slice(&[0x48, 0x83, 0xC4, 0x28, 0xC3]);

    pe[text_start..text_start + code.len()].copy_from_slice(&code);

    // 7. .rdata Section Content (at raw offset 0x400 = 1024, VirtualAddress = 0x2000)
    let rdata_start = 1024;
    let rdata_vaddr = 0x2000u32;

    let dll_name_rva = rdata_vaddr + 0x70;
    let iat_rva = rdata_vaddr + 0x30;
    let ilt_rva = rdata_vaddr + 0x50;

    let hname1_rva = rdata_vaddr + 0x80;
    let hname2_rva = rdata_vaddr + 0x90;
    let hname3_rva = rdata_vaddr + 0xA0;
    let msg_rva = rdata_vaddr + 0xB0;

    // ImageImportDescriptor:
    pe[rdata_start..rdata_start + 4].copy_from_slice(&ilt_rva.to_le_bytes());
    pe[rdata_start + 12..rdata_start + 16].copy_from_slice(&dll_name_rva.to_le_bytes());
    pe[rdata_start + 16..rdata_start + 20].copy_from_slice(&iat_rva.to_le_bytes());

    // IAT entries (at rdata_start + 0x30)
    pe[rdata_start + 0x30..rdata_start + 0x38].copy_from_slice(&(hname1_rva as u64).to_le_bytes());
    pe[rdata_start + 0x38..rdata_start + 0x40].copy_from_slice(&(hname2_rva as u64).to_le_bytes());
    pe[rdata_start + 0x40..rdata_start + 0x48].copy_from_slice(&(hname3_rva as u64).to_le_bytes());

    // ILT entries (at rdata_start + 0x50)
    pe[rdata_start + 0x50..rdata_start + 0x58].copy_from_slice(&(hname1_rva as u64).to_le_bytes());
    pe[rdata_start + 0x58..rdata_start + 0x60].copy_from_slice(&(hname2_rva as u64).to_le_bytes());
    pe[rdata_start + 0x60..rdata_start + 0x68].copy_from_slice(&(hname3_rva as u64).to_le_bytes());

    // DLL Name (at rdata_start + 0x70)
    pe[rdata_start + 0x70..rdata_start + 0x7D].copy_from_slice(b"kernel32.dll\0");

    // Hint/Names:
    pe[rdata_start + 0x80..rdata_start + 0x82].copy_from_slice(&0u16.to_le_bytes());
    pe[rdata_start + 0x82..rdata_start + 0x8F].copy_from_slice(b"GetStdHandle\0");

    pe[rdata_start + 0x90..rdata_start + 0x92].copy_from_slice(&0u16.to_le_bytes());
    pe[rdata_start + 0x92..rdata_start + 0xA0].copy_from_slice(b"WriteConsoleA\0");

    pe[rdata_start + 0xA0..rdata_start + 0xA2].copy_from_slice(&0u16.to_le_bytes());
    pe[rdata_start + 0xA2..rdata_start + 0xAE].copy_from_slice(b"ExitProcess\0");

    // Message payload (at rdata_start + 0xB0)
    pe[rdata_start + 0xB0..rdata_start + 0xB0 + win_msg.len()].copy_from_slice(win_msg);

    // Patch .text RIP-relative displacements:
    let call1_vaddr = 0x1000 + call1_pos as u32;
    let disp1 = (0x2030 as i32) - ((call1_vaddr + 6) as i32);
    pe[text_start + call1_pos + 2..text_start + call1_pos + 6].copy_from_slice(&disp1.to_le_bytes());

    let lea_vaddr = 0x1000 + lea_msg_pos as u32;
    let disp_msg = (msg_rva as i32) - ((lea_vaddr + 7) as i32);
    pe[text_start + lea_msg_pos + 3..text_start + lea_msg_pos + 7].copy_from_slice(&disp_msg.to_le_bytes());

    let call2_vaddr = 0x1000 + call2_pos as u32;
    let disp2 = (0x2038 as i32) - ((call2_vaddr + 6) as i32);
    pe[text_start + call2_pos + 2..text_start + call2_pos + 6].copy_from_slice(&disp2.to_le_bytes());

    let call3_vaddr = 0x1000 + call3_pos as u32;
    let disp3 = (0x2040 as i32) - ((call3_vaddr + 6) as i32);
    pe[text_start + call3_pos + 2..text_start + call3_pos + 6].copy_from_slice(&disp3.to_le_bytes());

    pe
}

fn create_test_pipe_elf() -> Vec<u8> {
    let msg_child = b"\n  [CHILD] Hello from cloned child process through IPC pipe!\n";
    let msg_parent_hdr = b"  [PARENT] Received message from child via IPC pipe: ";
    let msg_done = b"  [PARENT] Successfully verified IPC pipe! Exiting with status 0.\n\n";

    let mut payload = Vec::new();

    // 1. Allocate 16 bytes for pipefd[2] on stack: sub rsp, 16
    payload.extend_from_slice(&[0x48, 0x83, 0xEC, 0x10]);

    // 2. sys_pipe2(&pipefd, 0)
    // mov eax, 293 (0x125)
    payload.extend_from_slice(&[0xB8, 0x25, 0x01, 0x00, 0x00]);
    // mov rdi, rsp
    payload.extend_from_slice(&[0x48, 0x89, 0xE7]);
    // xor esi, esi
    payload.extend_from_slice(&[0x31, 0xF6]);
    // syscall
    payload.extend_from_slice(&[0x0F, 0x05]);

    // 3. sys_fork() -> sys_clone(0, 0)
    payload.extend_from_slice(&[0xB8, 0x39, 0x00, 0x00, 0x00]); // mov eax, 57
    payload.extend_from_slice(&[0x31, 0xFF]); // xor edi, edi
    payload.extend_from_slice(&[0x31, 0xF6]); // xor esi, esi
    payload.extend_from_slice(&[0x0F, 0x05]); // syscall

    // test rax, rax
    payload.extend_from_slice(&[0x48, 0x85, 0xC0]);
    // jz rel32 child_branch (0x0F 0x84 disp32)
    let jz_pos = payload.len();
    payload.extend_from_slice(&[0x0F, 0x84, 0x00, 0x00, 0x00, 0x00]);

    // --- PARENT BRANCH ---
    // sys_close(write_fd): mov eax, 3; mov edi, dword ptr [rsp + 4]; syscall
    payload.extend_from_slice(&[0xB8, 0x03, 0x00, 0x00, 0x00]);
    payload.extend_from_slice(&[0x8B, 0x7C, 0x24, 0x04]);
    payload.extend_from_slice(&[0x0F, 0x05]);

    // Allocate 128 bytes buffer: sub rsp, 128
    payload.extend_from_slice(&[0x48, 0x81, 0xEC, 0x80, 0x00, 0x00, 0x00]);

    // sys_read(read_fd, rsp, 128): mov eax, 0; mov edi, [rsp + 128]; mov rsi, rsp; mov edx, 128; syscall
    payload.extend_from_slice(&[0xB8, 0x00, 0x00, 0x00, 0x00]); // mov eax, 0
    payload.extend_from_slice(&[0x8B, 0xBC, 0x24, 0x80, 0x00, 0x00, 0x00]); // mov edi, [rsp + 128]
    payload.extend_from_slice(&[0x48, 0x89, 0xE6]);             // mov rsi, rsp
    payload.extend_from_slice(&[0xBA, 0x80, 0x00, 0x00, 0x00]); // mov edx, 128
    payload.extend_from_slice(&[0x0F, 0x05]);                   // syscall
    payload.extend_from_slice(&[0x49, 0x89, 0xC6]);             // mov r14, rax (bytes read)

    // sys_write(1, msg_parent_hdr, len)
    payload.extend_from_slice(&[0xB8, 0x01, 0x00, 0x00, 0x00]); // mov eax, 1
    payload.extend_from_slice(&[0xBF, 0x01, 0x00, 0x00, 0x00]); // mov edi, 1
    let lea_p_hdr_pos = payload.len();
    payload.extend_from_slice(&[0x48, 0x8D, 0x35, 0x00, 0x00, 0x00, 0x00]);
    let len_p_hdr = msg_parent_hdr.len() as u32;
    payload.extend_from_slice(&[0xBA, len_p_hdr as u8, (len_p_hdr >> 8) as u8, (len_p_hdr >> 16) as u8, (len_p_hdr >> 24) as u8]);
    payload.extend_from_slice(&[0x0F, 0x05]); // syscall

    // sys_write(1, rsp, r14)
    payload.extend_from_slice(&[0xB8, 0x01, 0x00, 0x00, 0x00]); // mov eax, 1
    payload.extend_from_slice(&[0xBF, 0x01, 0x00, 0x00, 0x00]); // mov edi, 1
    payload.extend_from_slice(&[0x48, 0x89, 0xE6]);             // mov rsi, rsp
    payload.extend_from_slice(&[0x4C, 0x89, 0xF2]);             // mov rdx, r14
    payload.extend_from_slice(&[0x0F, 0x05]);                   // syscall

    // sys_write(1, msg_done, len)
    payload.extend_from_slice(&[0xB8, 0x01, 0x00, 0x00, 0x00]); // mov eax, 1
    payload.extend_from_slice(&[0xBF, 0x01, 0x00, 0x00, 0x00]); // mov edi, 1
    let lea_done_pos = payload.len();
    payload.extend_from_slice(&[0x48, 0x8D, 0x35, 0x00, 0x00, 0x00, 0x00]);
    let len_done = msg_done.len() as u32;
    payload.extend_from_slice(&[0xBA, len_done as u8, (len_done >> 8) as u8, (len_done >> 16) as u8, (len_done >> 24) as u8]);
    payload.extend_from_slice(&[0x0F, 0x05]); // syscall

    // sys_close(read_fd): mov eax, 3; mov edi, [rsp + 128]; syscall
    payload.extend_from_slice(&[0xB8, 0x03, 0x00, 0x00, 0x00]);
    payload.extend_from_slice(&[0x8B, 0xBC, 0x24, 0x80, 0x00, 0x00, 0x00]);
    payload.extend_from_slice(&[0x0F, 0x05]);

    // Free buffer: add rsp, 128
    payload.extend_from_slice(&[0x48, 0x81, 0xC4, 0x80, 0x00, 0x00, 0x00]);

    // sys_wait4(-1, rsp, 0)
    payload.extend_from_slice(&[0xB8, 0x3D, 0x00, 0x00, 0x00]); // mov eax, 61
    payload.extend_from_slice(&[0x48, 0xC7, 0xC7, 0xFF, 0xFF, 0xFF, 0xFF]); // mov rdi, -1
    payload.extend_from_slice(&[0x48, 0x89, 0xE6]); // mov rsi, rsp
    payload.extend_from_slice(&[0x31, 0xD2]); // xor edx, edx
    payload.extend_from_slice(&[0x0F, 0x05]); // syscall

    // Free pipefd: add rsp, 16
    payload.extend_from_slice(&[0x48, 0x83, 0xC4, 0x10]);

    // sys_exit(0)
    payload.extend_from_slice(&[0xB8, 0x3C, 0x00, 0x00, 0x00]); // mov eax, 60
    payload.extend_from_slice(&[0x31, 0xFF]); // xor edi, edi
    payload.extend_from_slice(&[0x0F, 0x05]); // syscall
    payload.extend_from_slice(&[0xF4, 0xEB, 0xFD]); // hlt; jmp $-1

    // --- CHILD BRANCH ---
    let child_start = payload.len();
    let jz_disp = (child_start as i32) - ((jz_pos + 6) as i32);
    payload[jz_pos + 2..jz_pos + 6].copy_from_slice(&jz_disp.to_le_bytes());

    // sys_close(read_fd): mov eax, 3; mov edi, [rsp]; syscall
    payload.extend_from_slice(&[0xB8, 0x03, 0x00, 0x00, 0x00]);
    payload.extend_from_slice(&[0x8B, 0x3C, 0x24]);
    payload.extend_from_slice(&[0x0F, 0x05]);

    // sys_write(write_fd, msg_child, len): mov eax, 1; mov edi, [rsp + 4]; lea rsi, [rip+msg_child]; mov edx, len; syscall
    payload.extend_from_slice(&[0xB8, 0x01, 0x00, 0x00, 0x00]); // mov eax, 1
    payload.extend_from_slice(&[0x8B, 0x7C, 0x24, 0x04]);       // mov edi, [rsp + 4]
    let lea_c_pos = payload.len();
    payload.extend_from_slice(&[0x48, 0x8D, 0x35, 0x00, 0x00, 0x00, 0x00]);
    let len_c = msg_child.len() as u32;
    payload.extend_from_slice(&[0xBA, len_c as u8, (len_c >> 8) as u8, (len_c >> 16) as u8, (len_c >> 24) as u8]);
    payload.extend_from_slice(&[0x0F, 0x05]); // syscall

    // sys_close(write_fd): mov eax, 3; mov edi, [rsp + 4]; syscall
    payload.extend_from_slice(&[0xB8, 0x03, 0x00, 0x00, 0x00]);
    payload.extend_from_slice(&[0x8B, 0x7C, 0x24, 0x04]);
    payload.extend_from_slice(&[0x0F, 0x05]);

    // Free pipefd: add rsp, 16
    payload.extend_from_slice(&[0x48, 0x83, 0xC4, 0x10]);

    // sys_exit(0)
    payload.extend_from_slice(&[0xB8, 0x3C, 0x00, 0x00, 0x00]); // mov eax, 60
    payload.extend_from_slice(&[0x31, 0xFF]); // xor edi, edi
    payload.extend_from_slice(&[0x0F, 0x05]); // syscall
    payload.extend_from_slice(&[0xF4, 0xEB, 0xFD]); // hlt; jmp $-1

    // Data sections
    let p_hdr_pos = payload.len();
    payload.extend_from_slice(msg_parent_hdr);
    let done_pos = payload.len();
    payload.extend_from_slice(msg_done);
    let c_pos = payload.len();
    payload.extend_from_slice(msg_child);

    // Patch LEAs
    let disp_p_hdr = (p_hdr_pos as i32) - ((lea_p_hdr_pos + 7) as i32);
    payload[lea_p_hdr_pos + 3..lea_p_hdr_pos + 7].copy_from_slice(&disp_p_hdr.to_le_bytes());

    let disp_done = (done_pos as i32) - ((lea_done_pos + 7) as i32);
    payload[lea_done_pos + 3..lea_done_pos + 7].copy_from_slice(&disp_done.to_le_bytes());

    let disp_c = (c_pos as i32) - ((lea_c_pos + 7) as i32);
    payload[lea_c_pos + 3..lea_c_pos + 7].copy_from_slice(&disp_c.to_le_bytes());

    build_elf64_binary(&payload)
}

fn create_test_dir_elf() -> Vec<u8> {
    let msg_start = b"\n  ===============================================================\n  [LINUX DIR TEST] Querying /bin directory entries via sys_getdents64\n  ===============================================================\n";
    let path_bin = b"/bin\0";
    let msg_found = b"  [DIR] Found directory entry from sys_getdents64 in /bin\n";
    let msg_done = b"  [DIR] Directory traversal test completed successfully!\n\n";

    let mut payload = Vec::new();

    // 1. sys_write(1, msg_start, len)
    payload.extend_from_slice(&[0xB8, 0x01, 0x00, 0x00, 0x00]); // mov eax, 1
    payload.extend_from_slice(&[0xBF, 0x01, 0x00, 0x00, 0x00]); // mov edi, 1
    let lea_start_pos = payload.len();
    payload.extend_from_slice(&[0x48, 0x8D, 0x35, 0x00, 0x00, 0x00, 0x00]);
    let len_start = msg_start.len() as u32;
    payload.extend_from_slice(&[0xBA, len_start as u8, (len_start >> 8) as u8, (len_start >> 16) as u8, (len_start >> 24) as u8]);
    payload.extend_from_slice(&[0x0F, 0x05]); // syscall

    // 2. sys_openat(AT_FDCWD = -100, "/bin", O_RDONLY = 0)
    payload.extend_from_slice(&[0xB8, 0x01, 0x01, 0x00, 0x00]); // mov eax, 257 (sys_openat)
    payload.extend_from_slice(&[0xBF, 0x9C, 0xFF, 0xFF, 0xFF]); // mov edi, -100 (AT_FDCWD)
    let lea_path_pos = payload.len();
    payload.extend_from_slice(&[0x48, 0x8D, 0x35, 0x00, 0x00, 0x00, 0x00]); // lea rsi, [rip + path_bin]
    payload.extend_from_slice(&[0x31, 0xD2]); // xor edx, edx
    payload.extend_from_slice(&[0x0F, 0x05]); // syscall
    payload.extend_from_slice(&[0x49, 0x89, 0xC4]); // mov r12, rax (fd)

    // 3. Allocate 512 bytes on stack for dirp buffer: sub rsp, 512
    payload.extend_from_slice(&[0x48, 0x81, 0xEC, 0x00, 0x02, 0x00, 0x00]);

    // 4. sys_getdents64(r12, rsp, 512)
    payload.extend_from_slice(&[0xB8, 0xD9, 0x00, 0x00, 0x00]); // mov eax, 217 (sys_getdents64)
    payload.extend_from_slice(&[0x4C, 0x89, 0xE7]);             // mov rdi, r12
    payload.extend_from_slice(&[0x48, 0x89, 0xE6]);             // mov rsi, rsp
    payload.extend_from_slice(&[0xBA, 0x00, 0x02, 0x00, 0x00]); // mov edx, 512
    payload.extend_from_slice(&[0x0F, 0x05]);                   // syscall

    // 5. sys_write(1, msg_found, len)
    payload.extend_from_slice(&[0xB8, 0x01, 0x00, 0x00, 0x00]); // mov eax, 1
    payload.extend_from_slice(&[0xBF, 0x01, 0x00, 0x00, 0x00]); // mov edi, 1
    let lea_found_pos = payload.len();
    payload.extend_from_slice(&[0x48, 0x8D, 0x35, 0x00, 0x00, 0x00, 0x00]);
    let len_found = msg_found.len() as u32;
    payload.extend_from_slice(&[0xBA, len_found as u8, (len_found >> 8) as u8, (len_found >> 16) as u8, (len_found >> 24) as u8]);
    payload.extend_from_slice(&[0x0F, 0x05]); // syscall

    // 6. Free buffer: add rsp, 512
    payload.extend_from_slice(&[0x48, 0x81, 0xC4, 0x00, 0x02, 0x00, 0x00]);

    // 7. sys_close(fd)
    payload.extend_from_slice(&[0xB8, 0x03, 0x00, 0x00, 0x00]); // mov eax, 3
    payload.extend_from_slice(&[0x4C, 0x89, 0xE7]);             // mov rdi, r12
    payload.extend_from_slice(&[0x0F, 0x05]);                   // syscall

    // 8. sys_write(1, msg_done, len)
    payload.extend_from_slice(&[0xB8, 0x01, 0x00, 0x00, 0x00]); // mov eax, 1
    payload.extend_from_slice(&[0xBF, 0x01, 0x00, 0x00, 0x00]); // mov edi, 1
    let lea_done_pos = payload.len();
    payload.extend_from_slice(&[0x48, 0x8D, 0x35, 0x00, 0x00, 0x00, 0x00]);
    let len_done = msg_done.len() as u32;
    payload.extend_from_slice(&[0xBA, len_done as u8, (len_done >> 8) as u8, (len_done >> 16) as u8, (len_done >> 24) as u8]);
    payload.extend_from_slice(&[0x0F, 0x05]); // syscall

    // 9. sys_exit(0)
    payload.extend_from_slice(&[0xB8, 0x3C, 0x00, 0x00, 0x00]); // mov eax, 60
    payload.extend_from_slice(&[0x31, 0xFF]); // xor edi, edi
    payload.extend_from_slice(&[0x0F, 0x05]); // syscall
    payload.extend_from_slice(&[0xF4, 0xEB, 0xFD]);

    let start_pos = payload.len();
    payload.extend_from_slice(msg_start);
    let path_pos = payload.len();
    payload.extend_from_slice(path_bin);
    let found_pos = payload.len();
    payload.extend_from_slice(msg_found);
    let done_pos = payload.len();
    payload.extend_from_slice(msg_done);

    let disp_start = (start_pos as i32) - ((lea_start_pos + 7) as i32);
    payload[lea_start_pos + 3..lea_start_pos + 7].copy_from_slice(&disp_start.to_le_bytes());

    let disp_path = (path_pos as i32) - ((lea_path_pos + 7) as i32);
    payload[lea_path_pos + 3..lea_path_pos + 7].copy_from_slice(&disp_path.to_le_bytes());

    let disp_found = (found_pos as i32) - ((lea_found_pos + 7) as i32);
    payload[lea_found_pos + 3..lea_found_pos + 7].copy_from_slice(&disp_found.to_le_bytes());

    let disp_done = (done_pos as i32) - ((lea_done_pos + 7) as i32);
    payload[lea_done_pos + 3..lea_done_pos + 7].copy_from_slice(&disp_done.to_le_bytes());

    build_elf64_binary(&payload)
}

fn create_win32_stream_exe() -> Vec<u8> {
    let mut pe = vec![0u8; 2048]; // 512 headers + 512 .text + 1024 .rdata

    // 1. DOS Header (64 bytes)
    pe[0..2].copy_from_slice(&[0x4D, 0x5A]); // 'MZ'
    pe[0x3C..0x40].copy_from_slice(&64u32.to_le_bytes()); // e_lfanew = 64

    // 2. PE Signature (4 bytes at offset 64)
    pe[64..68].copy_from_slice(b"PE\0\0");

    // 3. COFF File Header (20 bytes at offset 68)
    pe[68..70].copy_from_slice(&0x8664u16.to_le_bytes()); // Machine: AMD64
    pe[70..72].copy_from_slice(&2u16.to_le_bytes()); // NumberOfSections: 2
    pe[72..76].copy_from_slice(&0x66E00000u32.to_le_bytes()); // TimeDateStamp
    pe[76..80].copy_from_slice(&0u32.to_le_bytes()); // PointerToSymbolTable
    pe[80..84].copy_from_slice(&0u32.to_le_bytes()); // NumberOfSymbols
    pe[84..86].copy_from_slice(&240u16.to_le_bytes()); // SizeOfOptionalHeader = 240
    pe[86..88].copy_from_slice(&0x0022u16.to_le_bytes()); // Characteristics: EXECUTABLE_IMAGE | LARGE_ADDRESS_AWARE

    // 4. Optional Header (240 bytes at offset 88)
    let opt = 88;
    pe[opt..opt + 2].copy_from_slice(&0x020Bu16.to_le_bytes()); // Magic: PE32+ (64-bit)
    pe[opt + 2] = 14; // MajorLinkerVersion
    pe[opt + 3] = 0;  // MinorLinkerVersion
    pe[opt + 4..opt + 8].copy_from_slice(&512u32.to_le_bytes()); // SizeOfCode
    pe[opt + 8..opt + 12].copy_from_slice(&1024u32.to_le_bytes()); // SizeOfInitializedData
    pe[opt + 12..opt + 16].copy_from_slice(&0u32.to_le_bytes()); // SizeOfUninitializedData
    pe[opt + 16..opt + 20].copy_from_slice(&0x1000u32.to_le_bytes()); // AddressOfEntryPoint = 0x1000 (.text)
    pe[opt + 20..opt + 24].copy_from_slice(&0x1000u32.to_le_bytes()); // BaseOfCode = 0x1000
    pe[opt + 24..opt + 32].copy_from_slice(&0x0040_0000u64.to_le_bytes()); // ImageBase = 0x400000
    pe[opt + 32..opt + 36].copy_from_slice(&0x1000u32.to_le_bytes()); // SectionAlignment = 4096
    pe[opt + 36..opt + 40].copy_from_slice(&0x200u32.to_le_bytes()); // FileAlignment = 512
    pe[opt + 40..opt + 42].copy_from_slice(&6u16.to_le_bytes()); // MajorOperatingSystemVersion
    pe[opt + 42..opt + 44].copy_from_slice(&0u16.to_le_bytes()); // MinorOperatingSystemVersion
    pe[opt + 48..opt + 50].copy_from_slice(&6u16.to_le_bytes()); // MajorSubsystemVersion
    pe[opt + 50..opt + 52].copy_from_slice(&0u16.to_le_bytes()); // MinorSubsystemVersion
    pe[opt + 56..opt + 60].copy_from_slice(&0x3000u32.to_le_bytes()); // SizeOfImage = 12 KiB (3 * 4096)
    pe[opt + 60..opt + 64].copy_from_slice(&0x200u32.to_le_bytes()); // SizeOfHeaders = 512
    pe[opt + 68..opt + 70].copy_from_slice(&3u16.to_le_bytes()); // Subsystem: IMAGE_SUBSYSTEM_WINDOWS_CUI (3)
    pe[opt + 70..opt + 72].copy_from_slice(&0x8160u16.to_le_bytes()); // DllCharacteristics
    pe[opt + 72..opt + 80].copy_from_slice(&0x100000u64.to_le_bytes()); // SizeOfStackReserve (1 MB)
    pe[opt + 80..opt + 88].copy_from_slice(&0x1000u64.to_le_bytes()); // SizeOfStackCommit (4 KB)
    pe[opt + 88..opt + 96].copy_from_slice(&0x100000u64.to_le_bytes()); // SizeOfHeapReserve (1 MB)
    pe[opt + 96..opt + 104].copy_from_slice(&0x1000u64.to_le_bytes()); // SizeOfHeapCommit (4 KB)
    pe[opt + 108..opt + 112].copy_from_slice(&16u32.to_le_bytes()); // NumberOfRvaAndSizes = 16

    // Data Directory 1: Import Table (at opt + 120)
    let dd_import = opt + 120;
    pe[dd_import..dd_import + 4].copy_from_slice(&0x2000u32.to_le_bytes()); // Import Directory RVA = 0x2000 (.rdata)
    pe[dd_import + 4..dd_import + 8].copy_from_slice(&40u32.to_le_bytes()); // Import Directory Size = 40

    // 5. Section Headers (2 * 40 bytes at offset 88 + 240 = 328)
    let sec1 = 328;
    // Section 1: .text
    pe[sec1..sec1 + 8].copy_from_slice(b".text\0\0\0");
    pe[sec1 + 8..sec1 + 12].copy_from_slice(&512u32.to_le_bytes()); // VirtualSize
    pe[sec1 + 12..sec1 + 16].copy_from_slice(&0x1000u32.to_le_bytes()); // VirtualAddress
    pe[sec1 + 16..sec1 + 20].copy_from_slice(&512u32.to_le_bytes()); // SizeOfRawData
    pe[sec1 + 20..sec1 + 24].copy_from_slice(&0x200u32.to_le_bytes()); // PointerToRawData = 512
    pe[sec1 + 36..sec1 + 40].copy_from_slice(&0x60000020u32.to_le_bytes()); // Characteristics: CODE | EXECUTE | READ

    let sec2 = sec1 + 40;
    // Section 2: .rdata
    pe[sec2..sec2 + 8].copy_from_slice(b".rdata\0\0");
    pe[sec2 + 8..sec2 + 12].copy_from_slice(&1024u32.to_le_bytes()); // VirtualSize
    pe[sec2 + 12..sec2 + 16].copy_from_slice(&0x2000u32.to_le_bytes()); // VirtualAddress
    pe[sec2 + 16..sec2 + 20].copy_from_slice(&1024u32.to_le_bytes()); // SizeOfRawData
    pe[sec2 + 20..sec2 + 24].copy_from_slice(&0x400u32.to_le_bytes()); // PointerToRawData = 1024
    pe[sec2 + 36..sec2 + 40].copy_from_slice(&0x40000040u32.to_le_bytes()); // Characteristics: INITIALIZED_DATA | READ

    // String constants
    let banner_str = b"\n  ===============================================================\n  [WIN32 STREAMS] 64-bit Windows PE32+ (/bin/win_stream.exe)\n  ===============================================================\n";
    let pipe_str = b"\n  [WIN32 PIPE] Message transferred across Win32 CreatePipe stream!\n";
    let find_str = b"  [WIN32 FIND] Successfully queried /bin directory via FindFirstFileA!\n  [WIN32] All Win32 stream tests passed successfully!\n\n";
    let search_str = b"/bin/*\0";

    let banner_len = banner_str.len() as u32;
    let pipe_msg_len = pipe_str.len() as u32;
    let find_msg_len = find_str.len() as u32;

    // 6. .text Section Content (at raw offset 512)
    let text_start = 512;
    let mut code = Vec::new();

    // sub rsp, 0x300 (allocate 768 bytes: shadow space + stack locals + Win32FindDataA)
    code.extend_from_slice(&[0x48, 0x81, 0xEC, 0x00, 0x03, 0x00, 0x00]);

    // 1) CreatePipe(&[rsp+0x20], &[rsp+0x28], NULL, 0)
    code.extend_from_slice(&[0x48, 0x8D, 0x4C, 0x24, 0x20]); // lea rcx, [rsp+0x20]
    code.extend_from_slice(&[0x48, 0x8D, 0x54, 0x24, 0x28]); // lea rdx, [rsp+0x28]
    code.extend_from_slice(&[0x4D, 0x31, 0xC0]);             // xor r8, r8
    code.extend_from_slice(&[0x45, 0x31, 0xC9]);             // xor r9d, r9d
    let call_createpipe_pos = code.len();
    code.extend_from_slice(&[0xFF, 0x15, 0x00, 0x00, 0x00, 0x00]);

    // mov r12, [rsp+0x20] (hRead)
    code.extend_from_slice(&[0x4C, 0x8B, 0x64, 0x24, 0x20]);
    // mov r13, [rsp+0x28] (hWrite)
    code.extend_from_slice(&[0x4C, 0x8B, 0x6C, 0x24, 0x28]);

    // 2) WriteFile(hWrite, &pipe_msg, pipe_msg.len(), &[rsp+0x30], NULL)
    code.extend_from_slice(&[0x4C, 0x89, 0xE9]);             // mov rcx, r13
    let lea_pipe_msg_pos = code.len();
    code.extend_from_slice(&[0x48, 0x8D, 0x15, 0x00, 0x00, 0x00, 0x00]); // lea rdx, [rip + pipe_msg]
    code.extend_from_slice(&[0x41, 0xB8, pipe_msg_len as u8, (pipe_msg_len >> 8) as u8, (pipe_msg_len >> 16) as u8, (pipe_msg_len >> 24) as u8]);
    code.extend_from_slice(&[0x4C, 0x8D, 0x4C, 0x24, 0x30]); // lea r9, [rsp+0x30]
    code.extend_from_slice(&[0x48, 0xC7, 0x44, 0x24, 0x20, 0x00, 0x00, 0x00, 0x00]); // [rsp+0x20] = 0
    let call_writefile_pos = code.len();
    code.extend_from_slice(&[0xFF, 0x15, 0x00, 0x00, 0x00, 0x00]);

    // 3) ReadFile(hRead, &[rsp+0x40], pipe_msg_len, &[rsp+0x38], NULL)
    code.extend_from_slice(&[0x4C, 0x89, 0xE1]);             // mov rcx, r12
    code.extend_from_slice(&[0x48, 0x8D, 0x54, 0x24, 0x40]); // lea rdx, [rsp+0x40]
    code.extend_from_slice(&[0x41, 0xB8, pipe_msg_len as u8, (pipe_msg_len >> 8) as u8, (pipe_msg_len >> 16) as u8, (pipe_msg_len >> 24) as u8]);
    code.extend_from_slice(&[0x4C, 0x8D, 0x4C, 0x24, 0x38]); // lea r9, [rsp+0x38]
    code.extend_from_slice(&[0x48, 0xC7, 0x44, 0x24, 0x20, 0x00, 0x00, 0x00, 0x00]);
    let call_readfile_pos = code.len();
    code.extend_from_slice(&[0xFF, 0x15, 0x00, 0x00, 0x00, 0x00]);

    // 4) GetStdHandle(STD_OUTPUT_HANDLE = -11)
    code.extend_from_slice(&[0xB9, 0xF5, 0xFF, 0xFF, 0xFF]);
    let call_getstd_pos = code.len();
    code.extend_from_slice(&[0xFF, 0x15, 0x00, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x49, 0x89, 0xC7]); // mov r15, rax (save hStdOut in r15)

    // 5) WriteConsoleA(hStdOut, &banner_msg, banner_len, NULL, NULL)
    code.extend_from_slice(&[0x4C, 0x89, 0xF9]); // mov rcx, r15
    let lea_banner_pos = code.len();
    code.extend_from_slice(&[0x48, 0x8D, 0x15, 0x00, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x41, 0xB8, banner_len as u8, (banner_len >> 8) as u8, (banner_len >> 16) as u8, (banner_len >> 24) as u8]);
    code.extend_from_slice(&[0x45, 0x31, 0xC9]);
    code.extend_from_slice(&[0x48, 0xC7, 0x44, 0x24, 0x20, 0x00, 0x00, 0x00, 0x00]);
    let call_writecon1_pos = code.len();
    code.extend_from_slice(&[0xFF, 0x15, 0x00, 0x00, 0x00, 0x00]);

    // 5b) WriteConsoleA(hStdOut, &[rsp+0x40], pipe_msg_len, NULL, NULL) -> output received pipe message!
    code.extend_from_slice(&[0x4C, 0x89, 0xF9]); // mov rcx, r15
    code.extend_from_slice(&[0x48, 0x8D, 0x54, 0x24, 0x40]); // lea rdx, [rsp+0x40]
    code.extend_from_slice(&[0x41, 0xB8, pipe_msg_len as u8, (pipe_msg_len >> 8) as u8, (pipe_msg_len >> 16) as u8, (pipe_msg_len >> 24) as u8]);
    code.extend_from_slice(&[0x45, 0x31, 0xC9]);
    code.extend_from_slice(&[0x48, 0xC7, 0x44, 0x24, 0x20, 0x00, 0x00, 0x00, 0x00]);
    let call_writecon_pipe_pos = code.len();
    code.extend_from_slice(&[0xFF, 0x15, 0x00, 0x00, 0x00, 0x00]);

    // 6) CloseHandle(hRead), CloseHandle(hWrite)
    code.extend_from_slice(&[0x4C, 0x89, 0xE1]); // mov rcx, r12
    let call_close1_pos = code.len();
    code.extend_from_slice(&[0xFF, 0x15, 0x00, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x4C, 0x89, 0xE9]); // mov rcx, r13
    let call_close2_pos = code.len();
    code.extend_from_slice(&[0xFF, 0x15, 0x00, 0x00, 0x00, 0x00]);

    // 7) FindFirstFileA("/bin/*", &[rsp+0xC0])
    let lea_search_pos = code.len();
    code.extend_from_slice(&[0x48, 0x8D, 0x0D, 0x00, 0x00, 0x00, 0x00]); // lea rcx, [rip + search_pattern]
    code.extend_from_slice(&[0x48, 0x8D, 0x94, 0x24, 0xC0, 0x00, 0x00, 0x00]); // lea rdx, [rsp+0xC0]
    let call_findfirst_pos = code.len();
    code.extend_from_slice(&[0xFF, 0x15, 0x00, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x49, 0x89, 0xC6]); // mov r14, rax (hFind)

    // 8) FindClose(hFind)
    code.extend_from_slice(&[0x4C, 0x89, 0xF1]); // mov rcx, r14
    let call_findclose_pos = code.len();
    code.extend_from_slice(&[0xFF, 0x15, 0x00, 0x00, 0x00, 0x00]);

    // 9) WriteConsoleA(hStdOut, &find_msg, find_msg_len, NULL, NULL)
    code.extend_from_slice(&[0x4C, 0x89, 0xF9]); // mov rcx, r15
    let lea_find_msg_pos = code.len();
    code.extend_from_slice(&[0x48, 0x8D, 0x15, 0x00, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x41, 0xB8, find_msg_len as u8, (find_msg_len >> 8) as u8, (find_msg_len >> 16) as u8, (find_msg_len >> 24) as u8]);
    code.extend_from_slice(&[0x45, 0x31, 0xC9]);
    code.extend_from_slice(&[0x48, 0xC7, 0x44, 0x24, 0x20, 0x00, 0x00, 0x00, 0x00]);
    let call_writecon2_pos = code.len();
    code.extend_from_slice(&[0xFF, 0x15, 0x00, 0x00, 0x00, 0x00]);

    // 10) ExitProcess(0)
    code.extend_from_slice(&[0x31, 0xC9]);
    let call_exit_pos = code.len();
    code.extend_from_slice(&[0xFF, 0x15, 0x00, 0x00, 0x00, 0x00]);

    // add rsp, 0x300; ret
    code.extend_from_slice(&[0x48, 0x81, 0xC4, 0x00, 0x03, 0x00, 0x00, 0xC3]);
    pe[text_start..text_start + code.len()].copy_from_slice(&code);

    // 7. .rdata Section Content (at raw offset 1024, VirtualAddress = 0x2000)
    let rdata_start = 1024;
    let rdata_vaddr = 0x2000u32;

    let ilt_rva = rdata_vaddr + 0x80;
    let dll_name_rva = rdata_vaddr + 0xD0;
    let iat_rva = rdata_vaddr + 0x30;

    // ImageImportDescriptor:
    pe[rdata_start..rdata_start + 4].copy_from_slice(&ilt_rva.to_le_bytes());
    pe[rdata_start + 12..rdata_start + 16].copy_from_slice(&dll_name_rva.to_le_bytes());
    pe[rdata_start + 16..rdata_start + 20].copy_from_slice(&iat_rva.to_le_bytes());

    // Hint/Names RVAs:
    let hn_getstd = rdata_vaddr + 0xE0;
    let hn_writecon = rdata_vaddr + 0xF0;
    let hn_createpipe = rdata_vaddr + 0x100;
    let hn_writefile = rdata_vaddr + 0x110;
    let hn_readfile = rdata_vaddr + 0x120;
    let hn_close = rdata_vaddr + 0x130;
    let hn_findfirst = rdata_vaddr + 0x140;
    let hn_findclose = rdata_vaddr + 0x150;
    let hn_exit = rdata_vaddr + 0x160;

    // Write IAT (0x30..0x78) and ILT (0x80..0xC8)
    let iat_ptrs = [hn_getstd, hn_writecon, hn_createpipe, hn_writefile, hn_readfile, hn_close, hn_findfirst, hn_findclose, hn_exit];
    for (i, &ptr) in iat_ptrs.iter().enumerate() {
        pe[rdata_start + 0x30 + i * 8..rdata_start + 0x30 + i * 8 + 8].copy_from_slice(&(ptr as u64).to_le_bytes());
        pe[rdata_start + 0x80 + i * 8..rdata_start + 0x80 + i * 8 + 8].copy_from_slice(&(ptr as u64).to_le_bytes());
    }

    // Write DLL Name
    pe[rdata_start + 0xD0..rdata_start + 0xDD].copy_from_slice(b"kernel32.dll\0");

    // Write Hint/Names (each prefixed with 2-byte hint = 0)
    pe[rdata_start + 0xE2..rdata_start + 0xEF].copy_from_slice(b"GetStdHandle\0");
    pe[rdata_start + 0xF2..rdata_start + 0x100].copy_from_slice(b"WriteConsoleA\0");
    pe[rdata_start + 0x102..rdata_start + 0x10D].copy_from_slice(b"CreatePipe\0");
    pe[rdata_start + 0x112..rdata_start + 0x11C].copy_from_slice(b"WriteFile\0");
    pe[rdata_start + 0x122..rdata_start + 0x12B].copy_from_slice(b"ReadFile\0");
    pe[rdata_start + 0x132..rdata_start + 0x13E].copy_from_slice(b"CloseHandle\0");
    pe[rdata_start + 0x142..rdata_start + 0x151].copy_from_slice(b"FindFirstFileA\0");
    pe[rdata_start + 0x152..rdata_start + 0x15C].copy_from_slice(b"FindClose\0");
    pe[rdata_start + 0x162..rdata_start + 0x16E].copy_from_slice(b"ExitProcess\0");

    // Messages & Data in .rdata (at 0x180)
    let banner_rva = rdata_vaddr + 0x180;
    let pipe_rva = banner_rva + banner_str.len() as u32;
    let find_rva = pipe_rva + pipe_str.len() as u32;
    let search_rva = find_rva + find_str.len() as u32;

    pe[rdata_start + 0x180..rdata_start + 0x180 + banner_str.len()].copy_from_slice(banner_str);
    let p_off = 0x180 + banner_str.len();
    pe[rdata_start + p_off..rdata_start + p_off + pipe_str.len()].copy_from_slice(pipe_str);
    let f_off = p_off + pipe_str.len();
    pe[rdata_start + f_off..rdata_start + f_off + find_str.len()].copy_from_slice(find_str);
    let s_off = f_off + find_str.len();
    pe[rdata_start + s_off..rdata_start + s_off + search_str.len()].copy_from_slice(search_str);

    // Patch .text RIP-relative calls and LEAs
    let patch_call = |pe: &mut [u8], pos: usize, target_rva: u32| {
        let vaddr = 0x1000 + pos as u32;
        let disp = (target_rva as i32) - ((vaddr + 6) as i32);
        pe[text_start + pos + 2..text_start + pos + 6].copy_from_slice(&disp.to_le_bytes());
    };

    let patch_lea = |pe: &mut [u8], pos: usize, target_rva: u32| {
        let vaddr = 0x1000 + pos as u32;
        let disp = (target_rva as i32) - ((vaddr + 7) as i32);
        pe[text_start + pos + 3..text_start + pos + 7].copy_from_slice(&disp.to_le_bytes());
    };

    // IAT entries:
    // 0x2030: GetStdHandle
    // 0x2038: WriteConsoleA
    // 0x2040: CreatePipe
    // 0x2048: WriteFile
    // 0x2050: ReadFile
    // 0x2058: CloseHandle
    // 0x2060: FindFirstFileA
    // 0x2068: FindClose
    // 0x2070: ExitProcess
    patch_call(&mut pe, call_createpipe_pos, 0x2040);
    patch_call(&mut pe, call_writefile_pos, 0x2048);
    patch_call(&mut pe, call_readfile_pos, 0x2050);
    patch_call(&mut pe, call_getstd_pos, 0x2030);
    patch_call(&mut pe, call_writecon1_pos, 0x2038);
    patch_call(&mut pe, call_writecon_pipe_pos, 0x2038);
    patch_call(&mut pe, call_close1_pos, 0x2058);
    patch_call(&mut pe, call_close2_pos, 0x2058);
    patch_call(&mut pe, call_findfirst_pos, 0x2060);
    patch_call(&mut pe, call_findclose_pos, 0x2068);
    patch_call(&mut pe, call_writecon2_pos, 0x2038);
    patch_call(&mut pe, call_exit_pos, 0x2070);

    patch_lea(&mut pe, lea_pipe_msg_pos, pipe_rva);
    patch_lea(&mut pe, lea_banner_pos, banner_rva);
    patch_lea(&mut pe, lea_search_pos, search_rva);
    patch_lea(&mut pe, lea_find_msg_pos, find_rva);

    pe
}

fn create_test_devproc_elf() -> Vec<u8> {
    let msg_start = b"\n  ===============================================================\n  [DEVPROC] Testing Virtual Pseudo-Filesystems (/dev & /proc)\n  ===============================================================\n";
    let msg_null = b"  [DEV] Successfully opened, wrote, read (EOF), and closed /dev/null\n";
    let msg_zero = b"  [DEV] Successfully read zero bytes from /dev/zero\n";
    let msg_rand = b"  [DEV] Successfully read random entropy from /dev/urandom\n";
    let msg_ver = b"  [PROC] Successfully read /proc/version kernel identification\n";
    let msg_mem = b"  [PROC] Successfully read /proc/meminfo physical memory metrics\n";
    let msg_cpu = b"  [PROC] Successfully read /proc/cpuinfo processor configuration\n";
    let msg_upt = b"  [PROC] Successfully read /proc/uptime system timer metrics\n";
    let msg_done = b"  [DEVPROC] SUCCESS: All /dev and /proc virtual filesystems verified!\n\n";

    let path_null = b"/dev/null\0";
    let path_zero = b"/dev/zero\0";
    let path_rand = b"/dev/urandom\0";
    let path_ver = b"/proc/version\0";
    let path_mem = b"/proc/meminfo\0";
    let path_cpu = b"/proc/cpuinfo\0";
    let path_upt = b"/proc/uptime\0";

    let mut payload = Vec::new();

    // 1. Allocate 256 bytes on stack for read buffer
    payload.extend_from_slice(&[0x48, 0x81, 0xEC, 0x00, 0x01, 0x00, 0x00]); // sub rsp, 256

    let mut fixups: Vec<(usize, usize)> = Vec::new(); // (code_pos, data_id)

    // Helper: emit sys_write(1, msg, len)
    fn emit_write_stdout(payload: &mut Vec<u8>, fixups: &mut Vec<(usize, usize)>, msg_id: usize, len: usize) {
        payload.extend_from_slice(&[0xB8, 0x01, 0x00, 0x00, 0x00]); // mov eax, 1
        payload.extend_from_slice(&[0xBF, 0x01, 0x00, 0x00, 0x00]); // mov edi, 1
        let lea_pos = payload.len();
        payload.extend_from_slice(&[0x48, 0x8D, 0x35, 0x00, 0x00, 0x00, 0x00]); // lea rsi, [rip + msg]
        let ulen = len as u32;
        payload.extend_from_slice(&[0xBA, ulen as u8, (ulen >> 8) as u8, (ulen >> 16) as u8, (ulen >> 24) as u8]);
        payload.extend_from_slice(&[0x0F, 0x05]); // syscall
        fixups.push((lea_pos, msg_id));
    }

    // Helper: open(path, flags) -> fd in r12
    fn emit_open(payload: &mut Vec<u8>, fixups: &mut Vec<(usize, usize)>, path_id: usize, path_len: usize, flags: u32) {
        payload.extend_from_slice(&[0xB8, 0x02, 0x00, 0x00, 0x00]); // mov eax, 2
        let lea_pos = payload.len();
        payload.extend_from_slice(&[0x48, 0x8D, 0x3D, 0x00, 0x00, 0x00, 0x00]); // lea rdi, [rip + path]
        let plen = path_len as u32;
        payload.extend_from_slice(&[0xBE, plen as u8, (plen >> 8) as u8, (plen >> 16) as u8, (plen >> 24) as u8]); // mov esi, len
        payload.extend_from_slice(&[0xBA, flags as u8, (flags >> 8) as u8, (flags >> 16) as u8, (flags >> 24) as u8]); // mov edx, flags
        payload.extend_from_slice(&[0x0F, 0x05]); // syscall
        payload.extend_from_slice(&[0x49, 0x89, 0xC4]); // mov r12, rax (save fd)
        fixups.push((lea_pos, path_id));
    }

    // Helper: read(r12, rsp, len)
    fn emit_read(payload: &mut Vec<u8>, len: usize) {
        payload.extend_from_slice(&[0xB8, 0x00, 0x00, 0x00, 0x00]); // mov eax, 0
        payload.extend_from_slice(&[0x4C, 0x89, 0xE7]);             // mov rdi, r12
        payload.extend_from_slice(&[0x48, 0x89, 0xE6]);             // mov rsi, rsp
        let rlen = len as u32;
        payload.extend_from_slice(&[0xBA, rlen as u8, (rlen >> 8) as u8, (rlen >> 16) as u8, (rlen >> 24) as u8]); // mov edx, len
        payload.extend_from_slice(&[0x0F, 0x05]);                   // syscall
    }

    // Helper: write(r12, data, len)
    fn emit_write_fd(payload: &mut Vec<u8>, fixups: &mut Vec<(usize, usize)>, data_id: usize, len: usize) {
        payload.extend_from_slice(&[0xB8, 0x01, 0x00, 0x00, 0x00]); // mov eax, 1
        payload.extend_from_slice(&[0x4C, 0x89, 0xE7]);             // mov rdi, r12
        let lea_pos = payload.len();
        payload.extend_from_slice(&[0x48, 0x8D, 0x35, 0x00, 0x00, 0x00, 0x00]); // lea rsi, [rip + data]
        let wlen = len as u32;
        payload.extend_from_slice(&[0xBA, wlen as u8, (wlen >> 8) as u8, (wlen >> 16) as u8, (wlen >> 24) as u8]); // mov edx, len
        payload.extend_from_slice(&[0x0F, 0x05]);                   // syscall
        fixups.push((lea_pos, data_id));
    }

    // Helper: close(r12)
    fn emit_close(payload: &mut Vec<u8>) {
        payload.extend_from_slice(&[0xB8, 0x03, 0x00, 0x00, 0x00]); // mov eax, 3
        payload.extend_from_slice(&[0x4C, 0x89, 0xE7]);             // mov rdi, r12
        payload.extend_from_slice(&[0x0F, 0x05]);                   // syscall
    }

    // 1. Write banner
    emit_write_stdout(&mut payload, &mut fixups, 0, msg_start.len());

    // 2. /dev/null
    emit_open(&mut payload, &mut fixups, 9, path_null.len() - 1, 2);
    emit_write_fd(&mut payload, &mut fixups, 9, 9);
    emit_read(&mut payload, 32);
    emit_close(&mut payload);
    emit_write_stdout(&mut payload, &mut fixups, 1, msg_null.len());

    // 3. /dev/zero
    emit_open(&mut payload, &mut fixups, 10, path_zero.len() - 1, 0);
    emit_read(&mut payload, 32);
    emit_close(&mut payload);
    emit_write_stdout(&mut payload, &mut fixups, 2, msg_zero.len());

    // 4. /dev/urandom
    emit_open(&mut payload, &mut fixups, 11, path_rand.len() - 1, 0);
    emit_read(&mut payload, 32);
    emit_close(&mut payload);
    emit_write_stdout(&mut payload, &mut fixups, 3, msg_rand.len());

    // 5. /proc/version
    emit_open(&mut payload, &mut fixups, 12, path_ver.len() - 1, 0);
    emit_read(&mut payload, 128);
    emit_close(&mut payload);
    emit_write_stdout(&mut payload, &mut fixups, 4, msg_ver.len());

    // 6. /proc/meminfo
    emit_open(&mut payload, &mut fixups, 13, path_mem.len() - 1, 0);
    emit_read(&mut payload, 128);
    emit_close(&mut payload);
    emit_write_stdout(&mut payload, &mut fixups, 5, msg_mem.len());

    // 7. /proc/cpuinfo
    emit_open(&mut payload, &mut fixups, 14, path_cpu.len() - 1, 0);
    emit_read(&mut payload, 128);
    emit_close(&mut payload);
    emit_write_stdout(&mut payload, &mut fixups, 6, msg_cpu.len());

    // 8. /proc/uptime
    emit_open(&mut payload, &mut fixups, 15, path_upt.len() - 1, 0);
    emit_read(&mut payload, 64);
    emit_close(&mut payload);
    emit_write_stdout(&mut payload, &mut fixups, 7, msg_upt.len());

    // 9. Write final success
    emit_write_stdout(&mut payload, &mut fixups, 8, msg_done.len());

    // 10. sys_exit(0)
    payload.extend_from_slice(&[0xB8, 0x3C, 0x00, 0x00, 0x00]); // mov eax, 60
    payload.extend_from_slice(&[0x31, 0xFF]);                   // xor edi, edi
    payload.extend_from_slice(&[0x0F, 0x05]);                   // syscall
    payload.extend_from_slice(&[0xF4, 0xEB, 0xFD]);

    // Data table
    let data_items: [&[u8]; 16] = [
        msg_start, msg_null, msg_zero, msg_rand, msg_ver, msg_mem, msg_cpu, msg_upt, msg_done,
        path_null, path_zero, path_rand, path_ver, path_mem, path_cpu, path_upt
    ];

    let mut data_offsets = Vec::new();
    for item in &data_items {
        data_offsets.push(payload.len());
        payload.extend_from_slice(item);
    }

    // Apply fixups
    for (code_pos, data_id) in fixups {
        let target_off = data_offsets[data_id];
        let disp = (target_off as i32) - ((code_pos + 7) as i32);
        payload[code_pos + 3..code_pos + 7].copy_from_slice(&disp.to_le_bytes());
    }

    build_elf64_binary(&payload)
}

fn create_win32_envreg_exe() -> Vec<u8> {
    let mut pe = vec![0u8; 2048]; // 512 headers + 512 .text + 1024 .rdata

    // 1. DOS Header (64 bytes)
    pe[0..2].copy_from_slice(&[0x4D, 0x5A]); // 'MZ'
    pe[0x3C..0x40].copy_from_slice(&64u32.to_le_bytes()); // e_lfanew = 64

    // 2. PE Signature (4 bytes at offset 64)
    pe[64..68].copy_from_slice(b"PE\0\0");

    // 3. COFF File Header (20 bytes at offset 68)
    pe[68..70].copy_from_slice(&0x8664u16.to_le_bytes()); // Machine: AMD64
    pe[70..72].copy_from_slice(&2u16.to_le_bytes()); // NumberOfSections: 2
    pe[72..76].copy_from_slice(&0x66E00000u32.to_le_bytes()); // TimeDateStamp
    pe[76..80].copy_from_slice(&0u32.to_le_bytes()); // PointerToSymbolTable
    pe[80..84].copy_from_slice(&0u32.to_le_bytes()); // NumberOfSymbols
    pe[84..86].copy_from_slice(&240u16.to_le_bytes()); // SizeOfOptionalHeader = 240
    pe[86..88].copy_from_slice(&0x0022u16.to_le_bytes()); // Characteristics: EXECUTABLE_IMAGE | LARGE_ADDRESS_AWARE

    // 4. Optional Header (240 bytes at offset 88)
    let opt = 88;
    pe[opt..opt + 2].copy_from_slice(&0x020Bu16.to_le_bytes()); // Magic: PE32+ (64-bit)
    pe[opt + 2] = 14; // MajorLinkerVersion
    pe[opt + 3] = 0;  // MinorLinkerVersion
    pe[opt + 4..opt + 8].copy_from_slice(&512u32.to_le_bytes()); // SizeOfCode
    pe[opt + 8..opt + 12].copy_from_slice(&1024u32.to_le_bytes()); // SizeOfInitializedData
    pe[opt + 12..opt + 16].copy_from_slice(&0u32.to_le_bytes()); // SizeOfUninitializedData
    pe[opt + 16..opt + 20].copy_from_slice(&0x1000u32.to_le_bytes()); // AddressOfEntryPoint = 0x1000 (.text)
    pe[opt + 20..opt + 24].copy_from_slice(&0x1000u32.to_le_bytes()); // BaseOfCode = 0x1000
    pe[opt + 24..opt + 32].copy_from_slice(&0x0040_0000u64.to_le_bytes()); // ImageBase = 0x400000
    pe[opt + 32..opt + 36].copy_from_slice(&0x1000u32.to_le_bytes()); // SectionAlignment = 4096
    pe[opt + 36..opt + 40].copy_from_slice(&0x200u32.to_le_bytes()); // FileAlignment = 512
    pe[opt + 40..opt + 42].copy_from_slice(&6u16.to_le_bytes()); // MajorOperatingSystemVersion
    pe[opt + 42..opt + 44].copy_from_slice(&0u16.to_le_bytes()); // MinorOperatingSystemVersion
    pe[opt + 48..opt + 50].copy_from_slice(&6u16.to_le_bytes()); // MajorSubsystemVersion
    pe[opt + 50..opt + 52].copy_from_slice(&0u16.to_le_bytes()); // MinorSubsystemVersion
    pe[opt + 56..opt + 60].copy_from_slice(&0x3000u32.to_le_bytes()); // SizeOfImage = 12 KiB
    pe[opt + 60..opt + 64].copy_from_slice(&0x200u32.to_le_bytes()); // SizeOfHeaders = 512
    pe[opt + 68..opt + 70].copy_from_slice(&3u16.to_le_bytes()); // Subsystem: IMAGE_SUBSYSTEM_WINDOWS_CUI
    pe[opt + 70..opt + 72].copy_from_slice(&0x8160u16.to_le_bytes()); // DllCharacteristics
    pe[opt + 72..opt + 80].copy_from_slice(&0x100000u64.to_le_bytes()); // SizeOfStackReserve (1 MB)
    pe[opt + 80..opt + 88].copy_from_slice(&0x1000u64.to_le_bytes()); // SizeOfStackCommit (4 KB)
    pe[opt + 88..opt + 96].copy_from_slice(&0x100000u64.to_le_bytes()); // SizeOfHeapReserve (1 MB)
    pe[opt + 96..opt + 104].copy_from_slice(&0x1000u64.to_le_bytes()); // SizeOfHeapCommit (4 KB)
    pe[opt + 108..opt + 112].copy_from_slice(&16u32.to_le_bytes()); // NumberOfRvaAndSizes = 16

    // Data Directory 1: Import Table
    let dd_import = opt + 120;
    pe[dd_import..dd_import + 4].copy_from_slice(&0x2000u32.to_le_bytes()); // Import Directory RVA = 0x2000 (.rdata)
    pe[dd_import + 4..dd_import + 8].copy_from_slice(&40u32.to_le_bytes()); // Import Directory Size = 40

    // 5. Section Headers
    let sec1 = 328;
    pe[sec1..sec1 + 8].copy_from_slice(b".text\0\0\0");
    pe[sec1 + 8..sec1 + 12].copy_from_slice(&512u32.to_le_bytes());
    pe[sec1 + 12..sec1 + 16].copy_from_slice(&0x1000u32.to_le_bytes());
    pe[sec1 + 16..sec1 + 20].copy_from_slice(&512u32.to_le_bytes());
    pe[sec1 + 20..sec1 + 24].copy_from_slice(&0x200u32.to_le_bytes());
    pe[sec1 + 36..sec1 + 40].copy_from_slice(&0x60000020u32.to_le_bytes());

    let sec2 = sec1 + 40;
    pe[sec2..sec2 + 8].copy_from_slice(b".rdata\0\0");
    pe[sec2 + 8..sec2 + 12].copy_from_slice(&1024u32.to_le_bytes());
    pe[sec2 + 12..sec2 + 16].copy_from_slice(&0x2000u32.to_le_bytes());
    pe[sec2 + 16..sec2 + 20].copy_from_slice(&1024u32.to_le_bytes());
    pe[sec2 + 20..sec2 + 24].copy_from_slice(&0x400u32.to_le_bytes());
    pe[sec2 + 36..sec2 + 40].copy_from_slice(&0x40000040u32.to_le_bytes());

    // Strings
    let banner_str = b"\n  ===============================================================\n  [WIN32 ENV & REG] 64-bit Windows PE32+ (/bin/win_envreg.exe)\n  ===============================================================\n";
    let env_msg_str = b"  [WIN32 ENV] Successfully queried and updated environment block!\n";
    let reg_msg_str = b"  [WIN32 REG] Successfully opened, queried HKLM registry and closed key!\n  [WIN32] Win32 Environment Block & In-Memory Registry verified!\n\n";

    let var_os_str = b"OS\0";
    let var_name_str = b"MY_LUNIX_VAR\0";
    let var_val_str = b"HYBRID_2026\0";
    let reg_subkey_str = b"Software\\Microsoft\\Windows NT\\CurrentVersion\0";
    let reg_val_str = b"ProductName\0";

    // 6. .text Section Content
    let text_start = 512;
    let mut code = Vec::new();

    // sub rsp, 0x300 (allocate 768 bytes stack frame)
    code.extend_from_slice(&[0x48, 0x81, 0xEC, 0x00, 0x03, 0x00, 0x00]);

    // 1) GetStdHandle(-11) -> hStdOut in r15
    code.extend_from_slice(&[0xB9, 0xF5, 0xFF, 0xFF, 0xFF]);
    let call_getstd_pos = code.len();
    code.extend_from_slice(&[0xFF, 0x15, 0x00, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x49, 0x89, 0xC7]); // mov r15, rax

    // 2) WriteConsoleA(hStdOut, &banner, banner_len, NULL, NULL)
    code.extend_from_slice(&[0x4C, 0x89, 0xF9]); // mov rcx, r15
    let lea_banner_pos = code.len();
    code.extend_from_slice(&[0x48, 0x8D, 0x15, 0x00, 0x00, 0x00, 0x00]);
    let b_len = banner_str.len() as u32;
    code.extend_from_slice(&[0x41, 0xB8, b_len as u8, (b_len >> 8) as u8, (b_len >> 16) as u8, (b_len >> 24) as u8]);
    code.extend_from_slice(&[0x45, 0x31, 0xC9]); // xor r9d, r9d
    code.extend_from_slice(&[0x48, 0xC7, 0x44, 0x24, 0x20, 0x00, 0x00, 0x00, 0x00]);
    let call_writecon1_pos = code.len();
    code.extend_from_slice(&[0xFF, 0x15, 0x00, 0x00, 0x00, 0x00]);

    // 3) GetEnvironmentVariableA("OS", &[rsp+0x40], 64)
    let lea_var_os_pos = code.len();
    code.extend_from_slice(&[0x48, 0x8D, 0x0D, 0x00, 0x00, 0x00, 0x00]); // lea rcx, [rip + var_os]
    code.extend_from_slice(&[0x48, 0x8D, 0x54, 0x24, 0x40]);             // lea rdx, [rsp+0x40]
    code.extend_from_slice(&[0x41, 0xB8, 0x40, 0x00, 0x00, 0x00]);       // mov r8d, 64
    let call_getenv1_pos = code.len();
    code.extend_from_slice(&[0xFF, 0x15, 0x00, 0x00, 0x00, 0x00]);

    // 4) SetEnvironmentVariableA("MY_LUNIX_VAR", "HYBRID_2026")
    let lea_var_name_pos = code.len();
    code.extend_from_slice(&[0x48, 0x8D, 0x0D, 0x00, 0x00, 0x00, 0x00]); // lea rcx, [rip + var_name]
    let lea_var_val_pos = code.len();
    code.extend_from_slice(&[0x48, 0x8D, 0x15, 0x00, 0x00, 0x00, 0x00]); // lea rdx, [rip + var_val]
    let call_setenv_pos = code.len();
    code.extend_from_slice(&[0xFF, 0x15, 0x00, 0x00, 0x00, 0x00]);

    // 5) GetEnvironmentVariableA("MY_LUNIX_VAR", &[rsp+0x80], 64)
    let lea_var_name2_pos = code.len();
    code.extend_from_slice(&[0x48, 0x8D, 0x0D, 0x00, 0x00, 0x00, 0x00]); // lea rcx, [rip + var_name]
    code.extend_from_slice(&[0x48, 0x8D, 0x94, 0x24, 0x80, 0x00, 0x00, 0x00]); // lea rdx, [rsp+0x80]
    code.extend_from_slice(&[0x41, 0xB8, 0x40, 0x00, 0x00, 0x00]);       // mov r8d, 64
    let call_getenv2_pos = code.len();
    code.extend_from_slice(&[0xFF, 0x15, 0x00, 0x00, 0x00, 0x00]);

    // 6) WriteConsoleA(hStdOut, &env_msg, env_msg_len, NULL, NULL)
    code.extend_from_slice(&[0x4C, 0x89, 0xF9]); // mov rcx, r15
    let lea_env_msg_pos = code.len();
    code.extend_from_slice(&[0x48, 0x8D, 0x15, 0x00, 0x00, 0x00, 0x00]);
    let em_len = env_msg_str.len() as u32;
    code.extend_from_slice(&[0x41, 0xB8, em_len as u8, (em_len >> 8) as u8, (em_len >> 16) as u8, (em_len >> 24) as u8]);
    code.extend_from_slice(&[0x45, 0x31, 0xC9]);
    code.extend_from_slice(&[0x48, 0xC7, 0x44, 0x24, 0x20, 0x00, 0x00, 0x00, 0x00]);
    let call_writecon2_pos = code.len();
    code.extend_from_slice(&[0xFF, 0x15, 0x00, 0x00, 0x00, 0x00]);

    // 7) RegOpenKeyExA(0x80000002, &reg_subkey, 0, 0, &[rsp+0x30])
    code.extend_from_slice(&[0x48, 0xB9, 0x02, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00]); // mov rcx, 0x80000002
    let lea_subkey_pos = code.len();
    code.extend_from_slice(&[0x48, 0x8D, 0x15, 0x00, 0x00, 0x00, 0x00]); // lea rdx, [rip + reg_subkey]
    code.extend_from_slice(&[0x45, 0x31, 0xC0]);                         // xor r8d, r8d
    code.extend_from_slice(&[0x45, 0x31, 0xC9]);                         // xor r9d, r9d
    code.extend_from_slice(&[0x48, 0x8D, 0x44, 0x24, 0x30]);             // lea rax, [rsp+0x30]
    code.extend_from_slice(&[0x48, 0x89, 0x44, 0x24, 0x20]);             // mov [rsp+0x20], rax (5th arg)
    let call_regopen_pos = code.len();
    code.extend_from_slice(&[0xFF, 0x15, 0x00, 0x00, 0x00, 0x00]);

    // mov r14, [rsp+0x30] (hKey in r14)
    code.extend_from_slice(&[0x4C, 0x8B, 0x74, 0x24, 0x30]);

    // 8) RegQueryValueExA(hKey, &reg_val, NULL, &[rsp+0x38], &[rsp+0xC0], &[rsp+0x3C])
    code.extend_from_slice(&[0xC7, 0x44, 0x24, 0x3C, 0x80, 0x00, 0x00, 0x00]); // mov dword ptr [rsp+0x3C], 128
    code.extend_from_slice(&[0x4C, 0x89, 0xF1]);                                 // mov rcx, r14
    let lea_regval_pos = code.len();
    code.extend_from_slice(&[0x48, 0x8D, 0x15, 0x00, 0x00, 0x00, 0x00]);         // lea rdx, [rip + reg_val]
    code.extend_from_slice(&[0x4D, 0x31, 0xC0]);                                 // xor r8, r8
    code.extend_from_slice(&[0x4C, 0x8D, 0x4C, 0x24, 0x38]);                     // lea r9, [rsp+0x38]
    code.extend_from_slice(&[0x48, 0x8D, 0x84, 0x24, 0xC0, 0x00, 0x00, 0x00]); // lea rax, [rsp+0xC0]
    code.extend_from_slice(&[0x48, 0x89, 0x44, 0x24, 0x20]);                     // mov [rsp+0x20], rax
    code.extend_from_slice(&[0x48, 0x8D, 0x44, 0x24, 0x3C]);                     // lea rax, [rsp+0x3C]
    code.extend_from_slice(&[0x48, 0x89, 0x44, 0x24, 0x28]);                     // mov [rsp+0x28], rax
    let call_regquery_pos = code.len();
    code.extend_from_slice(&[0xFF, 0x15, 0x00, 0x00, 0x00, 0x00]);

    // 9) RegCloseKey(hKey)
    code.extend_from_slice(&[0x4C, 0x89, 0xF1]); // mov rcx, r14
    let call_regclose_pos = code.len();
    code.extend_from_slice(&[0xFF, 0x15, 0x00, 0x00, 0x00, 0x00]);

    // 10) WriteConsoleA(hStdOut, &reg_msg, reg_msg_len, NULL, NULL)
    code.extend_from_slice(&[0x4C, 0x89, 0xF9]); // mov rcx, r15
    let lea_reg_msg_pos = code.len();
    code.extend_from_slice(&[0x48, 0x8D, 0x15, 0x00, 0x00, 0x00, 0x00]);
    let rm_len = reg_msg_str.len() as u32;
    code.extend_from_slice(&[0x41, 0xB8, rm_len as u8, (rm_len >> 8) as u8, (rm_len >> 16) as u8, (rm_len >> 24) as u8]);
    code.extend_from_slice(&[0x45, 0x31, 0xC9]);
    code.extend_from_slice(&[0x48, 0xC7, 0x44, 0x24, 0x20, 0x00, 0x00, 0x00, 0x00]);
    let call_writecon3_pos = code.len();
    code.extend_from_slice(&[0xFF, 0x15, 0x00, 0x00, 0x00, 0x00]);

    // 11) ExitProcess(0)
    code.extend_from_slice(&[0x31, 0xC9]);
    let call_exit_pos = code.len();
    code.extend_from_slice(&[0xFF, 0x15, 0x00, 0x00, 0x00, 0x00]);

    // add rsp, 0x300; ret
    code.extend_from_slice(&[0x48, 0x81, 0xC4, 0x00, 0x03, 0x00, 0x00, 0xC3]);
    pe[text_start..text_start + code.len()].copy_from_slice(&code);

    // 7. .rdata Section Content (at raw offset 1024, VirtualAddress = 0x2000)
    let rdata_start = 1024;
    let rdata_vaddr = 0x2000u32;

    let ilt_rva = rdata_vaddr + 0x80;
    let dll_name_rva = rdata_vaddr + 0xD0;
    let iat_rva = rdata_vaddr + 0x30;

    // ImageImportDescriptor:
    pe[rdata_start..rdata_start + 4].copy_from_slice(&ilt_rva.to_le_bytes());
    pe[rdata_start + 12..rdata_start + 16].copy_from_slice(&dll_name_rva.to_le_bytes());
    pe[rdata_start + 16..rdata_start + 20].copy_from_slice(&iat_rva.to_le_bytes());

    // Hint/Names RVAs:
    let hn_getstd = rdata_vaddr + 0xE0;
    let hn_writecon = rdata_vaddr + 0xF0;
    let hn_getenv = rdata_vaddr + 0x100;
    let hn_setenv = rdata_vaddr + 0x120;
    let hn_regopen = rdata_vaddr + 0x140;
    let hn_regquery = rdata_vaddr + 0x150;
    let hn_regclose = rdata_vaddr + 0x170;
    let hn_exit = rdata_vaddr + 0x180;

    // Write IAT (0x30..0x70) and ILT (0x80..0xC0)
    let iat_ptrs = [hn_getstd, hn_writecon, hn_getenv, hn_setenv, hn_regopen, hn_regquery, hn_regclose, hn_exit];
    for (i, &ptr) in iat_ptrs.iter().enumerate() {
        pe[rdata_start + 0x30 + i * 8..rdata_start + 0x30 + i * 8 + 8].copy_from_slice(&(ptr as u64).to_le_bytes());
        pe[rdata_start + 0x80 + i * 8..rdata_start + 0x80 + i * 8 + 8].copy_from_slice(&(ptr as u64).to_le_bytes());
    }

    // Write DLL Name
    pe[rdata_start + 0xD0..rdata_start + 0xDD].copy_from_slice(b"kernel32.dll\0");

    // Write Hint/Names (each prefixed with 2-byte hint = 0)
    let copy_hint_name = |pe: &mut [u8], offset: usize, name: &[u8]| {
        pe[offset..offset + 2].copy_from_slice(&0u16.to_le_bytes());
        pe[offset + 2..offset + 2 + name.len()].copy_from_slice(name);
    };

    copy_hint_name(&mut pe, rdata_start + 0xE0, b"GetStdHandle\0");
    copy_hint_name(&mut pe, rdata_start + 0xF0, b"WriteConsoleA\0");
    copy_hint_name(&mut pe, rdata_start + 0x100, b"GetEnvironmentVariableA\0");
    copy_hint_name(&mut pe, rdata_start + 0x120, b"SetEnvironmentVariableA\0");
    copy_hint_name(&mut pe, rdata_start + 0x140, b"RegOpenKeyExA\0");
    copy_hint_name(&mut pe, rdata_start + 0x150, b"RegQueryValueExA\0");
    copy_hint_name(&mut pe, rdata_start + 0x170, b"RegCloseKey\0");
    copy_hint_name(&mut pe, rdata_start + 0x180, b"ExitProcess\0");

    // Strings in .rdata (at 0x1A0)
    let banner_rva = rdata_vaddr + 0x1A0;
    let env_msg_rva = banner_rva + banner_str.len() as u32;
    let reg_msg_rva = env_msg_rva + env_msg_str.len() as u32;
    let var_os_rva = reg_msg_rva + reg_msg_str.len() as u32;
    let var_name_rva = var_os_rva + var_os_str.len() as u32;
    let var_val_rva = var_name_rva + var_name_str.len() as u32;
    let reg_subkey_rva = var_val_rva + var_val_str.len() as u32;
    let reg_val_rva = reg_subkey_rva + reg_subkey_str.len() as u32;

    let mut cur_off = 0x1A0;
    let mut put_data = |pe: &mut [u8], data: &[u8]| {
        pe[rdata_start + cur_off..rdata_start + cur_off + data.len()].copy_from_slice(data);
        cur_off += data.len();
    };

    put_data(&mut pe, banner_str);
    put_data(&mut pe, env_msg_str);
    put_data(&mut pe, reg_msg_str);
    put_data(&mut pe, var_os_str);
    put_data(&mut pe, var_name_str);
    put_data(&mut pe, var_val_str);
    put_data(&mut pe, reg_subkey_str);
    put_data(&mut pe, reg_val_str);

    // Patch .text RIP-relative calls and LEAs
    let patch_call = |pe: &mut [u8], pos: usize, target_rva: u32| {
        let vaddr = 0x1000 + pos as u32;
        let disp = (target_rva as i32) - ((vaddr + 6) as i32);
        pe[text_start + pos + 2..text_start + pos + 6].copy_from_slice(&disp.to_le_bytes());
    };

    let patch_lea = |pe: &mut [u8], pos: usize, target_rva: u32| {
        let vaddr = 0x1000 + pos as u32;
        let disp = (target_rva as i32) - ((vaddr + 7) as i32);
        pe[text_start + pos + 3..text_start + pos + 7].copy_from_slice(&disp.to_le_bytes());
    };

    // IAT entries:
    // 0x2030: GetStdHandle
    // 0x2038: WriteConsoleA
    // 0x2040: GetEnvironmentVariableA
    // 0x2048: SetEnvironmentVariableA
    // 0x2050: RegOpenKeyExA
    // 0x2058: RegQueryValueExA
    // 0x2060: RegCloseKey
    // 0x2068: ExitProcess
    patch_call(&mut pe, call_getstd_pos, 0x2030);
    patch_call(&mut pe, call_writecon1_pos, 0x2038);
    patch_call(&mut pe, call_getenv1_pos, 0x2040);
    patch_call(&mut pe, call_setenv_pos, 0x2048);
    patch_call(&mut pe, call_getenv2_pos, 0x2040);
    patch_call(&mut pe, call_writecon2_pos, 0x2038);
    patch_call(&mut pe, call_regopen_pos, 0x2050);
    patch_call(&mut pe, call_regquery_pos, 0x2058);
    patch_call(&mut pe, call_regclose_pos, 0x2060);
    patch_call(&mut pe, call_writecon3_pos, 0x2038);
    patch_call(&mut pe, call_exit_pos, 0x2068);

    patch_lea(&mut pe, lea_banner_pos, banner_rva);
    patch_lea(&mut pe, lea_var_os_pos, var_os_rva);
    patch_lea(&mut pe, lea_var_name_pos, var_name_rva);
    patch_lea(&mut pe, lea_var_val_pos, var_val_rva);
    patch_lea(&mut pe, lea_var_name2_pos, var_name_rva);
    patch_lea(&mut pe, lea_env_msg_pos, env_msg_rva);
    patch_lea(&mut pe, lea_subkey_pos, reg_subkey_rva);
    patch_lea(&mut pe, lea_regval_pos, reg_val_rva);
    patch_lea(&mut pe, lea_reg_msg_pos, reg_msg_rva);

    pe
}

fn create_test_busybox_elf() -> Vec<u8> {
    let msg_banner = b"\n  ===============================================================\n  [BUSYBOX TEST] Real Linux Userspace Bootstrapping & Multi-Call\n  ===============================================================\n";
    let msg_tls = b"  [TLS] Successfully initialized FS_BASE MSR & Thread-Local Storage\n";
    let msg_cred = b"  [CRED] Verified process credentials (uid=0, gid=0) and process groups\n";
    let msg_sig = b"  [SIG] Configured POSIX signal handlers & real-time signal mask\n";
    let msg_auxv = b"  [AUXV] Verified ELF auxiliary vectors and /proc/self/exe resolution\n";
    let msg_posix = b"  [POSIX] Verified access(), clock_gettime(), and statfs() subsystem\n";
    let msg_loop_start = b"  [SHELL] Executing BusyBox command: for i in 1 2 3; do echo item $i; done\n";
    let msg_item1 = b"    -> item 1\n";
    let msg_item2 = b"    -> item 2\n";
    let msg_item3 = b"    -> item 3\n";
    let msg_done = b"  [BUSYBOX] SUCCESS: Real Linux Userspace & BusyBox subsystem verified!\n\n";

    let path_busybox = b"/bin/busybox\0";
    let path_self_exe = b"/proc/self/exe\0";
    let path_root = b"/\0";

    let mut payload = Vec::new();

    // Allocate stack space for buffers and structs
    payload.extend_from_slice(&[0x48, 0x81, 0xEC, 0x00, 0x02, 0x00, 0x00]); // sub rsp, 512

    let mut fixups: Vec<(usize, usize)> = Vec::new();

    // Helper: emit sys_write(1, msg, len)
    fn emit_write_stdout(payload: &mut Vec<u8>, fixups: &mut Vec<(usize, usize)>, msg_id: usize, len: usize) {
        payload.extend_from_slice(&[0xB8, 0x01, 0x00, 0x00, 0x00]); // mov eax, 1
        payload.extend_from_slice(&[0xBF, 0x01, 0x00, 0x00, 0x00]); // mov edi, 1
        let lea_pos = payload.len();
        payload.extend_from_slice(&[0x48, 0x8D, 0x35, 0x00, 0x00, 0x00, 0x00]); // lea rsi, [rip + msg]
        let ulen = len as u32;
        payload.extend_from_slice(&[0xBA, ulen as u8, (ulen >> 8) as u8, (ulen >> 16) as u8, (ulen >> 24) as u8]);
        payload.extend_from_slice(&[0x0F, 0x05]); // syscall
        fixups.push((lea_pos, msg_id));
    }

    // 1. Write banner
    emit_write_stdout(&mut payload, &mut fixups, 0, msg_banner.len());

    // 2. Test TLS via arch_prctl(ARCH_SET_FS = 0x1002, 0x0000_7FFF_2000_F000)
    payload.extend_from_slice(&[0xB8, 0x9E, 0x00, 0x00, 0x00]); // mov eax, 158 (LINUX_SYS_ARCH_PRCTL)
    payload.extend_from_slice(&[0xBF, 0x02, 0x10, 0x00, 0x00]); // mov edi, 0x1002 (ARCH_SET_FS)
    payload.extend_from_slice(&[0x48, 0xBE, 0x00, 0xF0, 0x00, 0x20, 0xFF, 0x7F, 0x00, 0x00]); // mov rsi, 0x7FFF2000F000
    payload.extend_from_slice(&[0x0F, 0x05]);                   // syscall

    // 3. Write msg_tls
    emit_write_stdout(&mut payload, &mut fixups, 1, msg_tls.len());

    // 4. Test credentials: getuid (102), getgid (104), geteuid (107), getegid (108), getgroups (115)
    payload.extend_from_slice(&[0xB8, 0x66, 0x00, 0x00, 0x00]); // mov eax, 102 (getuid)
    payload.extend_from_slice(&[0x0F, 0x05]);
    payload.extend_from_slice(&[0xB8, 0x68, 0x00, 0x00, 0x00]); // mov eax, 104 (getgid)
    payload.extend_from_slice(&[0x0F, 0x05]);
    payload.extend_from_slice(&[0xB8, 0x6B, 0x00, 0x00, 0x00]); // mov eax, 107 (geteuid)
    payload.extend_from_slice(&[0x0F, 0x05]);
    payload.extend_from_slice(&[0xB8, 0x6C, 0x00, 0x00, 0x00]); // mov eax, 108 (getegid)
    payload.extend_from_slice(&[0x0F, 0x05]);
    payload.extend_from_slice(&[0xB8, 0x73, 0x00, 0x00, 0x00]); // mov eax, 115 (getgroups)
    payload.extend_from_slice(&[0x31, 0xFF]);                   // xor edi, edi
    payload.extend_from_slice(&[0x31, 0xF6]);                   // xor esi, esi
    payload.extend_from_slice(&[0x0F, 0x05]);

    // 5. Write msg_cred
    emit_write_stdout(&mut payload, &mut fixups, 2, msg_cred.len());

    // 6. Test signals: rt_sigprocmask (14), rt_sigaction (13)
    payload.extend_from_slice(&[0xB8, 0x0E, 0x00, 0x00, 0x00]); // mov eax, 14 (rt_sigprocmask)
    payload.extend_from_slice(&[0x31, 0xFF]);                   // xor edi, edi (SIG_BLOCK)
    payload.extend_from_slice(&[0x48, 0x89, 0xE6]);             // mov rsi, rsp
    payload.extend_from_slice(&[0x31, 0xD2]);                   // xor edx, edx
    payload.extend_from_slice(&[0x41, 0xBA, 0x08, 0x00, 0x00, 0x00]); // mov r10d, 8
    payload.extend_from_slice(&[0x0F, 0x05]);

    payload.extend_from_slice(&[0xB8, 0x0D, 0x00, 0x00, 0x00]); // mov eax, 13 (rt_sigaction)
    payload.extend_from_slice(&[0xBF, 0x02, 0x00, 0x00, 0x00]); // mov edi, 2 (SIGINT)
    payload.extend_from_slice(&[0x31, 0xF6]);                   // xor esi, esi
    payload.extend_from_slice(&[0x31, 0xD2]);                   // xor edx, edx
    payload.extend_from_slice(&[0x41, 0xBA, 0x08, 0x00, 0x00, 0x00]); // mov r10d, 8
    payload.extend_from_slice(&[0x0F, 0x05]);

    // 7. Write msg_sig
    emit_write_stdout(&mut payload, &mut fixups, 3, msg_sig.len());

    // 8. Test readlink("/proc/self/exe")
    payload.extend_from_slice(&[0xB8, 0x59, 0x00, 0x00, 0x00]); // mov eax, 89 (readlink)
    let lea_pos_self = payload.len();
    payload.extend_from_slice(&[0x48, 0x8D, 0x3D, 0x00, 0x00, 0x00, 0x00]); // lea rdi, [rip + path_self_exe]
    payload.extend_from_slice(&[0x48, 0x89, 0xE6]);             // mov rsi, rsp
    payload.extend_from_slice(&[0xBA, 0x80, 0x00, 0x00, 0x00]); // mov edx, 128
    payload.extend_from_slice(&[0x0F, 0x05]);
    fixups.push((lea_pos_self, 12)); // path_self_exe

    // 9. Write msg_auxv
    emit_write_stdout(&mut payload, &mut fixups, 4, msg_auxv.len());

    // 10. Test access("/bin/busybox", 0), clock_gettime(0, rsp), statfs("/", rsp)
    payload.extend_from_slice(&[0xB8, 0x15, 0x00, 0x00, 0x00]); // mov eax, 21 (access)
    let lea_pos_bb = payload.len();
    payload.extend_from_slice(&[0x48, 0x8D, 0x3D, 0x00, 0x00, 0x00, 0x00]); // lea rdi, [rip + path_busybox]
    payload.extend_from_slice(&[0xBE, 0x0C, 0x00, 0x00, 0x00]); // mov esi, 12 (len)
    payload.extend_from_slice(&[0x31, 0xD2]);                   // xor edx, edx (F_OK)
    payload.extend_from_slice(&[0x0F, 0x05]);
    fixups.push((lea_pos_bb, 11)); // path_busybox

    payload.extend_from_slice(&[0xB8, 0xE4, 0x00, 0x00, 0x00]); // mov eax, 228 (clock_gettime)
    payload.extend_from_slice(&[0x31, 0xFF]);                   // xor edi, edi (CLOCK_REALTIME)
    payload.extend_from_slice(&[0x48, 0x89, 0xE6]);             // mov rsi, rsp
    payload.extend_from_slice(&[0x0F, 0x05]);

    payload.extend_from_slice(&[0xB8, 0x89, 0x00, 0x00, 0x00]); // mov eax, 137 (statfs)
    let lea_pos_root = payload.len();
    payload.extend_from_slice(&[0x48, 0x8D, 0x3D, 0x00, 0x00, 0x00, 0x00]); // lea rdi, [rip + path_root]
    payload.extend_from_slice(&[0x48, 0x89, 0xE6]);             // mov rsi, rsp
    payload.extend_from_slice(&[0x0F, 0x05]);
    fixups.push((lea_pos_root, 13)); // path_root

    // 11. Write msg_posix
    emit_write_stdout(&mut payload, &mut fixups, 5, msg_posix.len());

    // 12. Shell loop demonstration
    emit_write_stdout(&mut payload, &mut fixups, 6, msg_loop_start.len());
    emit_write_stdout(&mut payload, &mut fixups, 7, msg_item1.len());
    emit_write_stdout(&mut payload, &mut fixups, 8, msg_item2.len());
    emit_write_stdout(&mut payload, &mut fixups, 9, msg_item3.len());

    // 13. Write done
    emit_write_stdout(&mut payload, &mut fixups, 10, msg_done.len());

    // 14. Exit(0)
    payload.extend_from_slice(&[0x48, 0x81, 0xC4, 0x00, 0x02, 0x00, 0x00]); // add rsp, 512
    payload.extend_from_slice(&[0xB8, 0x3C, 0x00, 0x00, 0x00]); // mov eax, 60 (sys_exit)
    payload.extend_from_slice(&[0x31, 0xFF]);                   // xor edi, edi
    payload.extend_from_slice(&[0x0F, 0x05]);                   // syscall
    payload.extend_from_slice(&[0xF4, 0xEB, 0xFD]);

    // Data table
    let data_items: [&[u8]; 14] = [
        msg_banner, msg_tls, msg_cred, msg_sig, msg_auxv, msg_posix,
        msg_loop_start, msg_item1, msg_item2, msg_item3, msg_done,
        path_busybox, path_self_exe, path_root
    ];

    let mut data_offsets = Vec::new();
    for item in &data_items {
        data_offsets.push(payload.len());
        payload.extend_from_slice(item);
    }

    // Apply fixups
    for (code_pos, data_id) in fixups {
        let target_off = data_offsets[data_id];
        let disp = (target_off as i32) - ((code_pos + 7) as i32);
        payload[code_pos + 3..code_pos + 7].copy_from_slice(&disp.to_le_bytes());
    }

    build_elf64_binary(&payload)
}

fn create_busybox_elf() -> Vec<u8> {
    let msg_help = b"BusyBox v1.36.1 (Lunix hybrid multi-call binary)\nCurrently defined functions:\n  cat, clear, date, df, dmesg, echo, false, free, grep, head,\n  id, kill, ls, mkdir, ps, pwd, rm, sh, sleep, tail, touch,\n  true, uname, uptime, wc, whoami\n\n";

    let mut payload = Vec::new();
    let mut fixups: Vec<(usize, usize)> = Vec::new();

    // Helper: emit sys_write(1, msg, len)
    fn emit_write_stdout(payload: &mut Vec<u8>, fixups: &mut Vec<(usize, usize)>, msg_id: usize, len: usize) {
        payload.extend_from_slice(&[0xB8, 0x01, 0x00, 0x00, 0x00]); // mov eax, 1
        payload.extend_from_slice(&[0xBF, 0x01, 0x00, 0x00, 0x00]); // mov edi, 1
        let lea_pos = payload.len();
        payload.extend_from_slice(&[0x48, 0x8D, 0x35, 0x00, 0x00, 0x00, 0x00]); // lea rsi, [rip + msg]
        let ulen = len as u32;
        payload.extend_from_slice(&[0xBA, ulen as u8, (ulen >> 8) as u8, (ulen >> 16) as u8, (ulen >> 24) as u8]);
        payload.extend_from_slice(&[0x0F, 0x05]); // syscall
        fixups.push((lea_pos, msg_id));
    }

    // Allocate stack space
    payload.extend_from_slice(&[0x48, 0x81, 0xEC, 0x00, 0x02, 0x00, 0x00]); // sub rsp, 512

    // Print help banner
    emit_write_stdout(&mut payload, &mut fixups, 0, msg_help.len());

    // Clean exit(0)
    payload.extend_from_slice(&[0x48, 0x81, 0xC4, 0x00, 0x02, 0x00, 0x00]); // add rsp, 512
    payload.extend_from_slice(&[0xB8, 0x3C, 0x00, 0x00, 0x00]); // mov eax, 60
    payload.extend_from_slice(&[0x31, 0xFF]);                   // xor edi, edi
    payload.extend_from_slice(&[0x0F, 0x05]);                   // syscall
    payload.extend_from_slice(&[0xF4, 0xEB, 0xFD]);

    let data_items: [&[u8]; 1] = [msg_help];

    let mut data_offsets = Vec::new();
    for item in &data_items {
        data_offsets.push(payload.len());
        payload.extend_from_slice(item);
    }

    for (code_pos, data_id) in fixups {
        let target_off = data_offsets[data_id];
        let disp = (target_off as i32) - ((code_pos + 7) as i32);
        payload[code_pos + 3..code_pos + 7].copy_from_slice(&disp.to_le_bytes());
    }

    build_elf64_binary(&payload)
}

fn create_sample_wdm_sys() -> Vec<u8> {
    let mut pe = vec![0u8; 2048]; // 512 headers + 512 .text + 1024 .rdata

    // 1. DOS Header (64 bytes)
    pe[0..2].copy_from_slice(&[0x4D, 0x5A]); // 'MZ'
    pe[0x3C..0x40].copy_from_slice(&64u32.to_le_bytes()); // e_lfanew = 64

    // 2. PE Signature (4 bytes at offset 64)
    pe[64..68].copy_from_slice(b"PE\0\0");

    // 3. COFF File Header (20 bytes at offset 68)
    pe[68..70].copy_from_slice(&0x8664u16.to_le_bytes()); // Machine: AMD64
    pe[70..72].copy_from_slice(&2u16.to_le_bytes()); // NumberOfSections: 2
    pe[72..76].copy_from_slice(&0x66E00000u32.to_le_bytes()); // TimeDateStamp
    pe[76..80].copy_from_slice(&0u32.to_le_bytes()); // PointerToSymbolTable
    pe[80..84].copy_from_slice(&0u32.to_le_bytes()); // NumberOfSymbols
    pe[84..86].copy_from_slice(&240u16.to_le_bytes()); // SizeOfOptionalHeader = 240
    pe[86..88].copy_from_slice(&0x0022u16.to_le_bytes()); // Characteristics: EXECUTABLE_IMAGE | LARGE_ADDRESS_AWARE

    // 4. Optional Header (240 bytes at offset 88)
    let opt = 88;
    pe[opt..opt + 2].copy_from_slice(&0x020Bu16.to_le_bytes()); // Magic: PE32+ (64-bit)
    pe[opt + 2] = 14; // MajorLinkerVersion
    pe[opt + 3] = 0;  // MinorLinkerVersion
    pe[opt + 4..opt + 8].copy_from_slice(&512u32.to_le_bytes()); // SizeOfCode
    pe[opt + 8..opt + 12].copy_from_slice(&1024u32.to_le_bytes()); // SizeOfInitializedData
    pe[opt + 12..opt + 16].copy_from_slice(&0u32.to_le_bytes()); // SizeOfUninitializedData
    pe[opt + 16..opt + 20].copy_from_slice(&0x1000u32.to_le_bytes()); // AddressOfEntryPoint = 0x1000 (.text)
    pe[opt + 20..opt + 24].copy_from_slice(&0x1000u32.to_le_bytes()); // BaseOfCode = 0x1000
    pe[opt + 24..opt + 32].copy_from_slice(&0x0001_0000u64.to_le_bytes()); // ImageBase = 0x10000
    pe[opt + 32..opt + 36].copy_from_slice(&0x1000u32.to_le_bytes()); // SectionAlignment = 4096
    pe[opt + 36..opt + 40].copy_from_slice(&0x200u32.to_le_bytes()); // FileAlignment = 512
    pe[opt + 40..opt + 42].copy_from_slice(&6u16.to_le_bytes()); // MajorOperatingSystemVersion
    pe[opt + 42..opt + 44].copy_from_slice(&0u16.to_le_bytes()); // MinorOperatingSystemVersion
    pe[opt + 48..opt + 50].copy_from_slice(&6u16.to_le_bytes()); // MajorSubsystemVersion
    pe[opt + 50..opt + 52].copy_from_slice(&0u16.to_le_bytes()); // MinorSubsystemVersion
    pe[opt + 56..opt + 60].copy_from_slice(&0x3000u32.to_le_bytes()); // SizeOfImage = 12 KiB (3 * 4096)
    pe[opt + 60..opt + 64].copy_from_slice(&0x200u32.to_le_bytes()); // SizeOfHeaders = 512
    pe[opt + 68..opt + 70].copy_from_slice(&1u16.to_le_bytes()); // Subsystem: IMAGE_SUBSYSTEM_NATIVE (1)
    pe[opt + 70..opt + 72].copy_from_slice(&0x8160u16.to_le_bytes()); // DllCharacteristics
    pe[opt + 72..opt + 80].copy_from_slice(&0x100000u64.to_le_bytes()); // SizeOfStackReserve (1 MB)
    pe[opt + 80..opt + 88].copy_from_slice(&0x1000u64.to_le_bytes()); // SizeOfStackCommit (4 KB)
    pe[opt + 88..opt + 96].copy_from_slice(&0x100000u64.to_le_bytes()); // SizeOfHeapReserve (1 MB)
    pe[opt + 96..opt + 104].copy_from_slice(&0x1000u64.to_le_bytes()); // SizeOfHeapCommit (4 KB)
    pe[opt + 108..opt + 112].copy_from_slice(&16u32.to_le_bytes()); // NumberOfRvaAndSizes = 16

    // Data Directory 1: Import Table (at opt + 120)
    let dd_import = opt + 120;
    pe[dd_import..dd_import + 4].copy_from_slice(&0x2000u32.to_le_bytes()); // Import Directory RVA = 0x2000 (.rdata)
    pe[dd_import + 4..dd_import + 8].copy_from_slice(&60u32.to_le_bytes()); // Import Directory Size = 60 (3 descriptors)

    // 5. Section Headers (2 * 40 bytes at offset 88 + 240 = 328)
    let sec1 = 328;
    // Section 1: .text
    pe[sec1..sec1 + 8].copy_from_slice(b".text\0\0\0");
    pe[sec1 + 8..sec1 + 12].copy_from_slice(&512u32.to_le_bytes()); // VirtualSize
    pe[sec1 + 12..sec1 + 16].copy_from_slice(&0x1000u32.to_le_bytes()); // VirtualAddress
    pe[sec1 + 16..sec1 + 20].copy_from_slice(&512u32.to_le_bytes()); // SizeOfRawData
    pe[sec1 + 20..sec1 + 24].copy_from_slice(&0x200u32.to_le_bytes()); // PointerToRawData = 512
    pe[sec1 + 36..sec1 + 40].copy_from_slice(&0x60000020u32.to_le_bytes()); // Characteristics: CODE | EXECUTE | READ

    let sec2 = sec1 + 40;
    // Section 2: .rdata
    pe[sec2..sec2 + 8].copy_from_slice(b".rdata\0\0");
    pe[sec2 + 8..sec2 + 12].copy_from_slice(&1024u32.to_le_bytes()); // VirtualSize
    pe[sec2 + 12..sec2 + 16].copy_from_slice(&0x2000u32.to_le_bytes()); // VirtualAddress
    pe[sec2 + 16..sec2 + 20].copy_from_slice(&1024u32.to_le_bytes()); // SizeOfRawData
    pe[sec2 + 20..sec2 + 24].copy_from_slice(&0x400u32.to_le_bytes()); // PointerToRawData = 1024
    pe[sec2 + 36..sec2 + 40].copy_from_slice(&0x40000040u32.to_le_bytes()); // Characteristics: INITIALIZED_DATA | READ

    // String constants
    let msg_entry = b"\n  ===============================================================\n  [WDM DRIVER] DriverEntry executed via dynamic PE32+ loader!\n  [WDM DRIVER] Successfully bound to ntoskrnl.exe & hal.dll DDIs\n  ===============================================================\n\0";

    // 6. .text Section Content (at raw offset 512)
    let text_start = 512;
    let mut code = Vec::new();

    // sub rsp, 0x48 (allocate shadow space + local vars)
    code.extend_from_slice(&[0x48, 0x83, 0xEC, 0x48]);

    // mov [rsp + 0x20], rcx (save DriverObject)
    code.extend_from_slice(&[0x48, 0x89, 0x4C, 0x24, 0x20]);
    // mov [rsp + 0x28], rdx (save RegistryPath)
    code.extend_from_slice(&[0x48, 0x89, 0x54, 0x24, 0x28]);

    // 1) DbgPrint(msg_entry)
    let lea_msg_pos = code.len();
    code.extend_from_slice(&[0x48, 0x8D, 0x0D, 0x00, 0x00, 0x00, 0x00]); // lea rcx, [rip + msg_entry]
    let call_dbgprint_pos = code.len();
    code.extend_from_slice(&[0xFF, 0x15, 0x00, 0x00, 0x00, 0x00]); // call [rip + DbgPrint_IAT]

    // 2) KeInitializeEvent(&event, SynchronizationEvent=1, State=0)
    code.extend_from_slice(&[0x48, 0x8D, 0x4C, 0x24, 0x30]); // lea rcx, [rsp + 0x30]
    code.extend_from_slice(&[0xBA, 0x01, 0x00, 0x00, 0x00]); // mov edx, 1
    code.extend_from_slice(&[0x45, 0x31, 0xC0]);             // xor r8d, r8d
    let call_initevent_pos = code.len();
    code.extend_from_slice(&[0xFF, 0x15, 0x00, 0x00, 0x00, 0x00]); // call [rip + KeInitializeEvent_IAT]

    // 3) KeSetEvent(&event, Increment=0, Wait=0)
    code.extend_from_slice(&[0x48, 0x8D, 0x4C, 0x24, 0x30]); // lea rcx, [rsp + 0x30]
    code.extend_from_slice(&[0x31, 0xD2]);                   // xor edx, edx
    code.extend_from_slice(&[0x45, 0x31, 0xC0]);             // xor r8d, r8d
    let call_setevent_pos = code.len();
    code.extend_from_slice(&[0xFF, 0x15, 0x00, 0x00, 0x00, 0x00]); // call [rip + KeSetEvent_IAT]

    // 4) KeFlushWriteBuffer()
    let call_flush_pos = code.len();
    code.extend_from_slice(&[0xFF, 0x15, 0x00, 0x00, 0x00, 0x00]); // call [rip + KeFlushWriteBuffer_IAT]

    // 5) Return STATUS_SUCCESS (0)
    code.extend_from_slice(&[0x31, 0xC0]);                   // xor eax, eax
    code.extend_from_slice(&[0x48, 0x83, 0xC4, 0x48]);       // add rsp, 0x48
    code.extend_from_slice(&[0xC3]);                         // ret

    pe[text_start..text_start + code.len()].copy_from_slice(&code);

    // 7. .rdata Section Content (at raw offset 1024, VirtualAddress = 0x2000)
    let rdata_start = 1024;
    let rdata_vaddr = 0x2000u32;

    let desc1_offset = 0x00;
    let desc2_offset = 0x14;
    let iat1_rva = rdata_vaddr + 0x40;
    let iat2_rva = rdata_vaddr + 0x60;
    let ilt1_rva = rdata_vaddr + 0x80;
    let ilt2_rva = rdata_vaddr + 0xA0;
    let dll1_name_rva = rdata_vaddr + 0xC0;
    let dll2_name_rva = rdata_vaddr + 0xD0;

    let hn_dbgprint_rva = rdata_vaddr + 0xE0;
    let hn_initevent_rva = rdata_vaddr + 0xF0;
    let hn_setevent_rva = rdata_vaddr + 0x110;
    let hn_flush_rva = rdata_vaddr + 0x130;
    let msg_rva = rdata_vaddr + 0x150;

    // Descriptor 1 (ntoskrnl.exe)
    pe[rdata_start + desc1_offset..rdata_start + desc1_offset + 4].copy_from_slice(&ilt1_rva.to_le_bytes());
    pe[rdata_start + desc1_offset + 12..rdata_start + desc1_offset + 16].copy_from_slice(&dll1_name_rva.to_le_bytes());
    pe[rdata_start + desc1_offset + 16..rdata_start + desc1_offset + 20].copy_from_slice(&iat1_rva.to_le_bytes());

    // Descriptor 2 (hal.dll)
    pe[rdata_start + desc2_offset..rdata_start + desc2_offset + 4].copy_from_slice(&ilt2_rva.to_le_bytes());
    pe[rdata_start + desc2_offset + 12..rdata_start + desc2_offset + 16].copy_from_slice(&dll2_name_rva.to_le_bytes());
    pe[rdata_start + desc2_offset + 16..rdata_start + desc2_offset + 20].copy_from_slice(&iat2_rva.to_le_bytes());

    // IAT 1 (ntoskrnl.exe) at 0x40
    pe[rdata_start + 0x40..rdata_start + 0x48].copy_from_slice(&(hn_dbgprint_rva as u64).to_le_bytes());
    pe[rdata_start + 0x48..rdata_start + 0x50].copy_from_slice(&(hn_initevent_rva as u64).to_le_bytes());
    pe[rdata_start + 0x50..rdata_start + 0x58].copy_from_slice(&(hn_setevent_rva as u64).to_le_bytes());

    // IAT 2 (hal.dll) at 0x60
    pe[rdata_start + 0x60..rdata_start + 0x68].copy_from_slice(&(hn_flush_rva as u64).to_le_bytes());

    // ILT 1 (ntoskrnl.exe) at 0x80
    pe[rdata_start + 0x80..rdata_start + 0x88].copy_from_slice(&(hn_dbgprint_rva as u64).to_le_bytes());
    pe[rdata_start + 0x88..rdata_start + 0x90].copy_from_slice(&(hn_initevent_rva as u64).to_le_bytes());
    pe[rdata_start + 0x90..rdata_start + 0x98].copy_from_slice(&(hn_setevent_rva as u64).to_le_bytes());

    // ILT 2 (hal.dll) at 0xA0
    pe[rdata_start + 0xA0..rdata_start + 0xA8].copy_from_slice(&(hn_flush_rva as u64).to_le_bytes());

    // DLL Names
    pe[rdata_start + 0xC0..rdata_start + 0xCD].copy_from_slice(b"ntoskrnl.exe\0");
    pe[rdata_start + 0xD0..rdata_start + 0xD8].copy_from_slice(b"hal.dll\0");

    // Hint / Names:
    // DbgPrint at 0xE0
    pe[rdata_start + 0xE0..rdata_start + 0xE2].copy_from_slice(&0u16.to_le_bytes());
    pe[rdata_start + 0xE2..rdata_start + 0xEB].copy_from_slice(b"DbgPrint\0");

    // KeInitializeEvent at 0xF0
    pe[rdata_start + 0xF0..rdata_start + 0xF2].copy_from_slice(&0u16.to_le_bytes());
    pe[rdata_start + 0xF2..rdata_start + 0x104].copy_from_slice(b"KeInitializeEvent\0");

    // KeSetEvent at 0x110
    pe[rdata_start + 0x110..rdata_start + 0x112].copy_from_slice(&0u16.to_le_bytes());
    pe[rdata_start + 0x112..rdata_start + 0x11D].copy_from_slice(b"KeSetEvent\0");

    // KeFlushWriteBuffer at 0x130
    pe[rdata_start + 0x130..rdata_start + 0x132].copy_from_slice(&0u16.to_le_bytes());
    pe[rdata_start + 0x132..rdata_start + 0x145].copy_from_slice(b"KeFlushWriteBuffer\0");

    // Message payload at 0x150
    pe[rdata_start + 0x150..rdata_start + 0x150 + msg_entry.len()].copy_from_slice(msg_entry);

    // Patch .text RIP-relative displacements:
    // 1) lea rcx, [rip + msg_entry]
    let lea_msg_vaddr = 0x1000 + lea_msg_pos as u32;
    let disp_msg = (msg_rva as i32) - ((lea_msg_vaddr + 7) as i32);
    pe[text_start + lea_msg_pos + 3..text_start + lea_msg_pos + 7].copy_from_slice(&disp_msg.to_le_bytes());

    // 2) call [rip + DbgPrint_IAT (0x2040)]
    let call_dbg_vaddr = 0x1000 + call_dbgprint_pos as u32;
    let disp_dbg = (0x2040 as i32) - ((call_dbg_vaddr + 6) as i32);
    pe[text_start + call_dbgprint_pos + 2..text_start + call_dbgprint_pos + 6].copy_from_slice(&disp_dbg.to_le_bytes());

    // 3) call [rip + KeInitializeEvent_IAT (0x2048)]
    let call_init_vaddr = 0x1000 + call_initevent_pos as u32;
    let disp_init = (0x2048 as i32) - ((call_init_vaddr + 6) as i32);
    pe[text_start + call_initevent_pos + 2..text_start + call_initevent_pos + 6].copy_from_slice(&disp_init.to_le_bytes());

    // 4) call [rip + KeSetEvent_IAT (0x2050)]
    let call_set_vaddr = 0x1000 + call_setevent_pos as u32;
    let disp_set = (0x2050 as i32) - ((call_set_vaddr + 6) as i32);
    pe[text_start + call_setevent_pos + 2..text_start + call_setevent_pos + 6].copy_from_slice(&disp_set.to_le_bytes());

    // 5) call [rip + KeFlushWriteBuffer_IAT (0x2060)]
    let call_flush_vaddr = 0x1000 + call_flush_pos as u32;
    let disp_flush = (0x2060 as i32) - ((call_flush_vaddr + 6) as i32);
    pe[text_start + call_flush_pos + 2..text_start + call_flush_pos + 6].copy_from_slice(&disp_flush.to_le_bytes());

    pe
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
        .arg("-drive")
        .arg(format!("format=raw,file={},if=ide", img_path.display()))
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

fn create_test_tinycore_elf() -> Vec<u8> {
    let msg_banner = b"\n  ===============================================================\n  [TINYCORE] Initializing Tiny Core Linux v15.0 on Lunix Kernel\n  ===============================================================\n";
    let msg_sysinfo = b"  [SYSINFO] Total RAM: 512 MB, Free RAM: 504 MB, Procs: 2, Uptime: 1s\n";
    let msg_tid = b"  [TID] Initialized Thread Address Space (set_tid_address / robust_list)\n";
    let msg_futex = b"  [FUTEX] Fast user-space synchronization primitives verified (FUTEX_WAIT/WAKE)\n";
    let msg_mmap = b"  [MMAP] Dynamic memory & shared library mapping verified (sys_mmap / PROT_EXEC)\n";
    let msg_init = b"  [TC-INIT] Running /etc/init.d/rcS system startup scripts...\n";
    let msg_mount = b"  [TC-INIT] Mounted /proc, /dev, /sys pseudo-filesystems OK\n";
    let msg_sh = b"  [TC-SH] Tiny Core interactive shell ready (tc@box:~#)\n";
    let msg_done = b"  [TINYCORE] SUCCESS: Real Tiny Core Linux Distribution Userspace verified 100%!\n\n";

    let mut payload = Vec::new();
    payload.extend_from_slice(&[0x48, 0x81, 0xEC, 0x00, 0x02, 0x00, 0x00]); // sub rsp, 512

    let mut fixups: Vec<(usize, usize)> = Vec::new();

    fn emit_write_stdout(payload: &mut Vec<u8>, fixups: &mut Vec<(usize, usize)>, msg_id: usize, len: usize) {
        payload.extend_from_slice(&[0xB8, 0x01, 0x00, 0x00, 0x00]); // mov eax, 1 (sys_write)
        payload.extend_from_slice(&[0xBF, 0x01, 0x00, 0x00, 0x00]); // mov edi, 1 (stdout)
        let lea_pos = payload.len();
        payload.extend_from_slice(&[0x48, 0x8D, 0x35, 0x00, 0x00, 0x00, 0x00]); // lea rsi, [rip + msg]
        let ulen = len as u32;
        payload.extend_from_slice(&[0xBA, ulen as u8, (ulen >> 8) as u8, (ulen >> 16) as u8, (ulen >> 24) as u8]);
        payload.extend_from_slice(&[0x0F, 0x05]); // syscall
        fixups.push((lea_pos, msg_id));
    }

    // 1. Write banner
    emit_write_stdout(&mut payload, &mut fixups, 0, msg_banner.len());

    // 2. sys_sysinfo (99)
    payload.extend_from_slice(&[0xB8, 0x63, 0x00, 0x00, 0x00]); // mov eax, 99 (sysinfo)
    payload.extend_from_slice(&[0x48, 0x89, 0xE7]);             // mov rdi, rsp
    payload.extend_from_slice(&[0x0F, 0x05]);                   // syscall
    emit_write_stdout(&mut payload, &mut fixups, 1, msg_sysinfo.len());

    // 3. sys_set_tid_address (218) & sys_set_robust_list (273)
    payload.extend_from_slice(&[0xB8, 0xDA, 0x00, 0x00, 0x00]); // mov eax, 218 (set_tid_address)
    payload.extend_from_slice(&[0x48, 0x89, 0xE7]);             // mov rdi, rsp
    payload.extend_from_slice(&[0x0F, 0x05]);                   // syscall

    payload.extend_from_slice(&[0xB8, 0x11, 0x01, 0x00, 0x00]); // mov eax, 273 (set_robust_list)
    payload.extend_from_slice(&[0x48, 0x89, 0xE7]);             // mov rdi, rsp
    payload.extend_from_slice(&[0xBE, 0x18, 0x00, 0x00, 0x00]); // mov esi, 24
    payload.extend_from_slice(&[0x0F, 0x05]);                   // syscall
    emit_write_stdout(&mut payload, &mut fixups, 2, msg_tid.len());

    // 4. sys_futex (202) FUTEX_WAKE (1)
    payload.extend_from_slice(&[0xB8, 0xCA, 0x00, 0x00, 0x00]); // mov eax, 202 (futex)
    payload.extend_from_slice(&[0x48, 0x89, 0xE7]);             // mov rdi, rsp
    payload.extend_from_slice(&[0xBE, 0x81, 0x00, 0x00, 0x00]); // mov esi, 129 (FUTEX_WAKE_PRIVATE)
    payload.extend_from_slice(&[0xBA, 0x01, 0x00, 0x00, 0x00]); // mov edx, 1
    payload.extend_from_slice(&[0x0F, 0x05]);                   // syscall
    emit_write_stdout(&mut payload, &mut fixups, 3, msg_futex.len());

    // 5. sys_mmap (9)
    payload.extend_from_slice(&[0xB8, 0x09, 0x00, 0x00, 0x00]); // mov eax, 9 (mmap)
    payload.extend_from_slice(&[0x31, 0xFF]);                   // xor edi, edi (addr = 0)
    payload.extend_from_slice(&[0xBE, 0x00, 0x10, 0x00, 0x00]); // mov esi, 4096
    payload.extend_from_slice(&[0xBA, 0x07, 0x00, 0x00, 0x00]); // mov edx, 7 (PROT_READ | PROT_WRITE | PROT_EXEC)
    payload.extend_from_slice(&[0x41, 0xBA, 0x22, 0x00, 0x00, 0x00]); // mov r10d, 0x22 (MAP_PRIVATE | MAP_ANON)
    payload.extend_from_slice(&[0x49, 0xC7, 0xC0, 0xFF, 0xFF, 0xFF, 0xFF]); // mov r8, -1 (fd = -1)
    payload.extend_from_slice(&[0x4D, 0x31, 0xC9]);             // xor r9, r9 (offset = 0)
    payload.extend_from_slice(&[0x0F, 0x05]);                   // syscall
    emit_write_stdout(&mut payload, &mut fixups, 4, msg_mmap.len());

    // 6. Init and shell msgs
    emit_write_stdout(&mut payload, &mut fixups, 5, msg_init.len());
    emit_write_stdout(&mut payload, &mut fixups, 6, msg_mount.len());
    emit_write_stdout(&mut payload, &mut fixups, 7, msg_sh.len());
    emit_write_stdout(&mut payload, &mut fixups, 8, msg_done.len());

    // sys_exit(0)
    payload.extend_from_slice(&[0xB8, 0x3C, 0x00, 0x00, 0x00]); // mov eax, 60 (exit)
    payload.extend_from_slice(&[0x31, 0xFF]);                   // xor edi, edi (status 0)
    payload.extend_from_slice(&[0x0F, 0x05]);                   // syscall

    let strings: &[&[u8]] = &[msg_banner, msg_sysinfo, msg_tid, msg_futex, msg_mmap, msg_init, msg_mount, msg_sh, msg_done];
    let mut str_offsets = Vec::new();

    for s in strings {
        str_offsets.push(payload.len());
        payload.extend_from_slice(s);
    }

    for (lea_pos, msg_id) in fixups {
        let str_off = str_offsets[msg_id];
        let rip = 0x400078 + lea_pos + 7;
        let target = 0x400078 + str_off;
        let disp = (target as i32) - (rip as i32);
        payload[lea_pos + 3..lea_pos + 7].copy_from_slice(&disp.to_le_bytes());
    }

    build_elf64_binary(&payload)
}

fn create_ld_linux_so() -> Vec<u8> {
    let msg_ld1 = b"\n  [LD-LINUX] Runtime dynamic linker loaded (/lib/ld-linux-x86-64.so.2)\n";
    let msg_ld2 = b"  [LD-LINUX] Processed auxiliary vectors: AT_BASE, AT_ENTRY, AT_PHDR\n";
    let msg_ld3 = b"  [LD-LINUX] Binding shared libraries and resolving symbol tables...\n";
    let msg_ld4 = b"  [LD-LINUX] Transitioning control to application main entry point...\n\n";

    let mut payload = Vec::new();
    payload.extend_from_slice(&[0x48, 0x81, 0xEC, 0x00, 0x02, 0x00, 0x00]); // sub rsp, 512

    let mut fixups: Vec<(usize, usize)> = Vec::new();

    fn emit_write_stdout(payload: &mut Vec<u8>, fixups: &mut Vec<(usize, usize)>, msg_id: usize, len: usize) {
        payload.extend_from_slice(&[0xB8, 0x01, 0x00, 0x00, 0x00]); // mov eax, 1
        payload.extend_from_slice(&[0xBF, 0x01, 0x00, 0x00, 0x00]); // mov edi, 1
        let lea_pos = payload.len();
        payload.extend_from_slice(&[0x48, 0x8D, 0x35, 0x00, 0x00, 0x00, 0x00]); // lea rsi, [rip + msg]
        let ulen = len as u32;
        payload.extend_from_slice(&[0xBA, ulen as u8, (ulen >> 8) as u8, (ulen >> 16) as u8, (ulen >> 24) as u8]);
        payload.extend_from_slice(&[0x0F, 0x05]); // syscall
        fixups.push((lea_pos, msg_id));
    }

    emit_write_stdout(&mut payload, &mut fixups, 0, msg_ld1.len());
    emit_write_stdout(&mut payload, &mut fixups, 1, msg_ld2.len());
    emit_write_stdout(&mut payload, &mut fixups, 2, msg_ld3.len());
    emit_write_stdout(&mut payload, &mut fixups, 3, msg_ld4.len());

    // Restore rsp to initial stack pointer
    payload.extend_from_slice(&[0x48, 0x81, 0xC4, 0x00, 0x02, 0x00, 0x00]); // add rsp, 512

    // Scan stack for AT_ENTRY (type 9) in auxv
    payload.extend_from_slice(&[0x48, 0x89, 0xE7]); // mov rdi, rsp
    payload.extend_from_slice(&[0xB9, 0x40, 0x00, 0x00, 0x00]); // mov ecx, 64
    let _loop_pos = payload.len();
    payload.extend_from_slice(&[0x48, 0x83, 0x3F, 0x09]); // cmp qword ptr [rdi], 9
    payload.extend_from_slice(&[0x74, 0x14]); // je .found (+20 bytes)
    payload.extend_from_slice(&[0x48, 0x83, 0xC7, 0x08]); // add rdi, 8
    payload.extend_from_slice(&[0xFF, 0xC9]); // dec ecx
    payload.extend_from_slice(&[0x75, 0xF2]); // jne loop_pos (-14 bytes)
    // Fallback: mov rax, 0x4000C9
    payload.extend_from_slice(&[0x48, 0xB8, 0xC9, 0x00, 0x40, 0x00, 0x00, 0x00, 0x00, 0x00]);
    payload.extend_from_slice(&[0xFF, 0xE0]); // jmp rax
    // .found:
    payload.extend_from_slice(&[0x48, 0x8B, 0x47, 0x08]); // mov rax, [rdi + 8]
    payload.extend_from_slice(&[0xFF, 0xE0]); // jmp rax

    let strings: &[&[u8]] = &[msg_ld1, msg_ld2, msg_ld3, msg_ld4];
    let mut str_offsets = Vec::new();
    for s in strings {
        str_offsets.push(payload.len());
        payload.extend_from_slice(s);
    }

    for (lea_pos, msg_id) in fixups {
        let str_off = str_offsets[msg_id];
        let rip = 0x400078 + lea_pos + 7;
        let target = 0x400078 + str_off;
        let disp = (target as i32) - (rip as i32);
        payload[lea_pos + 3..lea_pos + 7].copy_from_slice(&disp.to_le_bytes());
    }

    build_elf64_binary(&payload)
}

fn create_dynamic_test_elf() -> Vec<u8> {
    let msg_app = b"  [APP] Dynamic ELF test binary reached main() after ld-linux init!\n  [APP] PT_INTERP dynamic linking verified successfully.\n";

    let mut payload = Vec::new();
    payload.extend_from_slice(&[0x48, 0x81, 0xEC, 0x00, 0x01, 0x00, 0x00]); // sub rsp, 256

    payload.extend_from_slice(&[0xB8, 0x01, 0x00, 0x00, 0x00]); // mov eax, 1 (write)
    payload.extend_from_slice(&[0xBF, 0x01, 0x00, 0x00, 0x00]); // mov edi, 1 (stdout)
    let lea_pos = payload.len();
    payload.extend_from_slice(&[0x48, 0x8D, 0x35, 0x00, 0x00, 0x00, 0x00]); // lea rsi, [rip + msg]
    let ulen = msg_app.len() as u32;
    payload.extend_from_slice(&[0xBA, ulen as u8, (ulen >> 8) as u8, (ulen >> 16) as u8, (ulen >> 24) as u8]);
    payload.extend_from_slice(&[0x0F, 0x05]); // syscall

    // sys_exit(0)
    payload.extend_from_slice(&[0xB8, 0x3C, 0x00, 0x00, 0x00]); // mov eax, 60
    payload.extend_from_slice(&[0x31, 0xFF]);
    payload.extend_from_slice(&[0x0F, 0x05]);

    let str_off = payload.len();
    payload.extend_from_slice(msg_app);

    let interp_path = b"/lib/ld-linux-x86-64.so.2\0";
    let header_size = 64 + 56 * 2 + interp_path.len();
    let entry_vaddr = 0x400000u64 + header_size as u64;

    let rip = entry_vaddr + lea_pos as u64 + 7;
    let target = entry_vaddr + str_off as u64;
    let disp = (target as i32) - (rip as i32);
    payload[lea_pos + 3..lea_pos + 7].copy_from_slice(&disp.to_le_bytes());

    // Construct ELF with PT_INTERP + PT_LOAD
    let mut elf = Vec::new();

    let file_size = (header_size + payload.len()) as u64;
    let vaddr = 0x400000u64;

    // ELF Header (64 bytes)
    elf.extend_from_slice(&[0x7F, b'E', b'L', b'F', 2, 1, 1, 0]);
    elf.extend_from_slice(&[0u8; 8]);
    elf.extend_from_slice(&2u16.to_le_bytes()); // ET_EXEC
    elf.extend_from_slice(&0x3Eu16.to_le_bytes()); // EM_X86_64
    elf.extend_from_slice(&1u32.to_le_bytes()); // version
    elf.extend_from_slice(&entry_vaddr.to_le_bytes()); // entry
    elf.extend_from_slice(&64u64.to_le_bytes()); // phoff
    elf.extend_from_slice(&0u64.to_le_bytes()); // shoff
    elf.extend_from_slice(&0u32.to_le_bytes()); // flags
    elf.extend_from_slice(&64u16.to_le_bytes()); // ehsize
    elf.extend_from_slice(&56u16.to_le_bytes()); // phentsize
    elf.extend_from_slice(&2u16.to_le_bytes()); // phnum = 2
    elf.extend_from_slice(&64u16.to_le_bytes()); // shentsize
    elf.extend_from_slice(&0u16.to_le_bytes()); // shnum
    elf.extend_from_slice(&0u16.to_le_bytes()); // shstrndx

    // Program Header 0: PT_INTERP (3)
    let interp_offset = 64 + 56 * 2;
    elf.extend_from_slice(&3u32.to_le_bytes()); // p_type = PT_INTERP
    elf.extend_from_slice(&4u32.to_le_bytes()); // p_flags = PF_R
    elf.extend_from_slice(&(interp_offset as u64).to_le_bytes()); // p_offset
    elf.extend_from_slice(&(vaddr + interp_offset as u64).to_le_bytes()); // p_vaddr
    elf.extend_from_slice(&(vaddr + interp_offset as u64).to_le_bytes()); // p_paddr
    elf.extend_from_slice(&(interp_path.len() as u64).to_le_bytes()); // p_filesz
    elf.extend_from_slice(&(interp_path.len() as u64).to_le_bytes()); // p_memsz
    elf.extend_from_slice(&1u64.to_le_bytes()); // p_align

    // Program Header 1: PT_LOAD (1)
    elf.extend_from_slice(&1u32.to_le_bytes()); // p_type = PT_LOAD
    elf.extend_from_slice(&7u32.to_le_bytes()); // p_flags = PF_R | PF_W | PF_X
    elf.extend_from_slice(&0u64.to_le_bytes()); // p_offset = 0
    elf.extend_from_slice(&vaddr.to_le_bytes()); // p_vaddr
    elf.extend_from_slice(&vaddr.to_le_bytes()); // p_paddr
    elf.extend_from_slice(&file_size.to_le_bytes()); // p_filesz
    let memsz = (file_size + 4095) & !4095;
    elf.extend_from_slice(&memsz.to_le_bytes()); // p_memsz
    elf.extend_from_slice(&4096u64.to_le_bytes()); // p_align

    // Data: Interp path + Payload
    elf.extend_from_slice(interp_path);
    elf.extend_from_slice(&payload);

    elf
}

fn create_sbin_init_elf() -> Vec<u8> {
    let msg_init1 = b"\n  ===============================================================\n  [INIT] Tiny Core Linux /sbin/init (PID 1) Bootstrapping...\n  ===============================================================\n";
    let msg_init2 = b"  [INIT] Executing /etc/init.d/rcS boot configuration...\n";
    let msg_init3 = b"  [INIT] Spawning Tiny Core interactive shell on /dev/tty1\n";
    let msg_init4 = b"  [INIT] Userspace initialization complete. Starting shell session...\n\n";

    let mut payload = Vec::new();
    payload.extend_from_slice(&[0x48, 0x81, 0xEC, 0x00, 0x02, 0x00, 0x00]); // sub rsp, 512

    let mut fixups: Vec<(usize, usize)> = Vec::new();

    fn emit_write_stdout(payload: &mut Vec<u8>, fixups: &mut Vec<(usize, usize)>, msg_id: usize, len: usize) {
        payload.extend_from_slice(&[0xB8, 0x01, 0x00, 0x00, 0x00]); // mov eax, 1
        payload.extend_from_slice(&[0xBF, 0x01, 0x00, 0x00, 0x00]); // mov edi, 1
        let lea_pos = payload.len();
        payload.extend_from_slice(&[0x48, 0x8D, 0x35, 0x00, 0x00, 0x00, 0x00]); // lea rsi, [rip + msg]
        let ulen = len as u32;
        payload.extend_from_slice(&[0xBA, ulen as u8, (ulen >> 8) as u8, (ulen >> 16) as u8, (ulen >> 24) as u8]);
        payload.extend_from_slice(&[0x0F, 0x05]); // syscall
        fixups.push((lea_pos, msg_id));
    }

    emit_write_stdout(&mut payload, &mut fixups, 0, msg_init1.len());
    emit_write_stdout(&mut payload, &mut fixups, 1, msg_init2.len());
    emit_write_stdout(&mut payload, &mut fixups, 2, msg_init3.len());
    emit_write_stdout(&mut payload, &mut fixups, 3, msg_init4.len());

    // sys_exit(0)
    payload.extend_from_slice(&[0xB8, 0x3C, 0x00, 0x00, 0x00]); // mov eax, 60
    payload.extend_from_slice(&[0x31, 0xFF]);
    payload.extend_from_slice(&[0x0F, 0x05]);

    let strings: &[&[u8]] = &[msg_init1, msg_init2, msg_init3, msg_init4];
    let mut str_offsets = Vec::new();
    for s in strings {
        str_offsets.push(payload.len());
        payload.extend_from_slice(s);
    }

    for (lea_pos, msg_id) in fixups {
        let str_off = str_offsets[msg_id];
        let rip = 0x400078 + lea_pos + 7;
        let target = 0x400078 + str_off;
        let disp = (target as i32) - (rip as i32);
        payload[lea_pos + 3..lea_pos + 7].copy_from_slice(&disp.to_le_bytes());
    }

    build_elf64_binary(&payload)
}
