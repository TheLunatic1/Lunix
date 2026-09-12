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


    // Create /etc (os-release, hostname)
    root_dir.create_dir("etc")?;
    let etc_dir = root_dir.open_dir("etc")?;

    let mut os_rel = etc_dir.create_file("os-release")?;
    os_rel.write_all(b"NAME=\"Lunix OS\"\nPRETTY_NAME=\"Lunix Hybrid OS (Windows NT + Linux ABI)\"\nID=lunix\nVERSION=\"0.1.0\"\nVERSION_ID=0.1.0\nBUILD_ID=\"2026-09-11\"\nANSI_COLOR=\"0;36\"\nHOME_URL=\"https://github.com/TheLunatic1/Lunix\"\n")?;

    let mut host_file = etc_dir.create_file("hostname")?;
    host_file.write_all(b"lunix-os\n")?;

    // Create /home/lunix/readme.txt
    root_dir.create_dir("home")?;
    let home_dir = root_dir.open_dir("home")?;
    home_dir.create_dir("lunix")?;
    let user_dir = home_dir.open_dir("lunix")?;
    let mut readme = user_dir.create_file("readme.txt")?;
    readme.write_all(b"Welcome to Lunix OS!\n\nA from-scratch hybrid operating system kernel written in 100% pure Rust.\nCombines Windows NT Driver Model (WDM) with Linux POSIX ABI & ELF64 execution.\n\nType 'help' to explore all Linux shell commands or 'gui' for LunixWM desktop.\n")?;

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
