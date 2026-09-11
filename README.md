# Lunix: Next-Generation Pure Rust Hybrid Operating System Kernel

**Lunix** is an independent, from-scratch 64-bit hybrid operating system kernel, window compositor, and UEFI bootloader written in 100% pure Rust (`#![no_std]`, `#![no_main]`). It combines the architectural strengths of **Windows** (NT WDM driver model, PE32+ loader, Win64 ABI, rich GUI window manager) and **Linux** (POSIX/Linux syscalls, ELF64 user binary execution, VFS, standard Linux shell suite, and BSD socket networking).

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                 LunixWM 32-bit Graphical Window Compositor                  │
│       (Glassmorphic Chrome • Drop Shadows • Taskbar • PS/2 Mouse Drag)       │
│  ┌───────────────────────┐   ┌───────────────────────────┐   ┌───────────┐  │
│  │ System Resource Mon   │   │ NT Driver Subsystem Mgr   │   │  FAT32 VFS│  │
│  └───────────────────────┘   └───────────────────────────┘   └───────────┘  │
├─────────────────────────────────────────────────────────────────────────────┤
│                 Ring 3 User Mode & Linux Syscall ABI Layer                  │
│          (Fast SYSCALL MSR • ELF64 Loader • BSD Sockets • POSIX VFS)        │
│  ┌───────────────────────────┐           ┌───────────────────────────────┐  │
│  │ Standalone Linux Binaries │           │   Standard Linux Shell Suite  │  │
│  │ (/bin/hello, sysinfo.elf) │           │ (ls, cat, cd, ps, ifconfig)  │  │
│  └───────────────────────────┘           └───────────────────────────────┘  │
├─────────────────────────────────────────────────────────────────────────────┤
│                 Windows .SYS Drivers & NT Subsystem                         │
│       (PE32+ Loader • Relocator • ntoskrnl.exe & hal.dll • Win64 ABI)       │
├─────────────────────────────────────────────────────────────────────────────┤
│                    Lunix Core Hybrid Kernel & VFS                           │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │ Preemptive Round-Robin Scheduler & Task Context Switcher (switch_to)  │  │
│  │ Virtual Filesystem (VFS) & FAT32 Block Driver (/dev/sda)              │  │
│  │ Intel 82540EM (e1000) Gigabit NIC Driver & TCP/IP Protocol Stack      │  │
│  │ Memory Mgmt (PMM Bitmap 4GB, 4-Level VMM Huge Paging, 16MB Heap)      │  │
│  │ ACPI 2.0 Parser (MADT) • Local APIC (1000 Hz Timer) • IOAPIC Router   │  │
│  │ SMP Multi-Core AP Trampoline (0x8000 Real Mode -> 64-bit Long Mode)   │  │
│  │ CPU Management (GDT DPL0/3, TSS RSP0/IST) • Lockless UART Handlers    │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
├─────────────────────────────────────────────────────────────────────────────┤
│          Lunix Custom Rust UEFI Bootloader (lunix-bootloader.efi)           │
│       (GOP Framebuffer • Memory Map • ACPI RSDP • ELF64 Segment Mapper)     │
├─────────────────────────────────────────────────────────────────────────────┤
│             Hardware (x86_64 Multi-Core / APIC / Intel e1000 NIC)           │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Key Features

1. **Custom Rust UEFI Bootloader (`lunix-bootloader`)**:
   - Written in pure Rust targeting `x86_64-unknown-uefi`.
   - Locates Graphics Output Protocol (GOP) for high-resolution 32-bit linear framebuffers.
   - Parses the FAT32 volume to load and parse ELF64 segments into target physical RAM.
   - Discovers ACPI 2.0 RSDP and passes strongly-typed `BootInfo` to the kernel entry point.

2. **SMP Multi-Core & APIC Architecture (`arch/x86_64`)**:
   - ACPI 2.0 MADT parser discovering all physical and logical CPU cores.
   - Local APIC configuration with calibrated 1000 Hz (1ms) periodic timer.
   - IOAPIC routing hardware interrupts (IRQ1 Keyboard -> 0x21, IRQ12 Mouse -> 0x2C).
   - Real-mode 16-bit SMP AP trampoline at `0x8000` waking secondary cores via INIT-SIPI-SIPI.

3. **Storage & Virtual Filesystem (VFS) (`fs`)**:
   - Unified `BlockDevice` abstraction layer.
   - Primary/Secondary IDE/ATA PIO controller driver registering `/dev/sda` (64 MB disk).
   - Full FAT32 driver parsing BIOS Parameter Block (BPB), FAT cluster chains, and directory entries mounted to `/`.

4. **Preemptive Multitasking & Scheduler (`task`, `sync`)**:
   - Thread Control Blocks (TCBs) with state machines (`Ready`, `Running`, `Sleeping`, `Blocked`, `Dead`).
   - Naked assembly `context_switch` preserving callee-saved registers and switching stack pointers.
   - Priority round-robin scheduling with timer ticks and synchronization primitives (`Mutex`, `Semaphore`, `WaitQueue`).

5. **Ring 3 User Mode & Linux Syscall ABI (`syscall`, `task/elf`)**:
   - User code and data segment descriptors (DPL=3) loaded with `iretq`.
   - Fast MSR `syscall`/`sysret` configuration (`IA32_STAR`, `IA32_LSTAR`, `IA32_FMASK`).
   - Dedicated 16 KiB isolated kernel syscall stack.
   - Comprehensive Linux x86_64 syscalls (`sys_clone`, `sys_fork`, `sys_execve`, `sys_wait4`, `sys_getdents64`, `sys_openat`, `sys_pipe2`, `sys_dup2`, `sys_read`, `sys_write`, `sys_open`, `sys_close`, `sys_stat`, `sys_mmap`, `sys_brk`, `sys_sched_yield`, `sys_nanosleep`, `sys_socket`, `sys_connect`, `sys_sendto`, `sys_recvfrom`, `sys_exit`, `sys_exit_group`, `sys_uname`, `sys_getcwd`, `sys_chdir`).
   - Standalone 64-bit ELF binary loader executing userspace programs directly from the disk image (`/bin/hello.elf`, `/bin/sysinfo.elf`).

6. **Windows NT & Win32 User-Mode Subsystem (`subsystems/nt`)**:
   - 64-bit PE32+ parser, section mapper, base relocator (`DIR64`), and IAT resolver.
   - Native Rust `extern "win64"` ABI calling convention for Windows callbacks.
   - Kernel Driver Layer (WDM): `ntoskrnl.exe` and `hal.dll` DDI shims handling `IoCreateDevice`, `IoCompleteRequest`, `IoCallDriver`, `ExAllocatePoolWithTag`, `DbgPrint`, and port I/O.
   - User-Mode Win32 Layer: `kernel32.dll` and `ntdll.dll` API shims (`GetStdHandle`, `WriteConsoleA`, `WriteFile`, `ReadFile`, `ExitProcess`, `GetProcessHeap`, `HeapAlloc`, `HeapFree`, `VirtualAlloc`, `GetSystemInfo`, `Sleep`).
   - Universal Binary Launcher (`exec`): Runs both Linux ELF64 binaries (`.elf`) and Windows PE32+ executables (`/bin/win_hello.exe`) natively in Ring 3.

7. **Intel 82540EM (e1000) Gigabit NIC & TCP/IP Network Stack (`drivers/net`, `net`)**:
   - PCI bus mastering, 128 KiB BAR0 MMIO space, and EEPROM MAC address discovery.
   - 64 Transmit & 64 Receive circular DMA descriptor rings.
   - Ethernet II framing, dynamic ARP cache & requests, IPv4 packet routing & checksums.
   - ICMP Ping client with millisecond round-trip time latency calculation.
   - BSD socket management state machine.

8. **Graphical Compositor & LunixWM (`display`)**:
   - 32-bit double-buffered software compositor with hardware VRAM blit.
   - Cybernetic grid gradient wallpaper, glassmorphic window chrome, drop shadows, and top status bar.
   - PS/2 mouse driver with 3-byte packet streaming, pointer movement, and window dragging.
   - Built-in multi-window application suite (System Monitor, NT Driver Manager, File Explorer, Terminal).

---

## Interactive Linux & Lunix Shell Commands

The kernel boots into an interactive console with a stateful bash-style current working directory (`CWD`) and path resolution:

- `help`: Lists all Linux and Lunix shell commands
- `ls [-l] [path]`: Lists directory contents with permissions (`drwxr-xr-x`), sizes, and names
- `cat <path>`: Displays file text contents (e.g. `cat /etc/os-release`, `cat /home/lunix/readme.txt`)
- `cd [path]`: Changes current working directory (`.`, `..`, `/`, and subdirectories)
- `pwd`: Prints current working directory
- `echo [text]`: Prints arguments to stdout
- `uname [-a]`: Displays OS name, release (`0.1.0-hybrid`), kernel timestamp, and architecture
- `whoami`: Displays `root`
- `id`: Displays `uid=0(root) gid=0(root) groups=0(root)`
- `uptime`: Displays system uptime calculated from 1000 Hz APIC timer ticks and CPU core count
- `free [-m]`: Displays physical RAM, usable RAM, free RAM, and heap memory table
- `df [-h]`: Displays filesystem disk space usage for `/dev/sda` mounted at `/`
- `ps [-aux]`: Lists active processes/threads, TID/PID, TTY, STAT, and COMMAND
- `kill <tid>`: Sends terminate signal to a thread/process
- `dmesg`: Displays kernel boot logs and hardware topology ring buffer
- `exec <path.elf>`: Loads and executes a standalone 64-bit ELF user binary from disk in Ring 3 user mode!
- `ifconfig` / `ip`: Displays network interfaces (`eth0`), MAC address, IP address (`10.0.2.15`), netmask, and packet/byte counters
- `ping <ip>`: Sends ICMP Echo Requests and calculates round-trip time (RTT) latency
- `arp`: Displays the ARP cache table mapping IP addresses to hardware MAC addresses
- `netstat`: Displays active BSD socket connections, listening ports, and protocols
- `info`: Displays OS kernel and CPU architecture info
- `mem`: Live breakdown of physical RAM (PMM/VMM)
- `pci`: Scans and prints PCI bus hardware devices
- `block`: Lists registered block devices (`/dev/sda`, etc.)
- `stat <path>`: Displays file or directory metadata
- `smp`: Displays multi-core SMP and APIC status
- `acpi`: Displays ACPI 2.0 tables and interrupt topology
- `nt`: Displays Windows NT Subsystem and loaded driver status
- `spawn`: Spawns a preemptive background kernel worker thread
- `sysdemo`: Executes Ring 3 User Mode demo via fast SYSCALL ABI
- `gui`: Launches `LunixWM` 32-bit Graphical Window Compositor
- `clear`: Clears the framebuffer console screen and redraws the accent header
- `reboot`: Soft reboots the machine via 8042 keyboard controller reset
- `panic`: Triggers a test kernel panic diagnostic frame

---

## Building & Running

### 1. Build the Complete OS Image
```powershell
cargo run --package xtask -- build
```
This generates `target/lunix.img` (64 MiB FAT32 EFI disk image containing bootloader, kernel, ELF64 binaries, and config files).

### 2. Run with GUI Display in QEMU (with Intel e1000 Networking)
```powershell
cargo run --package xtask -- run
```

### 3. Run Headless Serial Mode
```powershell
tools\qemu\qemu-system-x86_64.exe -L tools\qemu\share -drive if=pflash,format=raw,readonly=on,file=tools\qemu\share\edk2-x86_64-code.fd -drive format=raw,file=target\lunix.img,if=ide -netdev user,id=net0 -device e1000,netdev=net0 -smp 2 -serial stdio -display none -m 512M
```

