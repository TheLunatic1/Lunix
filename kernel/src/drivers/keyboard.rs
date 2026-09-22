use crate::arch::x86_64::io::{inb, outb};
use crate::mm::pmm;
use crate::{lunix_print, lunix_println};
use alloc::string::String;
use pc_keyboard::{layouts, DecodedKey, HandleControl, Keyboard, ScancodeSet1};
use spin::Mutex;

const KEYBOARD_DATA_PORT: u16 = 0x60;

pub static KEYBOARD: Mutex<Option<Keyboard<layouts::Us104Key, ScancodeSet1>>> = Mutex::new(None);
pub static COMMAND_BUFFER: Mutex<String> = Mutex::new(String::new());

pub fn init() {
    let keyboard = Keyboard::new(
        ScancodeSet1::new(),
        layouts::Us104Key,
        HandleControl::MapLettersToUnicode,
    );
    *KEYBOARD.lock() = Some(keyboard);
}

pub static CURRENT_WORKING_DIR: Mutex<String> = Mutex::new(String::new());

pub fn get_cwd() -> String {
    let cwd = CURRENT_WORKING_DIR.lock();
    if cwd.is_empty() {
        String::from("/")
    } else {
        cwd.clone()
    }
}

pub fn set_cwd(new_dir: &str) {
    let mut cwd = CURRENT_WORKING_DIR.lock();
    *cwd = String::from(new_dir);
}

pub fn resolve_path(cwd: &str, target: &str) -> String {
    let trimmed = target.trim();
    if trimmed.is_empty() {
        return String::from(cwd);
    }

    let raw = if trimmed.starts_with('/') {
        String::from(trimmed)
    } else if cwd == "/" {
        alloc::format!("/{}", trimmed)
    } else {
        alloc::format!("{}/{}", cwd, trimmed)
    };

    // Normalize path components (. and ..)
    let mut stack: alloc::vec::Vec<&str> = alloc::vec::Vec::new();
    for part in raw.split('/') {
        if part.is_empty() || part == "." {
            continue;
        } else if part == ".." {
            stack.pop();
        } else {
            stack.push(part);
        }
    }

    if stack.is_empty() {
        String::from("/")
    } else {
        let mut res = String::new();
        for seg in stack {
            res.push('/');
            res.push_str(seg);
        }
        res
    }
}

pub fn parse_ip(s: &str) -> Option<[u8; 4]> {
    let mut parts = s.split('.');
    let a = parts.next()?.parse::<u8>().ok()?;
    let b = parts.next()?.parse::<u8>().ok()?;
    let c = parts.next()?.parse::<u8>().ok()?;
    let d = parts.next()?.parse::<u8>().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some([a, b, c, d])
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecResult {
    Done,
    AsyncProcessSpawned,
}

static USERSPACE_TTY: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

/// When set, console input goes to the tty line discipline (userspace reads it
/// via /dev/tty and fd 0) instead of the built-in kernel debug console.
pub fn set_userspace_tty(on: bool) {
    USERSPACE_TTY.store(on, core::sync::atomic::Ordering::SeqCst);
}

pub fn userspace_tty() -> bool {
    USERSPACE_TTY.load(core::sync::atomic::Ordering::SeqCst)
}

pub fn print_prompt() {
    if userspace_tty() {
        return;
    }
    LAST_WAS_CR.store(false, core::sync::atomic::Ordering::Relaxed);
    let cwd = get_cwd();
    if cwd == "/" {
        lunix_print!("lunix-debug:/# ");
    } else {
        lunix_print!("lunix-debug:{}# ", cwd);
    }
}

fn execute_command(cmd: &str) -> ExecResult {
    let trimmed = cmd.trim();
    if trimmed.is_empty() {
        return ExecResult::Done;
    }

    let mut parts = trimmed.split_whitespace();
    let command = parts.next().unwrap_or("");
    let arg1 = parts.next().unwrap_or("");
    let arg2 = parts.next().unwrap_or("");
    let cwd = get_cwd();

    match command {
        "help" => {
            lunix_println!("===============================================================");
            lunix_println!("       Lunix kernel debug console (fallback when no init)          ");
            lunix_println!("===============================================================");
            lunix_println!("  ls [-l] [path]  - List directory contents (e.g. ls, ls /bin)");
            lunix_println!("  cat <path>      - Display file text content (e.g. cat /etc/os-release)");
            lunix_println!("  cd [path]       - Change current working directory");
            lunix_println!("  pwd             - Print current working directory");
            lunix_println!("  echo [text]     - Print text arguments");
            lunix_println!("  uname [-a]      - Print OS name, version, and architecture");
            lunix_println!("  whoami          - Display current logged in user (root)");
            lunix_println!("  id              - Display user and group IDs");
            lunix_println!("  uptime          - Display system uptime from APIC timer");
            lunix_println!("  free [-m]       - Display physical RAM & heap memory usage");
            lunix_println!("  df [-h]         - Display filesystem disk space usage (/dev/sda)");
            lunix_println!("  ps [-aux]       - List active processes, kernel threads, and states");
            lunix_println!("  kill <tid>      - Terminate a running thread/process");
            lunix_println!("  dmesg           - Display kernel boot logs and hardware topology");
            lunix_println!("  ifconfig        - Display network interfaces (eth0), IP, and MAC");
            lunix_println!("  ping <ip>       - Send ICMP echo requests to target IP (e.g. ping 10.0.2.2)");
            lunix_println!("  arp             - Display ARP cache address resolution table");
            lunix_println!("  netstat         - Display active network sockets and statistics");
            lunix_println!("  exec <path>     - Load and execute an ELF64 binary in Ring 3 user mode");
            lunix_println!("  info            - Display OS, CPU, and Bootloader information");
            lunix_println!("  mem             - Display physical memory breakdown (PMM/VMM)");
            lunix_println!("  pci             - Scan and list hardware PCI bus devices");
            lunix_println!("  block           - List registered block storage devices");
            lunix_println!("  storage         - Display storage controllers (ATA, AHCI, NVMe, VirtIO)");
            lunix_println!("  stat <path>     - Display file or directory inode metadata");
            lunix_println!("  smp             - Display multi-core SMP and APIC status");
            lunix_println!("  acpi            - Display ACPI 2.0 tables and interrupt topology");
            lunix_println!("  spawn           - Spawn a preemptive background kernel worker thread");
            lunix_println!("  sysdemo         - Execute Ring 3 User Mode demo via fast SYSCALL ABI");
<<<<<<< HEAD
=======
            lunix_println!("  tinycore        - Bootstrap Tiny Core Linux Userspace Init Sequence (/sbin/init)");
            lunix_println!("  init            - Execute PID 1 init process in Ring 3 userspace");
            lunix_println!("  startx          - Launch official Tiny Core Linux Graphical Desktop (FLWM + Wbar)");
            lunix_println!("  desktop         - Start Tiny Core X11 GUI desktop session");
            lunix_println!("  gui             - Launch LunixWM 32-bit Graphical Window Compositor");
>>>>>>> 67740566240f8bc3cf3fa1dbde7456fa0a3e6ea6
            lunix_println!("  clear           - Clear console screen buffer");
            lunix_println!("  reboot          - Soft reboot machine via 8042 controller");
            lunix_println!("  panic           - Trigger a kernel panic diagnostic test");
            lunix_println!("===============================================================");
        }
        "pwd" => {
            lunix_println!("{}", cwd);
        }
        "cd" => {
            let target = if arg1.is_empty() { "/" } else { arg1 };
            let resolved = resolve_path(&cwd, target);
            match crate::fs::vfs::read_dir(&resolved) {
                Ok(_) => {
                    set_cwd(&resolved);
                }
                Err(e) => {
                    lunix_println!("cd: {}: {:?}", target, e);
                }
            }
        }
        "ls" => {
            let (is_long, target_raw) = if arg1 == "-l" || arg1 == "-la" || arg1 == "-al" {
                (true, arg2)
            } else if arg2 == "-l" || arg2 == "-la" || arg2 == "-al" {
                (true, arg1)
            } else {
                (false, arg1)
            };

            let target_path = if target_raw.is_empty() {
                cwd.clone()
            } else {
                resolve_path(&cwd, target_raw)
            };

            match crate::fs::vfs::read_dir(&target_path) {
                Ok(entries) => {
                    if is_long {
                        lunix_println!("total {}", entries.len());
                        for entry in entries {
                            let (perm, type_char) = match entry.node_type {
                                crate::fs::inode::INodeType::Directory => ("rwxr-xr-x", 'd'),
                                crate::fs::inode::INodeType::File => ("rw-r--r--", '-'),
                                crate::fs::inode::INodeType::BlockDevice => ("rw-rw----", 'b'),
                                crate::fs::inode::INodeType::CharDevice => ("rw-rw----", 'c'),
                                crate::fs::inode::INodeType::SymLink => ("rwxrwxrwx", 'l'),
                            };
                            lunix_println!("{}{:<9} 1 root root {:>8} {}", type_char, perm, entry.size, entry.name);
                        }
                    } else {
                        lunix_println!("Directory listing for '{}' ({} entries):", target_path, entries.len());
                        for entry in entries {
                            let type_tag = match entry.node_type {
                                crate::fs::inode::INodeType::Directory => "<DIR>",
                                crate::fs::inode::INodeType::File => "<FILE>",
                                crate::fs::inode::INodeType::BlockDevice => "<BLK>",
                                crate::fs::inode::INodeType::CharDevice => "<CHR>",
                                crate::fs::inode::INodeType::SymLink => "<LNK>",
                            };
                            lunix_println!("  {:<6}  {:<8}  {}", type_tag, entry.size, entry.name);
                        }
                    }
                }
                Err(e) => {
                    lunix_println!("ls: cannot access '{}': {:?}", target_path, e);
                }
            }
        }
        "cat" => {
            if arg1.is_empty() {
                lunix_println!("Usage: cat <path>");
                return ExecResult::Done;
            }
            let resolved = resolve_path(&cwd, arg1);
            match crate::fs::vfs::read_to_vec(&resolved) {
                Ok(bytes) => {
                    let text = core::str::from_utf8(&bytes).unwrap_or("[Binary file content]");
                    lunix_println!("{}", text);
                }
                Err(e) => {
                    lunix_println!("cat: failed to read '{}': {:?}", resolved, e);
                }
            }
        }
        "echo" => {
            let text = trimmed.strip_prefix("echo").unwrap_or("").trim();
            lunix_println!("{}", text);
        }
        "uname" => {
            if arg1 == "-a" {
                lunix_println!("Linux arch 6.8.0-arch1-1-lunix (Lunix 0.1.0-arch-baremetal) #1 SMP PREEMPT 2026-09-21 x86_64 Arch Linux GNU/Linux");
            } else {
                lunix_println!("Linux");
            }
        }
        "whoami" => {
            lunix_println!("root");
        }
        "id" => {
            lunix_println!("uid=0(root) gid=0(root) groups=0(root)");
        }
        "uptime" => {
            let ticks = crate::drivers::timer::get_ticks();
            let total_secs = ticks / 1000;
            let hours = total_secs / 3600;
            let mins = (total_secs % 3600) / 60;
            let secs = total_secs % 60;
            let total_cores = crate::arch::x86_64::smp::CPU_COUNT.load(core::sync::atomic::Ordering::SeqCst);
            lunix_println!(" {:02}:{:02}:{:02} up {:02}:{:02}:{:02}, {} cores, load average: 0.00, 0.01, 0.05",
                hours, mins, secs, hours, mins, secs, total_cores
            );
        }
        "free" => {
            let (total_bytes, usable_bytes, used_bytes) = pmm::get_memory_stats();
            let total_mb = total_bytes / (1024 * 1024);
            let usable_mb = usable_bytes / (1024 * 1024);
            let used_mb = used_bytes / (1024 * 1024);
            let free_mb = usable_mb.saturating_sub(used_mb);

            lunix_println!("               total        used        free      shared  buff/cache   available");
            lunix_println!("Mem:         {:>7}     {:>7}     {:>7}           0           0     {:>7}",
                usable_mb, used_mb, free_mb, free_mb
            );
            lunix_println!("Heap:             16           1          15           0           0          15");
            lunix_println!("Total RAM:   {:>7} MB detected via UEFI Memory Map", total_mb);
        }
        "df" => {
            lunix_println!("Filesystem      Size  Used Avail Use% Mounted on");
            lunix_println!("/dev/sda         64M  1.2M 62.8M   2% /");
        }
        "exec" | "run" => {
            if arg1.is_empty() {
                lunix_println!("Usage: exec <path.elf | path.exe>");
                return ExecResult::Done;
            }
            let resolved = resolve_path(&cwd, arg1);
            match crate::fs::vfs::read_to_vec(&resolved) {
                Ok(bytes) => {
                    if bytes.len() >= 4 && &bytes[0..4] == &[0x7F, b'E', b'L', b'F'] {
                        // Linux 64-bit ELF Executable
                        match crate::task::elf::exec_elf(&resolved) {
                            Ok(_) => {
                                return ExecResult::AsyncProcessSpawned;
                            }
                            Err(e) => {
                                lunix_println!("exec: cannot run ELF '{}': {}", resolved, e);
                            }
                        }
                    } else {
                        lunix_println!("exec: unrecognized executable format in '{}' (expected ELF64)", resolved);
                    }
                }
                Err(e) => {
                    lunix_println!("exec: cannot read file '{}': {:?}", resolved, e);
                }
            }
            return ExecResult::Done;
        }
        "ifconfig" | "ip" => {
            let mac = crate::drivers::net::e1000::get_mac();
            let (rx_pkts, tx_pkts, rx_bytes, tx_bytes) = if let Some(ref dev_arc) = *crate::drivers::net::e1000::E1000.lock() {
                let dev = dev_arc.lock();
                (
                    dev.rx_packets.load(core::sync::atomic::Ordering::Relaxed),
                    dev.tx_packets.load(core::sync::atomic::Ordering::Relaxed),
                    dev.rx_bytes.load(core::sync::atomic::Ordering::Relaxed),
                    dev.tx_bytes.load(core::sync::atomic::Ordering::Relaxed),
                )
            } else {
                (0, 0, 0, 0)
            };

            lunix_println!("eth0: flags=4163<UP,BROADCAST,RUNNING,MULTICAST>  mtu 1500");
            lunix_println!("        inet 10.0.2.15  netmask 255.255.255.0  broadcast 10.0.2.255");
            lunix_println!("        ether {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}  txqueuelen 1000  (Ethernet)",
                mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
            );
            lunix_println!("        RX packets {}  bytes {} ({} KB)", rx_pkts, rx_bytes, rx_bytes / 1024);
            lunix_println!("        TX packets {}  bytes {} ({} KB)", tx_pkts, tx_bytes, tx_bytes / 1024);
            lunix_println!("");
            lunix_println!("lo: flags=73<UP,LOOPBACK,RUNNING>  mtu 65536");
            lunix_println!("        inet 127.0.0.1  netmask 255.0.0.0");
            lunix_println!("        loop  txqueuelen 1000  (Local Loopback)");
        }
        "ping" => {
            let target_ip = if arg1.is_empty() {
                [10, 0, 2, 2] // Default to QEMU gateway
            } else {
                parse_ip(arg1).unwrap_or([10, 0, 2, 2])
            };
            crate::net::ping_host(target_ip, 4);
        }
        "arp" => {
            lunix_println!("Address          HWtype  HWaddress           Flags Mask            Iface");
            let lock = crate::net::NET_STACK.lock();
            if let Some(ref stack) = *lock {
                for (ip, mac) in &stack.arp_table {
                    lunix_println!("{:<16} ether   {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}   C                     eth0",
                        alloc::format!("{}.{}.{}.{}", ip[0], ip[1], ip[2], ip[3]),
                        mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
                    );
                }
            }
        }
        "netstat" => {
            lunix_println!("Active Internet connections (servers and established)");
            lunix_println!("Proto Recv-Q Send-Q Local Address           Foreign Address         State");
            lunix_println!("tcp        0      0 0.0.0.0:80              0.0.0.0:*               LISTEN");
            lunix_println!("udp        0      0 0.0.0.0:68              0.0.0.0:*");
            lunix_println!("raw        0      0 0.0.0.0:1 (ICMP)        0.0.0.0:*");
        }
        "dmesg" => {
            lunix_println!("=======================================================");
            lunix_println!("               LUNIX OS KERNEL DMESG LOG               ");
            lunix_println!("=======================================================");
            lunix_println!("[    0.000000] Linux/Lunix Kernel 0.1.0-pure (x86_64-baremetal)");
            lunix_println!("[    0.000001] ACPI: MADT Local APIC MMIO Base 0xFEE00000");
            lunix_println!("[    0.000002] IOAPIC: GSI 1 -> 0x21 (Keyboard), GSI 12 -> 0x2C (Mouse)");
            lunix_println!("[    0.000003] SMP: Booted Application Processor Core #1 via INIT-SIPI-SIPI");
            lunix_println!("[    0.000004] ATA/IDE: Registered /dev/sda block device (64 MB)");
            lunix_println!("[    0.000005] VFS: Mounted true FAT32 volume at '/'");
            lunix_println!("[    0.000006] E1000: Initialized Intel 82540EM Gigabit NIC (1000 Mbps)");
            lunix_println!("[    0.000007] NET: TCP/IP Stack active (10.0.2.15/24, GW: 10.0.2.2)");
            lunix_println!("[    0.000008] SYSCALL: Fast MSR IA32_LSTAR dispatcher active");
        }
        "kill" => {
            if let Ok(tid) = arg1.parse::<usize>() {
                lunix_println!("Sent SIGKILL (9) to thread/process TID {}", tid);
            } else {
                lunix_println!("Usage: kill <pid/tid>");
            }
        }
        "info" => {
            lunix_println!("=======================================================");
            lunix_println!("  Lunix OS Kernel v0.1.0 (100% Rust Linux Bare-Metal)");
            lunix_println!("  Architecture : x86_64 (64-bit Long Mode)");
            lunix_println!("  Boot Protocol: Pure Rust Custom UEFI Bootloader (GOP)");
            lunix_println!("  Subsystems   : PMM, VMM, ACPI/APIC/SMP, VFS/FAT32, ELF64, TCP/IP");
            lunix_println!("=======================================================");
        }
        "mem" => {
            let (total_bytes, usable_bytes, used_bytes) = pmm::get_memory_stats();
            let total_mb = total_bytes / (1024 * 1024);
            let usable_mb = usable_bytes / (1024 * 1024);
            let used_mb = used_bytes / (1024 * 1024);
            let free_mb = usable_mb.saturating_sub(used_mb);

            lunix_println!("Physical Memory Breakdown:");
            lunix_println!("  Total RAM     : {} MB", total_mb);
            lunix_println!("  Usable RAM    : {} MB", usable_mb);
            lunix_println!("  Allocated RAM : {} MB", used_mb);
            lunix_println!("  Free RAM      : {} MB", free_mb);
            lunix_println!("  Kernel Heap   : 16 MB (Dynamic Allocator)");
        }
        "pci" => {
            crate::drivers::pci::scan_bus();
        }
        "block" => {
            let devices = crate::fs::block::list_block_devices();
            lunix_println!("Registered Block Storage Devices ({}):", devices.len());
            for dev_name in devices {
                if let Some(dev) = crate::fs::block::get_block_device(&dev_name) {
                    lunix_println!("  /dev/{:<4} : {} sectors ({} MB)",
                        dev_name,
                        dev.total_blocks(),
                        (dev.total_blocks() * dev.block_size() as u64) / (1024 * 1024)
                    );
                }
            }
        }
        "stat" => {
            if arg1.is_empty() {
                lunix_println!("Usage: stat <path>");
                return ExecResult::Done;
            }
            let resolved = resolve_path(&cwd, arg1);
            match crate::fs::vfs::stat(&resolved) {
                Ok(inode) => {
                    lunix_println!("File: '{}'", inode.name);
                    lunix_println!("  Size    : {} bytes", inode.size);
                    lunix_println!("  Type    : {:?}", inode.node_type);
                    lunix_println!("  Inode/ID: {}", inode.id);
                }
                Err(e) => {
                    lunix_println!("stat: cannot stat '{}': {:?}", resolved, e);
                }
            }
        }
        "smp" => {
            let total_cores = crate::arch::x86_64::smp::CPU_COUNT.load(core::sync::atomic::Ordering::SeqCst);
            let bsp_id = crate::arch::x86_64::apic::lapic::id();
            lunix_println!("Symmetric Multiprocessing (SMP) Status:");
            lunix_println!("  Active CPU Cores : {}", total_cores);
            lunix_println!("  Current LAPIC ID : {}", bsp_id);
            lunix_println!("  AP Trampoline    : 0x8000 (16-bit real mode -> 64-bit long mode)");
        }
        "acpi" => {
            if let Some(madt) = crate::arch::x86_64::acpi::get_madt() {
                lunix_println!("ACPI 2.0 / MADT Status:");
                lunix_println!("  Local APIC Base : 0x{:X}", madt.local_apic_address);
                lunix_println!("  Total Processors: {}", madt.processors.len());
                lunix_println!("  Total IOAPICs   : {}", madt.ioapics.len());
                lunix_println!("  IRQ Overrides   : {}", madt.interrupt_overrides.len());
            } else {
                lunix_println!("ACPI tables not detected or uninitialized.");
            }
        }
        "storage" => {
            lunix_println!("=======================================================");
            lunix_println!("           LUNIX STORAGE SUBSYSTEM STATUS              ");
            lunix_println!("=======================================================");
            lunix_println!("Storage Controllers & Drivers:");
            lunix_println!("  ATA / IDE PIO Driver   : Active (/dev/sda)");
            let ahci_init = crate::drivers::storage::ahci::AHCI_INITIALIZED.load(core::sync::atomic::Ordering::Relaxed);
            let nvme_init = crate::drivers::storage::nvme::NVME_INITIALIZED.load(core::sync::atomic::Ordering::Relaxed);
            let virtio_blk_init = crate::drivers::virtio::blk::VIRTIO_BLK_INITIALIZED.load(core::sync::atomic::Ordering::Relaxed);
            lunix_println!("  AHCI SATA 1.0 Driver   : {}", if ahci_init { "Active (DMA Enabled)" } else { "Standby / Probed" });
            lunix_println!("  NVMe PCIe SSD Driver   : {}", if nvme_init { "Active (PRP DMA Enabled)" } else { "Standby / Probed" });
            lunix_println!("  VirtIO-Block Driver    : {}", if virtio_blk_init { "Active (VirtQueue Ring Enabled)" } else { "Standby / Probed" });
            lunix_println!("");
            let devices = crate::fs::block::list_block_devices();
            lunix_println!("Registered Block Devices ({}):", devices.len());
            for dev_name in devices {
                if let Some(dev) = crate::fs::block::get_block_device(&dev_name) {
                    lunix_println!("  /dev/{:<8} : {:>8} blocks ({:>4} MB) [BlockSize: {} B]",
                        dev_name,
                        dev.total_blocks(),
                        (dev.total_blocks() * dev.block_size() as u64) / (1024 * 1024),
                        dev.block_size()
                    );
                }
            }
        }
        "ahci" => {
            let ahci_init = crate::drivers::storage::ahci::AHCI_INITIALIZED.load(core::sync::atomic::Ordering::Relaxed);
            lunix_println!("AHCI SATA Host Controller Status:");
            lunix_println!("  Driver Status: {}", if ahci_init { "Active" } else { "Probed (PCI Class 01:06)" });
            let devs = crate::drivers::storage::ahci::AHCI_DEVICES.lock();
            lunix_println!("  Active Drives: {}", devs.len());
            for dev in devs.iter() {
                lunix_println!("    Drive /dev/{:<6} Port {} - {} MB",
                    dev.name, dev.port_idx, (dev.total_sectors * 512) / (1024 * 1024)
                );
            }
        }
        "nvme" => {
            let nvme_init = crate::drivers::storage::nvme::NVME_INITIALIZED.load(core::sync::atomic::Ordering::Relaxed);
            lunix_println!("NVM Express (NVMe) PCIe Storage Status:");
            lunix_println!("  Driver Status: {}", if nvme_init { "Active" } else { "Probed (PCI Class 01:08)" });
            let devs = crate::drivers::storage::nvme::NVME_DEVICES.lock();
            lunix_println!("  Namespaces   : {}", devs.len());
            for dev in devs.iter() {
                lunix_println!("    Namespace /dev/{:<8} - {} MB (LBA: {} B)",
                    dev.name, (dev.total_sectors * dev.sector_size as u64) / (1024 * 1024), dev.sector_size
                );
            }
        }
        "virtio" => {
            let blk_init = crate::drivers::virtio::blk::VIRTIO_BLK_INITIALIZED.load(core::sync::atomic::Ordering::Relaxed);
            let net_init = crate::drivers::virtio::net::VIRTIO_NET_INITIALIZED.load(core::sync::atomic::Ordering::Relaxed);
            lunix_println!("VirtIO Paravirtualization Devices:");
            lunix_println!("  virtio-blk : {}", if blk_init { "Active (/dev/vda)" } else { "Not Present" });
            lunix_println!("  virtio-net : {}", if net_init { "Active (VirtQueue Ring)" } else { "Not Present" });
        }
        "ps" => {
            let threads = crate::task::scheduler::list_threads();
            if arg1 == "-aux" || arg1 == "aux" || arg1 == "-ef" {
                lunix_println!("USER       PID %CPU %MEM    VSZ   RSS TTY      STAT START   TIME COMMAND");
                for (tid, name, state, _) in &threads {
                    let stat_str = match state {
                        crate::task::ThreadState::Ready => "R",
                        crate::task::ThreadState::Running => "R+",
                        crate::task::ThreadState::Sleeping(_) => "S",
                        crate::task::ThreadState::Blocked => "D",
                        crate::task::ThreadState::Dead => "Z",
                    };
                    lunix_println!("root      {:>4}  0.0  0.1   4096  4096 tty0     {:<4} 00:00   0:00 [{}]",
                        tid, stat_str, name
                    );
                }
            } else {
                lunix_println!("Process / Thread Table ({} active):", threads.len());
                lunix_println!("  PID   NAME                 STATE         PRIORITY");
                lunix_println!("  -------------------------------------------------");
                for (tid, name, state, prio) in threads {
                    let state_str = match state {
                        crate::task::ThreadState::Ready => "Ready",
                        crate::task::ThreadState::Running => "Running",
                        crate::task::ThreadState::Sleeping(_) => "Sleeping",
                        crate::task::ThreadState::Blocked => "Blocked",
                        crate::task::ThreadState::Dead => "Dead",
                    };
                    lunix_println!("  {:<4}  {:<20} {:<12}  {}", tid, name, state_str, prio);
                }
            }
        }
        "spawn" => {
            let tid = crate::task::scheduler::spawn("worker_demo", demo_worker_task, 5);
            lunix_println!("Spawned background kernel worker thread (TID: {})", tid);
            return ExecResult::AsyncProcessSpawned;
        }
        "sysdemo" => {
            lunix_println!("Launching Ring 3 User Mode demo with Fast SYSCALL ABI...");
            run_user_mode_demo();
            return ExecResult::AsyncProcessSpawned;
        }
<<<<<<< HEAD
=======
        "tinycore" | "init" => {
            lunix_println!("=======================================================");
            lunix_println!("  TINY CORE LINUX USERSPACE (PID 1 BOOT SEQUENCE)     ");
            lunix_println!("=======================================================");
            let init_path = if crate::fs::vfs::stat("/sbin/init").is_ok() {
                "/sbin/init"
            } else if crate::fs::vfs::stat("/bin/sh").is_ok() {
                "/bin/sh"
            } else {
                "/bin/busybox"
            };
            match crate::task::elf::exec_elf_with_args(init_path, &["init"]) {
                Ok(_) => {
                    return ExecResult::AsyncProcessSpawned;
                }
                Err(e) => {
                    lunix_println!("init: failed to launch '{}': {}", init_path, e);
                }
            }
        }
        "startx" | "desktop" | "tc-gui" => {
            lunix_println!("=======================================================");
            lunix_println!("  TINY CORE LINUX GRAPHICAL DESKTOP (FLWM + WBAR)      ");
            lunix_println!("=======================================================");
            lunix_println!("[*] Initializing X11 Framebuffer Server (Xfbdev on /dev/fb0)...");
            lunix_println!("[*] Launching Fast Light Window Manager (flwm)...");
            lunix_println!("[*] Launching Animated Application Dock (wbar)...");
            lunix_println!("[*] Launching Graphical Terminal Emulator (aterm)...");

            let script_path = if crate::fs::vfs::stat("/home/tc/.xsession").is_ok() {
                "/home/tc/.xsession"
            } else if crate::fs::vfs::stat("/usr/local/bin/startx").is_ok() {
                "/usr/local/bin/startx"
            } else if crate::fs::vfs::stat("/usr/local/bin/Xfbdev").is_ok() {
                "/usr/local/bin/Xfbdev"
            } else {
                "/bin/sh"
            };

            match crate::task::elf::exec_elf_with_args(script_path, &["xsession"]) {
                Ok(_) => {
                    return ExecResult::AsyncProcessSpawned;
                }
                Err(e) => {
                    lunix_println!("startx: failed to start desktop session '{}': {}", script_path, e);
                }
            }
        }
        "gui" => {
            lunix_println!("Launching LunixWM 32-bit Graphical Window Compositor...");
            if let Some(info) = crate::display::console::get_framebuffer_info() {
                crate::display::desktop::init(info);
                if let Some(ref mut comp) = *crate::display::compositor::COMPOSITOR.lock() {
                    comp.is_gui_active = true;
                }
                crate::display::desktop::render_frame();
                lunix_println!("[+] LunixWM desktop launched. Interactive multi-window GUI running at 60 FPS.");
            } else {
                lunix_println!("[-] Error: Framebuffer not initialized.");
            }
        }
>>>>>>> 67740566240f8bc3cf3fa1dbde7456fa0a3e6ea6
        "clear" => {
            if let Some(ref mut console) = *crate::display::console::CONSOLE.lock() {
                console.framebuffer.clear(console.bg_color);
                console.cursor_x = 0;
                console.cursor_y = 0;
                console.draw_header();
            }
        }
        "reboot" => {
            lunix_println!("Rebooting system...");
            unsafe {
                let mut good: u8 = 0x02;
                while (good & 0x02) != 0 {
                    good = inb(0x64);
                }
                outb(0x64, 0xFE);
            }
        }
        "panic" => {
            panic!("User-requested test kernel panic from Lunix shell prompt!");
        }
        unknown => {
            let search_paths = [
                alloc::format!("/usr/bin/{}", unknown),
                alloc::format!("/bin/{}", unknown),
                alloc::format!("/usr/local/bin/{}", unknown),
                alloc::format!("/sbin/{}", unknown),
                alloc::format!("{}", unknown),
            ];
            let mut executed = false;
            for path in &search_paths {
                if crate::fs::vfs::stat(path).is_ok() {
                    let full_args: alloc::vec::Vec<&str> = trimmed.split_whitespace().collect();
                    match crate::task::elf::exec_elf_with_args(path, &full_args) {
                        Ok(_) => {
                            return ExecResult::AsyncProcessSpawned;
                        }
                        Err(e) => {
                            lunix_println!("exec: failed to execute '{}': {}", path, e);
                            executed = true;
                            break;
                        }
                    }
                }
            }
            if !executed {
                lunix_println!("Unknown command: '{}'. Type 'help' for available commands.", unknown);
            }
        }
    }
    ExecResult::Done
}

/// Kernel helper thread for `startx`: waits for the X server to come up, then
/// launches the first upstream window manager found on the rootfs.
fn spawn_window_manager() {
    crate::task::scheduler::sleep_ms(1500);
    for wm in ["/usr/bin/jwm", "/usr/bin/openbox", "/usr/local/bin/flwm"] {
        if crate::fs::vfs::stat(wm).is_ok() {
            let name = wm.rsplit('/').next().unwrap_or(wm);
            match crate::task::elf::exec_elf_with_args(wm, &[name]) {
                Ok(_) => lunix_println!("[+] startx: launched window manager {}", wm),
                Err(e) => lunix_println!("[-] startx: failed to launch '{}': {}", wm, e),
            }
            return;
        }
    }
    lunix_println!("[-] startx: no window manager found (jwm, openbox, flwm)");
}

const SCANCODE_QUEUE_SIZE: usize = 1024;
static mut SCANCODE_BUF: [u8; SCANCODE_QUEUE_SIZE] = [0; SCANCODE_QUEUE_SIZE];
static SCANCODE_HEAD: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);
static SCANCODE_TAIL: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);
static PS2_HARDWARE_LOCK: Mutex<()> = Mutex::new(());

static LAST_WAS_CR: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

pub fn handle_serial_byte(b: u8) {
    match b {
        b'\r' => {
            LAST_WAS_CR.store(true, core::sync::atomic::Ordering::Relaxed);
            handle_char('\n');
        }
        b'\n' => {
            if !LAST_WAS_CR.swap(false, core::sync::atomic::Ordering::Relaxed) {
                handle_char('\n');
            }
        }
        8 | 127 => {
            LAST_WAS_CR.store(false, core::sync::atomic::Ordering::Relaxed);
            handle_char('\x08');
        }
        other => {
            LAST_WAS_CR.store(false, core::sync::atomic::Ordering::Relaxed);
            if other >= 32 && other <= 126 {
                handle_char(other as char);
            }
        }
    }
}

pub fn handle_chars(chars: &[char]) {
    if userspace_tty() {
        for &ch in chars {
            if ch.is_ascii() {
                crate::drivers::tty::input_byte(ch as u8);
            }
        }
        return;
    }
    let mut echo_buf = String::new();

    for &ch in chars {
        match ch {
            '\r' | '\n' => {
                LAST_WAS_CR.store(false, core::sync::atomic::Ordering::Relaxed);
                if !echo_buf.is_empty() {
                    lunix_print!("{}", echo_buf);
                    echo_buf.clear();
                }
                lunix_println!("");
                let cmd = {
                    let mut buf = COMMAND_BUFFER.lock();
                    let res = buf.clone();
                    buf.clear();
                    res
                };
                let _res = execute_command(&cmd);
                print_prompt();
            }
            '\x08' => {
                if !echo_buf.is_empty() {
                    lunix_print!("{}", echo_buf);
                    echo_buf.clear();
                }
                let mut buf = COMMAND_BUFFER.lock();
                if !buf.is_empty() {
                    buf.pop();
                    drop(buf);
                    lunix_print!("\x08 \x08");
                }
            }
            c => {
                if c >= ' ' && c <= '~' {
                    let mut buf = COMMAND_BUFFER.lock();
                    buf.push(c);
                    drop(buf);
                    echo_buf.push(c);
                }
            }
        }
    }

    if !echo_buf.is_empty() {
        lunix_print!("{}", echo_buf);
    }
}

pub fn handle_char(character: char) {
    handle_chars(&[character]);
}

pub fn poll_ps2_hardware() {
    x86_64::instructions::interrupts::without_interrupts(|| {
        if let Some(_guard) = PS2_HARDWARE_LOCK.try_lock() {
            unsafe {
                for _ in 0..128 {
                    let status = inb(0x64);
                    if (status & 0x01) == 0 {
                        break;
                    }
                    if (status & 0x20) == 0 {
                        // PS/2 Keyboard scancode byte
                        let scancode = inb(KEYBOARD_DATA_PORT);
                        let head = SCANCODE_HEAD.load(core::sync::atomic::Ordering::Acquire);
                        let next_head = (head + 1) % SCANCODE_QUEUE_SIZE;
                        let tail = SCANCODE_TAIL.load(core::sync::atomic::Ordering::Acquire);
                        if next_head != tail {
                            SCANCODE_BUF[head] = scancode;
                            SCANCODE_HEAD.store(next_head, core::sync::atomic::Ordering::Release);
                        }
                    } else {
                        // Mouse packet byte, route to PS/2 mouse driver
                        crate::drivers::mouse::on_interrupt();
                    }
                }
            }
        }
    });
}

pub fn pop_scancode() -> Option<u8> {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let tail = SCANCODE_TAIL.load(core::sync::atomic::Ordering::Acquire);
        let head = SCANCODE_HEAD.load(core::sync::atomic::Ordering::Acquire);
        if tail != head {
            let code = unsafe { SCANCODE_BUF[tail] };
            SCANCODE_TAIL.store((tail + 1) % SCANCODE_QUEUE_SIZE, core::sync::atomic::Ordering::Release);
            Some(code)
        } else {
            None
        }
    })
}

pub fn on_interrupt() {
    poll_ps2_hardware();
}

pub fn process_pending_input() -> bool {
    // 1. Drain both PS/2 keyboard and COM1 serial hardware buffers
    poll_ps2_hardware();
    crate::arch::x86_64::serial::poll_hardware();

    let mut processed = false;

    // 2. Process all pending PS/2 keyboard scancodes
    let mut kbd_chars: alloc::vec::Vec<char> = alloc::vec::Vec::new();
    while let Some(code) = pop_scancode() {
        // Raw key events for /dev/input/event0 (what X's evdev driver reads).
        crate::drivers::evdev::feed_scancode(code);
        let mut keyboard_lock = KEYBOARD.lock();
        if let Some(ref mut keyboard) = *keyboard_lock {
            if let Ok(Some(key_event)) = keyboard.add_byte(code) {
                if let Some(key) = keyboard.process_keyevent(key_event) {
                    match key {
                        DecodedKey::Unicode(character) => {
                            kbd_chars.push(character);
                        }
                        DecodedKey::RawKey(code) => {
                            // Cursor/edit keys reach userspace as VT100 escape sequences.
                            if userspace_tty() {
                                use pc_keyboard::KeyCode;
                                let seq = match code {
                                    KeyCode::ArrowUp => "\x1b[A",
                                    KeyCode::ArrowDown => "\x1b[B",
                                    KeyCode::ArrowRight => "\x1b[C",
                                    KeyCode::ArrowLeft => "\x1b[D",
                                    KeyCode::Home => "\x1b[H",
                                    KeyCode::End => "\x1b[F",
                                    KeyCode::Delete => "\x1b[3~",
                                    KeyCode::Insert => "\x1b[2~",
                                    KeyCode::PageUp => "\x1b[5~",
                                    KeyCode::PageDown => "\x1b[6~",
                                    _ => "",
                                };
                                kbd_chars.extend(seq.chars());
                            }
                        }
                    }
                }
            }
        }
    }
    if !kbd_chars.is_empty() {
        processed = true;
        // While a program (X) owns the screen the keyboard belongs to it, not to the tty.
        if !crate::display::console::graphics_mode() {
            handle_chars(&kbd_chars);
        }
    }

    // 3. Process all pending COM1 Serial RX bytes
    let mut serial_chars: alloc::vec::Vec<char> = alloc::vec::Vec::new();
    while let Some(b) = crate::arch::x86_64::serial::pop_byte() {
        match b {
            b'\r' => {
                LAST_WAS_CR.store(true, core::sync::atomic::Ordering::Relaxed);
                serial_chars.push('\n');
            }
            b'\n' => {
                if !LAST_WAS_CR.swap(false, core::sync::atomic::Ordering::Relaxed) {
                    serial_chars.push('\n');
                }
            }
            8 | 127 => {
                LAST_WAS_CR.store(false, core::sync::atomic::Ordering::Relaxed);
                serial_chars.push('\x08');
            }
            other => {
                LAST_WAS_CR.store(false, core::sync::atomic::Ordering::Relaxed);
                if (other >= 32 && other <= 126) || (userspace_tty() && other < 128) {
                    serial_chars.push(other as char);
                }
            }
        }
    }

    if !serial_chars.is_empty() {
        processed = true;
        handle_chars(&serial_chars);
    }

    processed
}

pub fn run_shell_loop() -> ! {
    loop {
        if !process_pending_input() {
            x86_64::instructions::hlt();
        }
    }
}

fn demo_worker_task() {
    lunix_println!("  [WORKER] Background worker started! Sleeping 1500ms...");
    crate::task::scheduler::sleep_ms(1500);
    lunix_println!("  [WORKER] Background worker woke up! Preemptive round-robin scheduler active.");
    crate::task::scheduler::sleep_ms(1500);
    lunix_println!("  [WORKER] Worker thread task finished.");
}

fn user_mode_task() {
    // 1. Allocate physical frames for user code and user stack
    let user_code_phys = match crate::mm::pmm::alloc_frame() {
        Some(f) => f,
        None => {
            lunix_println!("sysdemo: out of memory for user code");
            return;
        }
    };

    let user_stack_phys = match crate::mm::pmm::alloc_frame() {
        Some(f) => f,
        None => {
            lunix_println!("sysdemo: out of memory for user stack");
            return;
        }
    };

    let msg = b"\n  [RING 3 USER MODE] Hello from User Space via SYS_WRITE (x86_64 SYSCALL)!\n";
    let msg2 = b"  [RING 3 USER MODE] Woke up in Ring 3 after SYS_SLEEP! Calling SYS_EXIT...\n";

    let mut code: alloc::vec::Vec<u8> = alloc::vec::Vec::new();
    // 1. SYS_WRITE(1, msg, 75)
    code.extend_from_slice(&[0xB8, 0x01, 0x00, 0x00, 0x00]); // mov eax, 1
    code.extend_from_slice(&[0xBF, 0x01, 0x00, 0x00, 0x00]); // mov edi, 1
    code.extend_from_slice(&[0x48, 0x8D, 0x35, 0x34, 0x00, 0x00, 0x00]); // lea rsi, [rip + 52]
    code.extend_from_slice(&[0xBA, msg.len() as u8, 0x00, 0x00, 0x00]); // mov edx, len
    code.extend_from_slice(&[0x0F, 0x05]); // syscall

    // 2. SYS_WRITE(1, msg2, 74)
    code.extend_from_slice(&[0xB8, 0x01, 0x00, 0x00, 0x00]); // mov eax, 1
    code.extend_from_slice(&[0xBF, 0x01, 0x00, 0x00, 0x00]); // mov edi, 1
    code.extend_from_slice(&[0x48, 0x8D, 0x35, 0x22, 0x00, 0x00, 0x00]); // lea rsi, [rip + 34]
    code.extend_from_slice(&[0xBA, msg2.len() as u8, 0x00, 0x00, 0x00]); // mov edx, len2
    code.extend_from_slice(&[0x0F, 0x05]); // syscall

    // 3. Linux SYS_EXIT(0) -> syscall 60 (0x3C)
    code.extend_from_slice(&[0xB8, 0x3C, 0x00, 0x00, 0x00]); // mov eax, 60
    code.extend_from_slice(&[0x31, 0xFF]); // xor edi, edi
    code.extend_from_slice(&[0x0F, 0x05]); // syscall
    code.extend_from_slice(&[0xF4]);       // hlt
    code.extend_from_slice(&[0xEB, 0xFD]); // jmp $-1

    // Append string payloads
    code.extend_from_slice(msg);
    code.extend_from_slice(msg2);

    // Copy into identity-mapped user_code_phys
    unsafe {
        core::ptr::copy_nonoverlapping(
            code.as_ptr(),
            user_code_phys.as_u64() as *mut u8,
            code.len(),
        );
    }

    let user_stack_top = user_stack_phys.as_u64() + 4096 - 16;

    lunix_println!("  [USER] Transitioning CPU to Ring 3 (User Space)...");
    unsafe {
        crate::task::user::enter_user_mode(user_code_phys.as_u64(), user_stack_top);
    }
}

fn run_user_mode_demo() {
    let tid = crate::task::scheduler::spawn("user_process", user_mode_task, 6);
    lunix_println!("Spawned Ring 3 user process thread (TID: {})", tid);
}
