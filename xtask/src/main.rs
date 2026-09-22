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

<<<<<<< HEAD
=======
struct CpioEntry {
    name: String,
    mode: u32,
    data: Vec<u8>,
}

fn unpack_cpio_gz(gz_bytes: &[u8]) -> Result<Vec<CpioEntry>> {
    use flate2::read::GzDecoder;
    let mut decoder = GzDecoder::new(gz_bytes);
    let mut cpio_data = Vec::new();
    decoder.read_to_end(&mut cpio_data)?;

    let mut entries = Vec::new();
    let mut pos = 0;
    while pos + 110 <= cpio_data.len() {
        let magic = &cpio_data[pos..pos + 6];
        if magic != b"070701" && magic != b"070702" {
            break;
        }
        let mode_str = std::str::from_utf8(&cpio_data[pos + 14..pos + 22])
            .unwrap_or("0");
        let size_str = std::str::from_utf8(&cpio_data[pos + 54..pos + 62])
            .unwrap_or("0");
        let name_str = std::str::from_utf8(&cpio_data[pos + 94..pos + 102])
            .unwrap_or("0");

        let mode = u32::from_str_radix(mode_str, 16).unwrap_or(0);
        let filesize = usize::from_str_radix(size_str, 16).unwrap_or(0);
        let namesize = usize::from_str_radix(name_str, 16).unwrap_or(0);

        pos += 110;
        if pos + namesize > cpio_data.len() {
            break;
        }
        let name_bytes = &cpio_data[pos..pos + namesize.saturating_sub(1)];
        let name = String::from_utf8_lossy(name_bytes).to_string();
        pos = (pos + namesize + 3) & !3;

        if name == "TRAILER!!!" {
            break;
        }

        if pos + filesize > cpio_data.len() {
            break;
        }
        let data = cpio_data[pos..pos + filesize].to_vec();
        pos = (pos + filesize + 3) & !3;

        entries.push(CpioEntry { name, mode, data });
    }

    Ok(entries)
}

fn resolve_relative_path(base_file: &str, target: &str) -> String {
    if target.starts_with('/') {
        return target.trim_start_matches('/').to_string();
    }
    let mut parts: Vec<&str> = base_file.split('/').filter(|s| !s.is_empty()).collect();
    if !parts.is_empty() {
        parts.pop(); // remove filename, keep directory
    }
    for seg in target.split('/') {
        if seg.is_empty() || seg == "." {
            continue;
        } else if seg == ".." {
            parts.pop();
        } else {
            parts.push(seg);
        }
    }
    parts.join("/")
}

fn create_dir_all_fs<T: Read + Write + std::io::Seek>(
    fs: &fatfs::FileSystem<T>,
    path: &str,
) -> Result<()> {
    let clean = path.trim_start_matches('/').trim_end_matches('/');
    if clean.is_empty() || clean == "." {
        return Ok(());
    }
    let parts: Vec<&str> = clean.split('/').filter(|s| !s.is_empty() && *s != ".").collect();
    let mut cur = fs.root_dir();
    for seg in parts {
        let next = match cur.open_dir(seg) {
            Ok(d) => d,
            Err(_) => {
                if let Err(e) = cur.create_dir(seg) {
                    eprintln!("[WARN] create_dir_all_fs create_dir '{}' in '{}' failed: {:?}", seg, path, e);
                }
                cur.open_dir(seg)
                    .with_context(|| format!("create_dir_all_fs failed to open directory '{}' in path '{}'", seg, path))?
            }
        };
        cur = next;
    }
    Ok(())
}

fn write_file_to_fs<T: Read + Write + std::io::Seek>(
    fs: &fatfs::FileSystem<T>,
    path: &str,
    data: &[u8],
) -> Result<()> {
    let clean = path.trim_start_matches('/');
    let parts: Vec<&str> = clean.split('/').filter(|s| !s.is_empty() && *s != ".").collect();
    if parts.is_empty() {
        return Ok(());
    }

    let file_name = parts[parts.len() - 1];
    let dir_parts = &parts[..parts.len() - 1];

    let mut cur = fs.root_dir();
    for seg in dir_parts {
        let next = match cur.open_dir(seg) {
            Ok(d) => d,
            Err(_) => {
                if let Err(e) = cur.create_dir(seg) {
                    eprintln!("[WARN] write_file_to_fs create_dir '{}' for '{}' failed: {:?}", seg, path, e);
                }
                cur.open_dir(seg)
                    .with_context(|| format!("write_file_to_fs failed to open directory '{}' for file '{}'", seg, path))?
            }
        };
        cur = next;
    }

    let mut file = cur.create_file(file_name)
        .with_context(|| format!("write_file_to_fs failed to create file '{}'", path))?;
    file.write_all(data)
        .with_context(|| format!("write_file_to_fs failed to write data to '{}'", path))?;
    Ok(())
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

    let total_disk_size: usize = 512 * 1024 * 1024; // 512 MiB total disk
    let total_sectors = (total_disk_size / 512) as u64;
    let partition_start_lba: u64 = 2048; // 1 MiB alignment
    let partition_end_lba: u64 = total_sectors - 34; // Leaves 33 sectors for Backup GPT table & header
    let partition_sectors = (partition_end_lba - partition_start_lba + 1) as usize;
    let partition_size_bytes = partition_sectors * 512;
    let partition_start_bytes = (partition_start_lba * 512) as usize;

    let mut part_buf = std::io::Cursor::new(vec![0u8; partition_size_bytes]);

    fatfs::format_volume(
        &mut part_buf,
        fatfs::FormatVolumeOptions::new()
            .fat_type(fatfs::FatType::Fat32)
            .bytes_per_cluster(4096),
    )?;

    {
        let fs = fatfs::FileSystem::new(&mut part_buf, fatfs::FsOptions::new())?;

        // 1. Install Lunix Bootloader and Core Kernel FIRST
        write_file_to_fs(&fs, "EFI/BOOT/BOOTX64.EFI", &bootloader_bytes)?;
        write_file_to_fs(&fs, "LUNIX/KERNEL.BIN", &kernel_bytes)?;

        // 2. Unpack upstream official Tiny Core Linux rootfs (corepure64.gz) if available
        let root = get_workspace_root();
        let core_gz_path = root.join("target").join("corepure64.gz");
        let mut file_map: HashMap<String, Vec<u8>> = HashMap::new();

        if core_gz_path.is_file() {
            println!("[+] Extracting official upstream Tiny Core Linux x86_64 rootfs (corepure64.gz)...");
            let mut gz_bytes = Vec::new();
            File::open(&core_gz_path)?.read_to_end(&mut gz_bytes)?;
            let entries = unpack_cpio_gz(&gz_bytes)?;
            println!("    -> Unpacked {} entries from Tiny Core CPIO archive", entries.len());

            // A. Index all regular files
            for entry in &entries {
                if (entry.mode & 0o170000) == 0o100000 {
                    file_map.insert(entry.name.trim_start_matches('/').to_string(), entry.data.clone());
                }
            }

            // B. Create directory structure
            for entry in &entries {
                if (entry.mode & 0o170000) == 0o040000 {
                    let _ = create_dir_all_fs(&fs, &entry.name);
                }
            }

            // C. Write regular files
            for entry in &entries {
                if (entry.mode & 0o170000) == 0o100000 {
                    let _ = write_file_to_fs(&fs, &entry.name, &entry.data);
                }
            }

            // D. Materialize symlinks
            for entry in &entries {
                if (entry.mode & 0o170000) == 0o120000 {
                    let target_str = String::from_utf8_lossy(&entry.data);
                    let resolved = resolve_relative_path(&entry.name, &target_str);
                    let clean_tgt = resolved.trim_start_matches('/');

                    if let Some(target_data) = file_map.get(clean_tgt) {
                        let _ = write_file_to_fs(&fs, &entry.name, target_data);
                    } else if clean_tgt.ends_with("busybox") || target_str.contains("busybox") {
                        if let Some(bb_data) = file_map.get("bin/busybox") {
                            let _ = write_file_to_fs(&fs, &entry.name, bb_data);
                        }
                    } else {
                        let sym_text = format!("symlink:{}\n", target_str);
                        let _ = write_file_to_fs(&fs, &entry.name, sym_text.as_bytes());
                    }
                }
            }

            // Ensure /lib64/ld-linux-x86-64.so.2 exists
            if let Some(ld_data) = file_map.get("lib/ld-linux-x86-64.so.2") {
                let _ = write_file_to_fs(&fs, "lib64/ld-linux-x86-64.so.2", ld_data);
            }
        }

        // 3. Extract and integrate official Tiny Core GUI TCZ extensions
        let iso_path = root.join("target").join("TinyCorePure64.iso");
        let tcz_all_dir = root.join("target").join("scratch").join("tcz_all");

        if !tcz_all_dir.is_dir() && iso_path.is_file() {
            println!("[+] Extracting 33 official Tiny Core Linux GUI packages (.tcz) from ISO...");
            let cde_raw = root.join("target").join("scratch").join("tcz_raw");
            let _ = std::fs::create_dir_all(&cde_raw);
            let _ = std::fs::create_dir_all(&tcz_all_dir);
            let _ = Command::new("7z")
                .args(["x", "-y", &format!("-o{}", cde_raw.display()), &iso_path.to_string_lossy(), "cde/optional/*.tcz"])
                .output();

            let opt_dir = cde_raw.join("cde").join("optional");
            if opt_dir.is_dir() {
                if let Ok(entries) = std::fs::read_dir(opt_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.extension().and_then(|s| s.to_str()) == Some("tcz") {
                            let _ = Command::new("7z")
                                .args(["x", "-y", &format!("-o{}", tcz_all_dir.display()), &path.to_string_lossy()])
                                .output();
                        }
                    }
                }
            }
        }

        if tcz_all_dir.is_dir() {
            println!("[+] Ingesting official Tiny Core Desktop packages (Xfbdev, flwm, wbar, aterm, cpanel, editor)...");
            let mut tcz_count = 0;
            fn walk_and_write(fs: &fatfs::FileSystem<&mut std::io::Cursor<Vec<u8>>>, dir: &Path, base: &Path, count: &mut usize) -> Result<()> {
                if let Ok(entries) = std::fs::read_dir(dir) {
                    for entry in entries.flatten() {
                        let p = entry.path();
                        let rel = p.strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/");
                        if p.is_dir() {
                            let _ = create_dir_all_fs(fs, &rel);
                            walk_and_write(fs, &p, base, count)?;
                        } else if p.is_file() {
                            if let Ok(data) = std::fs::read(&p) {
                                let _ = write_file_to_fs(fs, &rel, &data);
                                *count += 1;

                                // Mirror shared libraries (.so) to /lib and /usr/lib for universal dynamic linking
                                if rel.starts_with("usr/local/lib/") && (rel.ends_with(".so") || rel.contains(".so.")) {
                                    let lib_rel = rel.trim_start_matches("usr/local/lib/");
                                    if !lib_rel.contains('/') {
                                        let _ = write_file_to_fs(fs, &format!("lib/{}", lib_rel), &data);
                                        let _ = write_file_to_fs(fs, &format!("usr/lib/{}", lib_rel), &data);
                                    }
                                }
                            }
                        }
                    }
                }
                Ok(())
            }
            let _ = walk_and_write(&fs, &tcz_all_dir, &tcz_all_dir, &mut tcz_count);
            println!("    -> Successfully ingested {} official Tiny Core GUI files into FAT32 rootfs", tcz_count);
        }

        // 4. Install Standalone User ELF & Win32 PE Test Suite
        write_file_to_fs(&fs, "bin/hello.elf", &create_hello_elf())?;
        write_file_to_fs(&fs, "bin/sysinfo.elf", &create_sysinfo_elf())?;
        write_file_to_fs(&fs, "bin/test_fork.elf", &create_test_fork_elf())?;
        write_file_to_fs(&fs, "bin/test_exec.elf", &create_test_exec_elf())?;
        write_file_to_fs(&fs, "bin/win_hello.exe", &create_win32_hello_exe())?;
        write_file_to_fs(&fs, "bin/test_pipe.elf", &create_test_pipe_elf())?;
        write_file_to_fs(&fs, "bin/test_dir.elf", &create_test_dir_elf())?;
        write_file_to_fs(&fs, "bin/win_stream.exe", &create_win32_stream_exe())?;
        write_file_to_fs(&fs, "bin/test_devproc.elf", &create_test_devproc_elf())?;
        write_file_to_fs(&fs, "bin/win_envreg.exe", &create_win32_envreg_exe())?;
        write_file_to_fs(&fs, "bin/test_busybox.elf", &create_test_busybox_elf())?;
        write_file_to_fs(&fs, "bin/test_tinycore.elf", &create_test_tinycore_elf())?;
        write_file_to_fs(&fs, "bin/test_dynamic.elf", &create_dynamic_test_elf())?;
        write_file_to_fs(&fs, "lib/ld-linux-test.so.2", &create_ld_linux_so())?;

        // 5. If upstream BusyBox was not in archive, provide fallback BusyBox & Glibc
        let busybox_elf = create_busybox_elf();
        if !file_map.contains_key("bin/busybox") {
            write_file_to_fs(&fs, "bin/busybox", &busybox_elf)?;
            write_file_to_fs(&fs, "bin/sh", &busybox_elf)?;
            write_file_to_fs(&fs, "sbin/init", &create_sbin_init_elf())?;
            write_file_to_fs(&fs, "sbin/reboot", &busybox_elf)?;
            write_file_to_fs(&fs, "sbin/halt", &busybox_elf)?;
        }

        if !file_map.contains_key("lib/ld-linux-x86-64.so.2") {
            let ld_so_bytes = create_ld_linux_so();
            write_file_to_fs(&fs, "lib/ld-linux-x86-64.so.2", &ld_so_bytes)?;
            write_file_to_fs(&fs, "lib/libc.so.6", &ld_so_bytes)?;
            write_file_to_fs(&fs, "lib64/ld-linux-x86-64.so.2", &ld_so_bytes)?;
        }

        // 6. Install Windows NT Sample Driver
        let sample_wdm = create_sample_wdm_sys();
        write_file_to_fs(&fs, "sys/drivers/sample_wdm.sys", &sample_wdm)?;
        write_file_to_fs(&fs, "bin/sample_wdm.sys", &sample_wdm)?;

        // 7. Setup /etc configuration files
        let inittab_data = b"::sysinit:/etc/init.d/rcS\ntty1::respawn:/bin/sh\n::ctrlaltdel:/sbin/reboot\n::shutdown:/sbin/halt\n";
        write_file_to_fs(&fs, "etc/inittab", inittab_data)?;

        let rcs_data = b"#!/bin/sh\necho '[BOOT] Running /etc/init.d/rcS system startup scripts...'\n[ -f /proc/cmdline ] || /bin/mount /proc\n/bin/mount -o remount,rw /\n/bin/hostname box\nexport PATH=/bin:/usr/bin:/sbin:/usr/sbin:/usr/local/bin\nexport LD_LIBRARY_PATH=/usr/local/lib:/usr/lib:/lib:/lib64:/usr/lib64\nexport USER=tc\nexport HOME=/home/tc\nexport SHELL=/bin/sh\n";
        write_file_to_fs(&fs, "etc/init.d/rcS", rcs_data)?;

        let passwd_data = b"root:x:0:0:root:/root:/bin/sh\ntc:x:1001:1001:tc:/home/tc:/bin/sh\n";
        write_file_to_fs(&fs, "etc/passwd", passwd_data)?;

        let group_data = b"root:x:0:\nstaff:x:50:tc\ntc:x:1001:\n";
        write_file_to_fs(&fs, "etc/group", group_data)?;

        let fstab_data = b"/dev/sda / fat32 defaults 0 0\nproc /proc proc defaults 0 0\ndev /dev devtmpfs defaults 0 0\n";
        write_file_to_fs(&fs, "etc/fstab", fstab_data)?;

        let issue_data = b"\nTiny Core Linux v15.0 on Lunix Hybrid Kernel (x86_64)\n\n";
        write_file_to_fs(&fs, "etc/issue", issue_data)?;

        let os_rel_data = b"NAME=\"TinyCore Linux on Lunix\"\nPRETTY_NAME=\"Tiny Core Linux v15.0 (Lunix Hybrid Kernel)\"\nID=tinycore\nVERSION=\"15.0\"\nVERSION_ID=15.0\nBUILD_ID=\"2026-09-14\"\nANSI_COLOR=\"0;36\"\nHOME_URL=\"http://tinycorelinux.net\"\n";
        write_file_to_fs(&fs, "etc/os-release", os_rel_data)?;

        write_file_to_fs(&fs, "etc/hostname", b"box\n")?;
        write_file_to_fs(&fs, "etc/ld.so.conf", b"/lib\n/usr/lib\n/usr/local/lib\n/lib64\n/usr/lib64\n")?;

        // 8. Setup Tiny Core Desktop Configuration
        let _ = create_dir_all_fs(&fs, "etc/sysconfig");
        write_file_to_fs(&fs, "etc/sysconfig/Xserver", b"Xfbdev\n")?;
        write_file_to_fs(&fs, "etc/sysconfig/desktop", b"flwm\n")?;
        write_file_to_fs(&fs, "etc/sysconfig/icons", b"wbar\n")?;
        write_file_to_fs(&fs, "etc/sysconfig/tcuser", b"tc\n")?;
        write_file_to_fs(&fs, "etc/sysconfig/tcedir", b"/tce\n")?;

        // 9. Setup user profiles & desktop sessions
        let _ = create_dir_all_fs(&fs, "home/tc");
        let tc_profile = b"export USER=\"tc\"\nexport HOME=\"/home/tc\"\nexport SHELL=\"/bin/sh\"\nexport PATH=\"/bin:/usr/bin:/sbin:/usr/sbin:/usr/local/bin\"\nexport LD_LIBRARY_PATH=\"/usr/local/lib:/usr/lib:/lib:/lib64:/usr/lib64\"\nexport DISPLAY=\":0.0\"\necho 'Welcome to Tiny Core Linux on Lunix!'\n";
        write_file_to_fs(&fs, "home/tc/.profile", tc_profile)?;
        write_file_to_fs(&fs, "home/tc/readme.txt", b"Tiny Core Linux v15.0 FLWM + Wbar Graphical Desktop running on 100% pure Rust Lunix Kernel.\n")?;

        let xsession_data = b"#!/bin/sh\n# Tiny Core Linux Official Desktop Session\nexport DISPLAY=:0.0\nexport HOME=/home/tc\nexport USER=tc\nexport DESKTOP=flwm\nexport ICONS=wbar\nexport LD_LIBRARY_PATH=/usr/local/lib:/usr/lib:/lib:/lib64:/usr/lib64\nexport PATH=/bin:/usr/bin:/sbin:/usr/sbin:/usr/local/bin\n\n/usr/local/bin/Xfbdev -br -screen 1024x768x32 -mouse /dev/input/mice,3 &\nexport XPID=$!\n/bin/sleep 1\n/usr/local/bin/flwm &\n/usr/local/bin/wbar -bpress -pos bottom -zoomf 2 -isize 32 &\n/usr/local/bin/aterm -geometry 80x24+50+50 &\n";
        write_file_to_fs(&fs, "home/tc/.xsession", xsession_data)?;

        let wbar_data = b"i: /usr/local/share/wbar/osxbarback.png\nt: /usr/local/lib/X11/fonts/TTF/luxisr/11\nc: wbar -bpress -pos bottom -zoomf 2 -isize 32\n\ni: /usr/local/share/pixmaps/terminal.png\nt: Terminal\nc: aterm\n\ni: /usr/local/share/pixmaps/editor.png\nt: Editor\nc: editor\n\ni: /usr/local/share/pixmaps/cpanel.png\nt: ControlPanel\nc: cpanel\n\ni: /usr/local/share/pixmaps/exit.png\nt: Exit\nc: exittc\n";
        write_file_to_fs(&fs, "home/tc/.wbar", wbar_data)?;
        write_file_to_fs(&fs, "home/tc/.setbackground", b"#!/bin/sh\n/usr/local/bin/hsetroot -solid '#2d415f'\n")?;

        let lunix_readme = b"Welcome to Lunix OS!\n\nA from-scratch hybrid operating system kernel written in 100% pure Rust.\nCombines Windows NT Driver Model (WDM) with Linux POSIX ABI & ELF64 execution.\n\nType 'startx' to launch the official Tiny Core FLWM + Wbar Graphical Desktop.\n";
        write_file_to_fs(&fs, "home/lunix/readme.txt", lunix_readme)?;

        // 10. Ensure directories exist
        let _ = create_dir_all_fs(&fs, "tmp/.X11-unix");
        let _ = create_dir_all_fs(&fs, "var/run");
        let _ = create_dir_all_fs(&fs, "var/log");
        let _ = create_dir_all_fs(&fs, "usr/bin");
        let _ = create_dir_all_fs(&fs, "usr/sbin");
        let _ = create_dir_all_fs(&fs, "usr/lib");
        let _ = create_dir_all_fs(&fs, "usr/local/bin");
        let _ = create_dir_all_fs(&fs, "usr/local/lib");
        let _ = create_dir_all_fs(&fs, "tce/optional");
    }

    // Construct the complete GPT + Protective MBR disk image
    let mut disk_data = vec![0u8; total_disk_size];
    let total_sectors = (total_disk_size / 512) as u64;

    // 1. Protective MBR at Sector 0
    let entry_offset = 446;
    disk_data[entry_offset] = 0x00; // Boot Indicator
    disk_data[entry_offset + 1] = 0x00; // Starting Head
    disk_data[entry_offset + 2] = 0x02; // Starting Sector
    disk_data[entry_offset + 3] = 0x00; // Starting Cylinder
    disk_data[entry_offset + 4] = 0xEE; // Partition Type: GPT Protective
    disk_data[entry_offset + 5] = 0xFF; // Ending Head
    disk_data[entry_offset + 6] = 0xFF; // Ending Sector
    disk_data[entry_offset + 7] = 0xFF; // Ending Cylinder
    disk_data[entry_offset + 8..entry_offset + 12].copy_from_slice(&1u32.to_le_bytes()); // Starting LBA = 1
    disk_data[entry_offset + 12..entry_offset + 16].copy_from_slice(&((total_sectors - 1) as u32).to_le_bytes()); // Total Sectors

    // MBR Boot Signature at offset 510
    disk_data[510] = 0x55;
    disk_data[511] = 0xAA;

    // 2. GPT Partition Entries Array (128 entries * 128 bytes = 16384 bytes = 32 sectors)
    let mut part_entries = vec![0u8; 128 * 128];
    // Entry 0: EFI System Partition (ESP)
    // Partition Type GUID: C12A7328-F81F-11D2-BA4B-00A0C93EC93B (mixed-endian format)
    let esp_type_guid: [u8; 16] = [
        0x28, 0x73, 0x2A, 0xC1, 0x1F, 0xF8, 0xD2, 0x11, 0xBA, 0x4B, 0x00, 0xA0, 0xC9, 0x3E, 0xC9, 0x3B,
    ];
    let unique_part_guid: [u8; 16] = [
        0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x00,
    ];
    part_entries[0..16].copy_from_slice(&esp_type_guid);
    part_entries[16..32].copy_from_slice(&unique_part_guid);
    part_entries[32..40].copy_from_slice(&2048u64.to_le_bytes()); // Starting LBA = 2048
    part_entries[40..48].copy_from_slice(&(total_sectors - 34).to_le_bytes()); // Ending LBA
    part_entries[48..56].copy_from_slice(&0u64.to_le_bytes()); // Attributes = 0
    // Partition Name: UTF-16LE "EFI System Partition"
    let name_utf16: Vec<u16> = "EFI System Partition".encode_utf16().collect();
    for (idx, &ch) in name_utf16.iter().enumerate() {
        if idx < 36 {
            part_entries[56 + idx * 2..56 + idx * 2 + 2].copy_from_slice(&ch.to_le_bytes());
        }
    }

    let part_crc = calc_crc32(&part_entries);
    let disk_guid: [u8; 16] = [
        0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10,
    ];

    // 3. Primary GPT Header at Sector 1 (offset 512..512+92)
    let mut primary_hdr = vec![0u8; 92];
    primary_hdr[0..8].copy_from_slice(b"EFI PART");
    primary_hdr[8..12].copy_from_slice(&0x00010000u32.to_le_bytes()); // Revision 1.0
    primary_hdr[12..16].copy_from_slice(&92u32.to_le_bytes()); // Header size
    primary_hdr[16..20].copy_from_slice(&0u32.to_le_bytes()); // CRC32 (zero during computation)
    primary_hdr[20..24].copy_from_slice(&0u32.to_le_bytes()); // Reserved
    primary_hdr[24..32].copy_from_slice(&1u64.to_le_bytes()); // My LBA = 1
    primary_hdr[32..40].copy_from_slice(&(total_sectors - 1).to_le_bytes()); // Alternate LBA
    primary_hdr[40..48].copy_from_slice(&2048u64.to_le_bytes()); // First Usable LBA
    primary_hdr[48..56].copy_from_slice(&(total_sectors - 34).to_le_bytes()); // Last Usable LBA
    primary_hdr[56..72].copy_from_slice(&disk_guid);
    primary_hdr[72..80].copy_from_slice(&2u64.to_le_bytes()); // Partition entries starting LBA = 2
    primary_hdr[80..84].copy_from_slice(&128u32.to_le_bytes()); // Number of partition entries = 128
    primary_hdr[84..88].copy_from_slice(&128u32.to_le_bytes()); // Size of each partition entry = 128
    primary_hdr[88..92].copy_from_slice(&part_crc.to_le_bytes());

    let primary_hdr_crc = calc_crc32(&primary_hdr);
    primary_hdr[16..20].copy_from_slice(&primary_hdr_crc.to_le_bytes());

    disk_data[512..512 + 92].copy_from_slice(&primary_hdr);
    disk_data[1024..1024 + 16384].copy_from_slice(&part_entries);

    // 4. Backup GPT Header & Partition Array at end of disk
    let backup_part_lba = total_sectors - 33;
    let backup_hdr_lba = total_sectors - 1;
    let backup_part_start = (backup_part_lba * 512) as usize;
    disk_data[backup_part_start..backup_part_start + 16384].copy_from_slice(&part_entries);

    let mut backup_hdr = vec![0u8; 92];
    backup_hdr[0..8].copy_from_slice(b"EFI PART");
    backup_hdr[8..12].copy_from_slice(&0x00010000u32.to_le_bytes());
    backup_hdr[12..16].copy_from_slice(&92u32.to_le_bytes());
    backup_hdr[16..20].copy_from_slice(&0u32.to_le_bytes());
    backup_hdr[20..24].copy_from_slice(&0u32.to_le_bytes());
    backup_hdr[24..32].copy_from_slice(&backup_hdr_lba.to_le_bytes()); // My LBA = backup_hdr_lba
    backup_hdr[32..40].copy_from_slice(&1u64.to_le_bytes()); // Alternate LBA = 1
    backup_hdr[40..48].copy_from_slice(&2048u64.to_le_bytes());
    backup_hdr[48..56].copy_from_slice(&(total_sectors - 34).to_le_bytes());
    backup_hdr[56..72].copy_from_slice(&disk_guid);
    backup_hdr[72..80].copy_from_slice(&backup_part_lba.to_le_bytes());
    backup_hdr[80..84].copy_from_slice(&128u32.to_le_bytes());
    backup_hdr[84..88].copy_from_slice(&128u32.to_le_bytes());
    backup_hdr[88..92].copy_from_slice(&part_crc.to_le_bytes());

    let backup_hdr_crc = calc_crc32(&backup_hdr);
    backup_hdr[16..20].copy_from_slice(&backup_hdr_crc.to_le_bytes());

    let backup_hdr_start = (backup_hdr_lba * 512) as usize;
    disk_data[backup_hdr_start..backup_hdr_start + 92].copy_from_slice(&backup_hdr);

    // 5. Patch FAT32 BPB_HiddSec to 2048 (offset 28..32) on Primary Boot Sector and Backup Boot Sector (Sector 6)
    let mut part_bytes = part_buf.into_inner();
    let hidden_sectors = 2048u32.to_le_bytes();
    part_bytes[28..32].copy_from_slice(&hidden_sectors);
    if part_bytes.len() >= (6 * 512 + 32) {
        part_bytes[6 * 512 + 28..6 * 512 + 32].copy_from_slice(&hidden_sectors);
    }

    // Copy FAT32 partition data at offset 1 MiB (2048 * 512)
    disk_data[partition_start_bytes..partition_start_bytes + partition_size_bytes].copy_from_slice(&part_bytes);

    // Write RAM buffer to file in a single fast write
    let mut img_file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(img_path)?;

    img_file.write_all(&disk_data)?;
    Ok(())
}

>>>>>>> 67740566240f8bc3cf3fa1dbde7456fa0a3e6ea6
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

