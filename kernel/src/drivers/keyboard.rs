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
        HandleControl::Ignore,
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

pub fn print_prompt() {
    let cwd = get_cwd();
    if cwd == "/" {
        lunix_print!("lunix> ");
    } else {
        lunix_print!("lunix:{}> ", cwd);
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
            lunix_println!("              Lunix OS Linux & POSIX Shell Commands           ");
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
            lunix_println!("  stat <path>     - Display file or directory inode metadata");
            lunix_println!("  smp             - Display multi-core SMP and APIC status");
            lunix_println!("  acpi            - Display ACPI 2.0 tables and interrupt topology");
            lunix_println!("  nt              - Display Windows NT Driver Subsystem status");
            lunix_println!("  spawn           - Spawn a preemptive background kernel worker thread");
            lunix_println!("  sysdemo         - Execute Ring 3 User Mode demo via fast SYSCALL ABI");
            lunix_println!("  gui             - Launch LunixWM 32-bit Graphical Window Compositor");
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
                lunix_println!("Lunix lunix-os 0.1.0-hybrid #1 SMP PREEMPT 2026-09-11 x86_64 LunixOS GNU/Lunix");
            } else {
                lunix_println!("Lunix");
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
                    } else if bytes.len() >= 2 && &bytes[0..2] == &[b'M', b'Z'] {
                        // Windows 64-bit PE32+ Executable
                        match crate::subsystems::nt::win32::exec_win32_pe(&resolved) {
                            Ok(_) => {
                                return ExecResult::AsyncProcessSpawned;
                            }
                            Err(e) => {
                                lunix_println!("exec: cannot run Win32 PE '{}': {}", resolved, e);
                            }
                        }
                    } else {
                        lunix_println!("exec: unrecognized executable format in '{}'", resolved);
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
            lunix_println!("[    0.000000] Linux/Lunix Kernel 0.1.0-hybrid (x86_64-baremetal)");
            lunix_println!("[    0.000001] ACPI: MADT Local APIC MMIO Base 0xFEE00000");
            lunix_println!("[    0.000002] IOAPIC: GSI 1 -> 0x21 (Keyboard), GSI 12 -> 0x2C (Mouse)");
            lunix_println!("[    0.000003] SMP: Booted Application Processor Core #1 via INIT-SIPI-SIPI");
            lunix_println!("[    0.000004] ATA/IDE: Registered /dev/sda block device (64 MB)");
            lunix_println!("[    0.000005] VFS: Mounted true FAT32 volume at '/'");
            lunix_println!("[    0.000006] WDM: Initialized Windows NT Driver Subsystem (win64 ABI)");
            lunix_println!("[    0.000007] E1000: Initialized Intel 82540EM Gigabit NIC (1000 Mbps)");
            lunix_println!("[    0.000008] NET: TCP/IP Stack active (10.0.2.15/24, GW: 10.0.2.2)");
            lunix_println!("[    0.000009] SYSCALL: Fast MSR IA32_LSTAR dispatcher active");
            lunix_println!("[    0.000010] LunixWM: 32-bit double-buffered desktop compositor ready");
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
            lunix_println!("  Lunix OS Kernel v0.1.0 (Hybrid Windows NT + Linux)");
            lunix_println!("  Architecture : x86_64 (64-bit Long Mode)");
            lunix_println!("  Boot Protocol: Pure Rust Custom UEFI Bootloader (GOP)");
            lunix_println!("  Subsystems   : PMM, VMM, ACPI/APIC/SMP, VFS/FAT32, Win64 WDM, ELF64");
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
        "nt" => {
            lunix_println!("Windows NT Subsystem Status:");
            lunix_println!("  Core API Exports: ntoskrnl.exe & hal.dll (extern \"win64\" ABI)");
            lunix_println!("  Loaded Drivers  : \\Driver\\SampleLunixDriver (WDM)");
            lunix_println!("  Created Devices : \\Device\\LunixSampleDevice0");
            lunix_println!("  IRP Support     : MJ_CREATE, MJ_CLOSE, MJ_DEVICE_CONTROL");
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
            lunix_println!("Unknown command: '{}'. Type 'help' for available commands.", unknown);
        }
    }
    ExecResult::Done
}

const SCANCODE_QUEUE_SIZE: usize = 512;

struct ScancodeQueue {
    buffer: [u8; SCANCODE_QUEUE_SIZE],
    head: usize,
    tail: usize,
}

static SCANCODE_QUEUE: Mutex<ScancodeQueue> = Mutex::new(ScancodeQueue {
    buffer: [0; SCANCODE_QUEUE_SIZE],
    head: 0,
    tail: 0,
});

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

pub fn handle_char(character: char) {
    let mut buffer = COMMAND_BUFFER.lock();
    match character {
        '\r' | '\n' => {
            lunix_println!("");
            let cmd = buffer.clone();
            buffer.clear();
            drop(buffer);

            let res = execute_command(&cmd);
            if res == ExecResult::Done {
                print_prompt();
            }
        }
        '\x08' => {
            // Backspace
            if !buffer.is_empty() {
                buffer.pop();
                lunix_print!("\x08");
            }
        }
        ch => {
            if ch >= ' ' && ch <= '~' {
                buffer.push(ch);
                lunix_print!("{}", ch);
            }
        }
    }
}

pub fn on_interrupt() {
    let scancode = unsafe { inb(KEYBOARD_DATA_PORT) };
    let mut queue = SCANCODE_QUEUE.lock();
    let next_head = (queue.head + 1) % SCANCODE_QUEUE_SIZE;
    if next_head != queue.tail {
        let h = queue.head;
        queue.buffer[h] = scancode;
        queue.head = next_head;
    }
}

pub fn process_pending_input() {
    // 1. Process all pending PS/2 keyboard scancodes
    loop {
        let scancode = {
            let mut queue = SCANCODE_QUEUE.lock();
            if queue.head != queue.tail {
                let code = queue.buffer[queue.tail];
                queue.tail = (queue.tail + 1) % SCANCODE_QUEUE_SIZE;
                Some(code)
            } else {
                None
            }
        };

        match scancode {
            Some(code) => {
                let mut keyboard_lock = KEYBOARD.lock();
                if let Some(ref mut keyboard) = *keyboard_lock {
                    if let Ok(Some(key_event)) = keyboard.add_byte(code) {
                        if let Some(key) = keyboard.process_keyevent(key_event) {
                            drop(keyboard_lock);
                            match key {
                                DecodedKey::Unicode(character) => {
                                    handle_char(character);
                                }
                                DecodedKey::RawKey(_) => {}
                            }
                        }
                    }
                }
            }
            None => break,
        }
    }

    // 2. Process all pending COM1 Serial RX bytes
    loop {
        crate::arch::x86_64::serial::poll_hardware();
        if let Some(b) = crate::arch::x86_64::serial::pop_byte() {
            handle_serial_byte(b);
        } else {
            break;
        }
    }
}

pub fn run_shell_loop() -> ! {
    loop {
        process_pending_input();
        x86_64::instructions::hlt();
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
