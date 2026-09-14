# AGENTS.md — AI Agent Guide & Technical Reference for Lunix OS

> **Note for AI Assistants & Autonomous Agents**: This document provides a complete technical map of the **Lunix** codebase. Read this before modifying or extending the kernel, bootloader, or subsystems.

---

## 1. Project Overview & Philosophy

**Lunix** is an independent, from-scratch 64-bit operating system kernel, window compositor, and custom UEFI bootloader written in 100% pure Rust (`#![no_std]`, `#![no_main]`).

### Core Design Principles:
1. **Pure Rust Bare-Metal**: No Linux kernel code, no GRUB/Limine, and no external C runtimes.
2. **Direct UEFI Boot**: Boots via our custom Rust UEFI bootloader (`lunix-bootloader.efi`) with automated GOP resolution, ACPI RSDP discovery, and ELF64 segment mapping.
3. **SMP Multi-Core & APIC**: ACPI 2.0 parser, Local APIC, IOAPIC routing, calibrated APIC timer, and 16-bit real-mode AP trampoline waking secondary CPU cores via INIT-SIPI-SIPI.
4. **VFS & FAT32 Storage**: Unified `BlockDevice` layer, IDE/ATA PIO driver (`/dev/sda`), and clean FAT32 filesystem mounted as `/`.
5. **Preemptive Multitasking**: Thread/Process management, assembly context switching, priority round-robin scheduler, and synchronization primitives (`Mutex`, `Semaphore`, `WaitQueue`).
6. **Ring 3 User Mode & Syscall ABI**: DPL=3 GDT selectors, TSS privilege stack table, fast MSR `syscall`/`sysret` dispatcher (`IA32_STAR`, `IA32_LSTAR`, `IA32_FMASK`), and reentrant syscall stack isolation.
7. **Windows NT Driver Subsystem**: Native Windows Driver Model (WDM) compatibility layer executing PE32+ drivers via Rust's native `extern "win64"` ABI with `ntoskrnl.exe` and `hal.dll` DDI shims and IRP dispatching.
8. **32-bit Graphical Compositor & LunixWM**: Double-buffered window manager with wallpaper gradient, drop shadows, window chrome, taskbar, PS/2 mouse driver, and multi-window application suite (System Monitor, NT Driver Manager, File Explorer, Terminal).

---

## 2. Workspace & Codebase Structure

```
d:\REPOSITORIES\Lunix
│
├── .cargo/
│   └── config.toml               # Target configuration & static relocation flags
├── rust-toolchain.toml           # Nightly toolchain & required components
├── Cargo.toml                    # Root Cargo Workspace definition
├── .gitignore                    # Git ignore rules for build artifacts & images
├── README.md                     # User-facing documentation
├── AGENTS.md                     # AI Agent architectural guide (this file)
│
├── common/                       # [Crate: lunix-common] (#![no_std])
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       └── boot_info.rs          # BootInfo, FramebufferInfo, MemoryMap, PixelFormat
│
├── bootloader/                   # [Crate: lunix-bootloader] (Target: x86_64-unknown-uefi)
│   ├── Cargo.toml
│   └── src/
│       └── main.rs               # UEFI entry point (efi_main), GOP setup, ELF64 loader
│
├── kernel/                       # [Crate: lunix-kernel] (Target: x86_64-unknown-none)
│   ├── Cargo.toml
│   ├── build.rs                  # Linker script argument passing
│   ├── linker.ld                 # Kernel linker script (Base: 0x2000000)
│   └── src/
│       ├── main.rs               # _start naked entry and kmain initialization sequence
│       ├── arch/x86_64/
│       │   ├── gdt.rs            # GDT, TSS (RSP0, IST), Kernel/User selectors
│       │   ├── idt.rs            # IDT with lockless UART exception handlers
│       │   ├── pic.rs            # 8259 PIC mask and remap
│       │   ├── serial.rs         # COM1 UART (0x3F8) direct logger
│       │   ├── fpu.rs            # x87 / SSE FPU initialization (CR0/CR4/MXCSR)
│       │   ├── io.rs             # Port I/O and MSR primitives (inb/outb/rdmsr/wrmsr)
│       │   ├── syscall.rs        # Fast MSR Syscall entry & stack isolation
│       │   ├── acpi/             # ACPI 2.0 parser (RSDP, RSDT/XSDT, MADT)
│       │   ├── apic/             # Local APIC & IOAPIC interrupt router
│       │   └── smp/              # Multi-Core AP trampoline & core startup
│       ├── display/
│       │   ├── font.rs           # 8x16 IBM VGA bitmap font table
│       │   ├── framebuffer.rs    # Scanline graphics & Color abstractions
│       │   ├── console.rs        # Text console & dual serial/GOP print macros
│       │   ├── window.rs         # Window and canvas drawing primitives
│       │   ├── compositor.rs     # 32-bit double-buffered desktop compositor
│       │   └── desktop.rs        # LunixWM multi-window desktop manager
│       ├── mm/
│       │   ├── pmm.rs            # Physical Frame Allocator (Bitmap tracking 4GB RAM)
│       │   ├── vmm.rs            # 4-Level PML4 Page Table manager & MMIO mappers
│       │   └── heap.rs           # 16 MiB dynamic heap (linked_list_allocator)
│       ├── drivers/
│       │   ├── pci.rs            # PCI configuration space scanner
│       │   ├── timer.rs          # APIC / PIT timer (1000 Hz / 1ms ticks)
│       │   ├── keyboard.rs       # PS/2 Keyboard scancode set 1 & interactive shell
│       │   ├── mouse.rs          # PS/2 3-byte mouse packet streaming & pointer state
│       │   └── storage/          # Block storage (ATA/IDE PIO driver)
│       ├── fs/
│       │   ├── block.rs          # BlockDevice trait & device registry
│       │   ├── inode.rs          # INode and INodeType definitions
│       │   ├── file.rs           # FileHandle and DirectoryEntry
│       │   ├── path.rs           # Path normalization & lookup
│       │   ├── vfs.rs            # Virtual File System mount table & operations
│       │   └── fat32/            # FAT32 driver (BPB, cluster chains, directory parser)
│       ├── task/
│       │   ├── thread.rs         # ThreadControlBlock & ThreadState
│       │   ├── process.rs        # ProcessControlBlock
│       │   ├── switch.rs         # Assembly context switch (context_switch)
│       │   ├── scheduler.rs      # Priority round-robin scheduler & timers
│       │   └── user.rs           # Ring 3 user mode entry via iretq
│       ├── sync/                 # Mutex, Semaphore, WaitQueue primitives
│       ├── syscall/              # POSIX-compatible syscall dispatcher
│       └── subsystems/nt/        # [Windows NT Subsystem]
│           ├── types.rs          # NT types (NTSTATUS, DRIVER_OBJECT, IRP, UNICODE_STRING)
│           ├── pe.rs             # PE32+ parser, section mapper, relocator, IAT resolver
│           ├── ntoskrnl.rs       # ntoskrnl.exe DDI shims (ExAllocatePoolWithTag, IoCreateDevice, DbgPrint)
│           ├── hal.rs            # hal.dll DDI shims (READ_PORT_*, WRITE_PORT_*)
│           └── mod.rs            # NT subsystem initialization & live WDM driver demo
│
└── xtask/                        # [Crate: xtask] (Build & Run Orchestrator)
    ├── Cargo.toml
    └── src/
        └── main.rs               # Builds bootloader, kernel, creates FAT32 image, runs QEMU
```

---

## 3. Critical Architectural Rules & Invariants

When working on this codebase, adhere to the following rules:

### A. Memory Layout & Linker Alignment
- In UEFI OVMF environments, memory below `0x1780000` is reserved by firmware/runtime services.
- The kernel is linked at `. = 0x2000000;` (32 MiB mark) in [`kernel/linker.ld`](file:///d:/REPOSITORIES/Lunix/kernel/linker.ld).
- The bootloader allocates physical memory at address `0x2000000` using `AllocateType::Address(0x2000000)`. Do **NOT** change this to an address below `0x1780000`.

### B. Relocation Model
- In [`.cargo/config.toml`](file:///d:/REPOSITORIES/Lunix/.cargo/config.toml), `rustflags` for `x86_64-unknown-none` MUST specify `-C relocation-model=static`.
- This ensures all function calls use direct PC-relative addressing with ZERO GOT/PLT indirect tables or dynamic relocations.

### C. GDT / IDT / Syscall Descriptors
- The kernel uses `SyncUnsafeCell` for `GDT`, `TSS`, and `IDT` to satisfy Rust 2024 edition static safety without deprecation warnings.
- GDT descriptor ordering MUST be preserved:
  - Selector `0x08`: Kernel Code (DPL 0)
  - Selector `0x10`: Kernel Data (DPL 0)
  - Selector `0x18 | 3 = 0x1B`: User Data (DPL 3)
  - Selector `0x20 | 3 = 0x23`: User Code (DPL 3)
  - Selector `0x28`: TSS
- This exact alignment matches `IA32_STAR` MSR (`0x0010_0008_0000_0000`) for `syscall` and `sysretq`.

### D. Hardened Exception Diagnostics
- Handlers in [`kernel/src/arch/x86_64/idt.rs`](file:///d:/REPOSITORIES/Lunix/kernel/src/arch/x86_64/idt.rs) use direct lockless UART port I/O (`write_str`, `write_hex`, `write_dec`).
- **Never** acquire locks or allocate heap inside fatal CPU exception handlers to prevent deadlock during fault logging.

### E. Windows NT Driver Calling Convention
- All Windows NT DDI exports and callbacks MUST be declared as `unsafe extern "win64" fn(...)`.
- Rust natively supports `win64` ABI on `x86_64`, passing arguments in `RCX, RDX, R8, R9` and returning values in `RAX`.

---

## 4. Build, Test & Run Workflow

### 1. Build Everything (Bootloader + Kernel + Image)
```powershell
cargo run --package xtask -- build
```
This generates `target/lunix.img` (64 MiB FAT32 EFI disk image).

### 2. Run with GUI Display in QEMU
```powershell
cargo run --package xtask -- run
```

### 3. Run in Headless Test Mode
```powershell
tools\qemu\qemu-system-x86_64.exe -L tools\qemu\share -drive if=pflash,format=raw,readonly=on,file=tools\qemu\share\edk2-x86_64-code.fd -drive format=raw,file=target\lunix.img,if=ide -smp 2 -serial stdio -display none -m 512M
```

---

## 5. Interactive Shell Commands

## 5. Interactive Linux Shell Commands

The kernel boots into an interactive graphical console with an IBM VGA font display and bash-compatible shell features (stateful current working directory `CWD`, relative path resolution, dynamic `lunix:{cwd}> ` prompt):
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
- `block`: Lists registered block devices (`/dev/sda`, `/dev/ahci0`, `/dev/nvme0n1`, `/dev/vda`)
- `storage`: Comprehensive storage controller breakdown (IDE, AHCI, NVMe, VirtIO)
- `ahci`: Displays AHCI SATA controller ABAR MMIO, port status, and attached drives
- `nvme`: Displays NVMe PCIe controller registers, queue pairs, and namespaces
- `virtio`: Displays VirtIO devices (VirtIO-Block, VirtIO-Net) and queue ring topologies
- `stat <path>`: Displays file or directory metadata
- `smp`: Displays multi-core SMP and APIC status
- `acpi`: Displays ACPI 2.0 tables and interrupt topology
- `tinycore`: Bootstraps TinyCore Linux PID 1 `/sbin/init`, parses `/etc/inittab`, executes `/etc/init.d/rcS` startup scripts, and mounts virtual filesystems
- `nt`: Displays Windows NT Subsystem and loaded driver status
- `nt load <path.sys>`: Dynamically loads and binds a 64-bit PE32+ WDM driver in Ring 0
- `nt list`: Displays active loaded Windows NT device drivers and dispatch routines
- `spawn`: Spawns a preemptive background kernel worker thread
- `sysdemo`: Executes Ring 3 User Mode demo via fast SYSCALL ABI
- `gui`: Launches `LunixWM` 32-bit Graphical Window Compositor
- `clear`: Clears the framebuffer console screen and redraws the accent header
- `reboot`: Soft reboots the machine via 8042 keyboard controller reset
- `panic`: Triggers a test kernel panic diagnostic frame

---

## 6. Subsystem Architecture Overview

### Phase 1: Interrupt Architecture & SMP
- **ACPI 2.0**: Discovers RSDP, RSDT/XSDT, and parses MADT for Local APIC addresses, CPU core records, and IOAPIC routing.
- **APIC**: Replaces legacy 8259 PIC with high-precision Local APIC (calibrated timer at 1000 Hz / 1ms periodic ticks) and IOAPIC (GSI 1 Keyboard -> 0x21, GSI 12 Mouse -> 0x2C).
- **SMP**: Wakes secondary cores using INIT-SIPI-SIPI sequence to an AP trampoline located at physical `0x8000`.

### Phase 2: Storage & VFS
- **ATA/IDE Driver**: Interacts directly with primary/secondary IDE controller ports (`0x1F0..0x1F7`, `0x3F6`) to register `/dev/sda` (64 MB disk).
- **FAT32 Driver**: Parses BPB layout, FAT cluster tables, directory clusters, and mounts the root filesystem to `/`.
- **VFS**: Provides standard POSIX file operations (`open`, `read`, `write`, `stat`, `read_dir`, `mount`).

### Phase 3: Preemptive Multitasking & Scheduler
- **Thread Management**: Dedicated thread stacks, priority scheduling, quantum accounting, and state machines (`Ready`, `Running`, `Sleeping`, `Blocked`, `Dead`).
- **Context Switcher**: Naked assembly `context_switch` preserving callee-saved registers (`RBX, RBP, R12-R15, RFLAGS, RSP`).
- **Synchronization**: `Mutex`, `Semaphore`, and `WaitQueue` primitives with timer-driven unblocking.

### Phase 4: Ring 3 User Mode & Linux Syscall ABI
- **Privilege Separation**: DPL 3 user code/data selectors loaded on `iretq` privilege transition.
- **Fast Syscalls**: `IA32_STAR`, `IA32_LSTAR`, and `IA32_FMASK` MSRs route user `syscall` directly into kernel entry without interrupt vector overhead.
- **Dedicated Syscall Stack**: Kernel allocates a dedicated 16 KiB aligned stack to prevent user code stack corruption.
- **Linux x86_64 Syscalls**: `sys_read` (0), `sys_write` (1), `sys_open` (2), `sys_close` (3), `sys_stat` (4), `sys_fstat` (5), `sys_lseek` (8), `sys_mmap` (9), `sys_munmap` (11), `sys_brk` (12), `sys_sched_yield` (24), `sys_nanosleep` (35), `sys_getpid` (39), `sys_socket` (41), `sys_connect` (42), `sys_sendto` (44), `sys_recvfrom` (45), `sys_bind` (49), `sys_exit` (60), `sys_uname` (63), `sys_getcwd` (79), `sys_chdir` (80).

### Phase 5: Windows NT Driver Subsystem (WDM)
- **PE32+ Loader**: Parses DOS header, PE header, optional header, and maps sections (`.text`, `.rdata`, `.data`, `.pdata`).
- **Base Relocations & IAT Resolver**: Applies 64-bit `IMAGE_REL_BASED_DIR64` relocations and resolves `ntoskrnl.exe` / `hal.dll` imports.
- **DDI Shims**: Implements `ExAllocatePoolWithTag`, `ExFreePoolWithTag`, `IoCreateDevice`, `IoDeleteDevice`, `IoCompleteRequest`, `IoCallDriver`, `DbgPrint`, `READ_PORT_*`, `WRITE_PORT_*`, and `HalGetBusDataByOffset`.

### Phase 6: Graphical Desktop Environment (LunixWM)
- **PS/2 Mouse Driver**: Streams 3-byte packets from IRQ12 / port `0x60`, tracking coordinates and button presses.
- **Double-Buffered Compositor**: Renders wallpaper gradient with cybernetic grid, window chrome with titlebars, drop shadows, window dragging, and top status bar.
- **LunixWM Applications**: Built-in System Resource Monitor, Windows NT Driver Manager, FAT32 File Explorer, and Interactive Terminal windows.

### Phase 7: Standalone ELF64 Binary Loader & Userspace Execution
- **ELF64 Parser & Segment Mapper**: Validates 64-bit ELF headers, parses `PT_LOAD` segments, allocates physical user frames, zeroes BSS sections, and loads segment contents.
- **User Stack Setup**: Allocates 64 KiB user stack and sets up initial Linux stack frame containing argc, argv, envp, and auxiliary vectors (`AT_PAGESZ`, `AT_ENTRY`, `AT_NULL`).
- **Binary Execution Runner**: `exec_elf(path)` loads any ELF executable from the VFS (e.g. `/bin/hello.elf`, `/bin/sysinfo.elf`) and launches it directly in Ring 3 user mode.

### Phase 8: Intel 82540EM Gigabit Network Driver
- **PCI Device Discovery**: Probes vendor `0x8086` and device IDs (`0x100E`, `0x1004`, `0x100F`, `0x153A`), enables PCI Bus Mastering.
- **MMIO & EEPROM**: Maps 128 KiB BAR0 memory space, reads factory hardware MAC from EEPROM registers (`EERD`) with fallback to `RAL`/`RAH`.
- **DMA Circular Rings**: Configures 64-entry Transmit and Receive descriptor rings backed by dedicated physical DMA memory frames.

### Phase 9: TCP/IP Protocol Stack & BSD Sockets
- **Ethernet II Framing**: Encapsulates and decapsulates frames with IEEE 802.3 ethertypes (`0x0800` IPv4, `0x0806` ARP).
- **ARP Subsystem**: Dynamic MAC table discovery, ARP request broadcasting, and automated ARP reply responder.
- **IPv4 Engine**: RFC 791 packet parsing, Internet 16-bit one's complement checksum validation, TTL management, and packet routing.
- **ICMP Ping**: Echo Request generator and Echo Reply parser with millisecond round-trip time tracking.
- **Socket & Network CLI**: Full socket management state machine and CLI commands (`ifconfig`, `ping`, `arp`, `netstat`).

### Phase 10: Linux Distro Engine & Process Lifecycle Syscalls
- **Process Lifecycle Syscalls**: Implements `sys_clone` (56), `sys_fork` (57), `sys_execve` (59), `sys_wait4` (61), `sys_exit_group` (231), `sys_getppid` (110).
- **Filesystem & Directory Syscalls**: `sys_getdents64` (217) yielding 64-bit Linux directory entries, `sys_openat` (257), `sys_fstatat` (262), `sys_readlink` (89).
- **Stream Pipes & Descriptors**: `sys_pipe` (22), `sys_pipe2` (293), `sys_dup` (32), `sys_dup2` (33), `sys_poll` (7).

### Phase 11: Windows Win32 User-Mode Subsystem & PE32+ Executable Engine
- **Win32 PE32+ Loader**: Parses DOS 'MZ' and PE64 headers, maps sections (`.text`, `.rdata`, `.data`), resolves IAT imports against `kernel32.dll` and `ntdll.dll`.
- **Win32 DDI Shims**: Implements `GetStdHandle`, `WriteConsoleA`, `WriteFile`, `ReadFile`, `ExitProcess`, `GetProcessHeap`, `HeapAlloc`, `HeapFree`, `VirtualAlloc`, `VirtualFree`, `GetSystemInfo`, `Sleep`, `GetCommandLineA`, `GetModuleHandleA`.
- **Universal Shell Binary Dispatcher**: Shell `exec <path>` auto-detects binary magic (`0x7F 'E' 'L' 'F'` for Linux ELF binaries vs `'M' 'Z'` for Windows PE32+ executables) and launches them in Ring 3 user mode.

### Phase 12: High-Speed Serial & Terminal FIFO Overrun Immunity
- **16550 UART FIFO 1-Byte Threshold**: Configures 16550 UART FIFO Control Register with `0x07` (1-byte trigger threshold) so QEMU immediately asserts IRQ 4 / vector `0x24` on the very first received byte.
- **Volatile Lock-Free Interrupt Draining**: `poll_hardware()` and `pop_byte()` operate with `without_interrupts` and volatile memory writes to prevent lock contention or dropped bytes in ISRs.
- **In-RAM Console Text Grid**: `Console` maintains a 160x64 `ConsoleCell` grid in CPU L1 RAM cache, eliminating thousands of slow uncached PCI MMIO reads during scrolling and frame rendering.
- **Dedicated QEMU Chardev Stream**: Uses `-chardev stdio,id=char0,mux=off -serial chardev:char0` for un-multiplexed raw stdio piping.
- **Host-Side Stdin Stream Forwarder**: `xtask` orchestrator pipes stdin via anonymous pipe with micro-pacing, eliminating Windows Console `KEY_EVENT` collisions and ensuring 100% paste fidelity across arbitrary length inputs.

### Phase 13: Milestone 1 — Process Hierarchy & Universal Execution Engine
- **Process Hierarchy & Tracking**: Extended `Process` with parent PID (`ppid`), child PID lists, termination status (`exit_code`), and active states (`is_alive`). Managed via thread-safe global `PROCESS_TABLE`.
- **Per-Process Standard File Descriptors**: Every created user process initializes FD 0 (`/dev/stdin`), FD 1 (`/dev/stdout`), and FD 2 (`/dev/stderr`), routing stream I/O through `Process::fds`.
- **Linux Fork & Clone Engine**: `sys_clone` (56) / `sys_fork` (57) duplicates the parent process, captures syscall return address (`LAST_USER_RIP`), and launches child in Ring 3 with `RAX=0` via `enter_user_mode_with_rax`.
- **Program Replacement (`execve`)**: `sys_execve` (59) loads standalone ELF64 executables, resets address space, populates System V stack frame with `argc`/`argv`, and transitions the calling thread.
- **Child Reaping (`wait4`)**: `sys_wait4` (61) blocks parent process until target child terminates, extracting exit status code (`WEXITSTATUS`).
- **Win32 Process Model**: Implements `CreateProcessA` (0x1010), `WaitForSingleObject` (0x1011), `GetExitCodeProcess` (0x1012), and `VirtualProtect` (0x1013) user-mode VDSO thunks and kernel shims.

### Phase 14: Milestone 2 — Streams, Pipes & Directory Navigation
- **Kernel IPC Pipes**: Dedicated circular `PipeBuffer` with atomic reader/writer tracking, non-blocking support, and preemptive sleep yielding. Integrated into `Process::fds` as `FdTarget::Pipe`.
- **Stream Syscalls**: Implements `sys_pipe` (22), `sys_pipe2` (293), `sys_dup` (32), `sys_dup2` (33), `sys_dup3` (292), and `sys_fcntl` (72) with `F_DUPFD`, `F_GETFD`, `F_SETFD` (`FD_CLOEXEC`), `F_GETFL`, `F_SETFL` (`O_NONBLOCK`).
- **Directory Traversal**: `sys_getdents64` (217) populates 64-bit Linux `struct linux_dirent64` entries from FAT32/VFS directory streams. `sys_openat` (257) and `sys_fstatat` (262) resolve relative path descriptors and populate metadata.
- **Terminal Control**: `sys_ioctl` (16) handles `TIOCGWINSZ`, `TCGETS`, `TCSETS`, and `FIONBIO`.
- **Win32 Stream & Search Subsystem**: Implements `CreatePipe` (0x1014), `SetStdHandle` (0x1015), `CreateFileA` (0x1016), `CloseHandle` (0x1017), `FindFirstFileA` (0x1018), `FindNextFileA` (0x1019), and `FindClose` (0x101A) VDSO thunks and kernel shims.

### Phase 15: Milestone 3 — Virtual Pseudo-Filesystems (`/dev` & `/proc`) and Windows Environment & In-Memory Registry
- **Linux Character & Block Devices (`devfs`)**:
  - `/dev/null`: Discards all writes; reads return EOF (0 bytes).
  - `/dev/zero`: Fills read buffers with `0x00`; discards all writes.
  - `/dev/urandom` & `/dev/random`: High-entropy pseudo-random byte generator powered by CPU `rdtsc` and 64-bit Xorshift PRNG.
  - `/dev/tty` & `/dev/console`: Routes read/write calls directly to active graphical framebuffer console and serial COM1 UART.
  - `/dev/sda`: Standard block storage device handle.
- **Linux Dynamic System Info Nodes (`procfs`)**:
  - `/proc/version`: Linux 6.8.0 kernel string with Rust compiler version and SMP build timestamp.
  - `/proc/meminfo`: Live physical memory accounting (`MemTotal`, `MemFree`, `MemAvailable`, `Cached`) parsed dynamically from PMM and Heap stats.
  - `/proc/cpuinfo`: Multi-core processor topology (`processor`, `vendor_id`, `model name`, `flags`, `cpu cores`) generated from APIC/SMP core records.
  - `/proc/mounts`: Dynamic mount table containing active root and pseudo-filesystem mount entries.
  - `/proc/uptime`: Real-time system uptime metrics generated from 1000 Hz Local APIC timer ticks.
- **Unified VFS File Descriptor Streaming**:
  - Added `FdTarget::VfsHandle(Arc<Mutex<Box<dyn FileHandle>>>)` to `Process::fds`.
  - Seamless stream routing for `sys_open`, `sys_openat`, `sys_read`, `sys_write`, `sys_lseek`, and `sys_close` across all physical and virtual filesystem handles.
- **Windows Win32 Environment & In-Memory Registry Subsystem**:
  - `WIN32_ENV`: Thread-safe global store pre-populated with `OS`, `PROCESSOR_ARCHITECTURE`, `NUMBER_OF_PROCESSORS`, `PATH`, `SYSTEMROOT`, `TEMP`, `USERPROFILE`.
  - `WIN32_REGISTRY`: Hierarchical registry key/value database pre-populated with `HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion` and `HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Environment`.
  - Win32 DDI Shims: `GetEnvironmentVariableA` (0x101B), `SetEnvironmentVariableA` (0x101C), `RegOpenKeyExA` (0x101D), `RegQueryValueExA` (0x101E), `RegCloseKey` (0x101F).
  - Extended Win64 ABI thunk generator in `emit_win32_thunk` to unpack 5th and 6th parameters from `[RSP + 0x28]` and `[RSP + 0x30]`.

### Phase 16: Milestone 4 — Real Linux Userspace Bootstrapping (BusyBox Rootfs, Signals & TLS)
- **Thread-Local Storage (TLS) Subsystem**:
  - Implemented `sys_arch_prctl` (158) with subfunctions `ARCH_SET_FS` (0x1002), `ARCH_GET_FS` (0x1003), `ARCH_SET_GS` (0x1001), `ARCH_GET_GS` (0x1004).
  - Configures CPU MSR `0xC0000100` (`IA32_FS_BASE`) via direct `wrmsr`/`rdmsr`, establishing instantaneous musl/glibc `pthread_t` runtime compatibility with zero context switch overhead.
- **Linux Signals & Signal Actions**:
  - Implemented `sys_rt_sigaction` (13), `sys_rt_sigprocmask` (14), `sys_rt_sigreturn` (15), and `sys_kill` (62).
- **Process Credentials & Process Groups**:
  - Implemented `sys_getuid` (102), `sys_getgid` (104), `sys_geteuid` (107), `sys_getegid` (108), `sys_getgroups` (115), `sys_setpgid` (109), `sys_getpgid` (121), `sys_getpgrp` (111), and `sys_setsid` (112).
- **Filesystem & Link Navigation**:
  - Implemented `sys_access` (21) & `sys_faccessat` (269) for permission checks.
  - Implemented `sys_readlink` (89) & `sys_readlinkat` (267) supporting `/proc/self/exe` resolution for multi-call binary self-discovery.
  - Implemented `sys_statfs` (137) & `sys_fstatfs` (138) returning filesystem geometry and block statistics (`f_type=0x4006` FAT32).
  - Implemented `sys_mprotect` (10), `sys_mkdir` (83), `sys_mkdirat` (258), `sys_unlink` (87), `sys_unlinkat` (263), `sys_rmdir` (84).
- **Timing & Resource Limits**:
  - Implemented `sys_clock_gettime` (228) and `sys_gettimeofday` (96) backed by 1000 Hz Local APIC timer ticks.
  - Implemented `sys_getrlimit` (97), `sys_setrlimit` (160), and `sys_prlimit64` (302) with standard POSIX limits (`RLIMIT_NOFILE`, `RLIMIT_STACK`, `RLIMIT_AS`).
- **Complete System V AMD64 ELF Stack Frame**:
  - Enhanced `load_elf_with_args` to construct System V stack layout with `argc`, `argv`, `envp` (`PATH=/bin:/usr/bin`, `HOME=/root`, `USER=root`, `TERM=linux`, `SHELL=/bin/sh`, `PWD=/`), 16-byte random seed, and complete auxiliary vector pairs (`AT_PHDR`, `AT_PHENT`, `AT_PHNUM`, `AT_PAGESZ`, `AT_BASE`, `AT_FLAGS`, `AT_ENTRY`, `AT_UID`, `AT_EUID`, `AT_GID`, `AT_EGID`, `AT_CLKTCK`, `AT_RANDOM`, `AT_NULL`).
- **BusyBox Userspace Multi-Call Distribution**:
  - Embedded multi-call static `/bin/busybox`, `/bin/sh`, and test verification binary `/bin/test_busybox.elf` on the root FAT32 filesystem.
  - Full suite verified 100% via `tests/test_milestone4.py` in QEMU.

### Phase 17: Milestone 5 — Advanced Hardware Drivers & WDM Expansion
- **Native AHCI 1.0 SATA Storage Subsystem**:
  - Probes PCI class `0x01` subclass `0x06` prog-if `0x01` (e.g. Intel ICH9/ICH6 AHCI).
  - Maps 32-bit/64-bit ABAR (BAR5) MMIO registers, resets HBA, configures GHC with AE/IE flags.
  - Allocates Command Lists (32 headers), Received FIS structures, and Physical Region Descriptor Tables (PRDTs).
  - Implements `BlockDevice` trait, registering high-speed SATA disk `/dev/ahci0` with DMA scatter-gather read/write.
- **Native NVMe PCIe Solid-State Storage Subsystem**:
  - Probes PCI class `0x01` subclass `0x08` prog-if `0x02` (NVMe controller).
  - Maps 64-bit BAR0 MMIO space, checks CAP/VS, sets AQA, allocates Admin Submission/Completion Queues (ASQ/ACQ).
  - Issues NVMe Controller Enable (`CC.EN=1`) with CSTS readiness polling.
  - Discovers Namespaces via Admin Command `0x06` (Identify Namespace), creates I/O Queue Pairs (IOSQ/IOCQ), and registers `/dev/nvme0n1` implementing `BlockDevice` with Physical Region Page (PRP) DMA block transfers.
- **Native VirtIO Virtualization Storage & Network Subsystem**:
  - Probes VirtIO PCI Subsystem Vendor `0x1AF4` Device IDs (`0x1001` Block, `0x1000` Network).
  - Configures Split VirtQueue ring buffers (`DescTable`, `AvailRing`, `UsedRing`).
  - Registers `/dev/vda` (`VirtioBlkDevice`) implementing `BlockDevice` and network interface `VirtioNetDevice`.
- **Expanded Windows NT Driver Model (WDM) Subsystem**:
  - Added full NT synchronization primitives: `KEVENT` (`KeInitializeEvent`, `KeSetEvent`, `KeResetEvent`, `KeClearEvent`, `KeWaitForSingleObject`), `KMUTEX` (`KeInitializeMutex`, `KeReleaseMutex`).
  - Added Device Stacks and Driver I/O: `IoCreateDevice`, `IoDeleteDevice`, `IoAttachDevice`, `IoAttachDeviceToDeviceStack`, `IoDetachDevice`, `IoAllocateIrp`, `IoFreeIrp`, `IoAllocateMdl`, `IoFreeMdl`, `MmProbeAndLockPages`, `MmUnlockPages`, `IoCompleteRequest`, `IoCallDriver`, `IoCreateSymbolicLink`, `IoDeleteSymbolicLink`, `DbgPrint`.
  - Added HAL DDI shims: `HalTranslateBusAddress`, `HalAllocateCommonBuffer`, `HalFreeCommonBuffer`, `KeFlushWriteBuffer`.
- **Dynamic PE32+ `.sys` Driver Loader**:
  - Parses 64-bit PE/COFF headers from disk files (e.g. `/sys/drivers/sample_wdm.sys`), allocates executable memory frames, applies base relocations (`IMAGE_REL_BASED_DIR64`), binds `ntoskrnl.exe` and `hal.dll` IAT imports.
  - Dynamically initializes `DRIVER_OBJECT`, populates major dispatch tables (`IRP_MJ_CREATE`, `IRP_MJ_READ`, `IRP_MJ_WRITE`, `IRP_MJ_DEVICE_CONTROL`), and calls `DriverEntry(DriverObject, RegistryPath)` directly in Ring 0 via Rust's native `extern "win64"` ABI.
  - Fully automated and verified 100% via `tests/test_milestone5.py` and interactive shell commands (`storage`, `ahci`, `nvme`, `virtio`, `nt load <path.sys>`, `nt list`).

### Phase 18: Milestone 6 — Official Upstream Tiny Core Linux Rootfs Ingestion, Linux Framebuffer (/dev/fb0) & VMware Support
- **Official Upstream Tiny Core Linux Rootfs Ingestion**:
  - Ingests all 1,855+ files (385 directories, 1,060 regular files, 285 symlinks) from upstream `corepure64.gz` / `TinyCorePure64.iso` (Tiny Core Linux v15.0 x86_64) into the root FAT32 disk image (`target/lunix.img`, 256 MiB).
  - Replaces upstream `vmlinuz64` with custom bare-metal `lunix-kernel` and UEFI bootloader `lunix-bootloader.efi`.
  - Official 64-bit GNU Glibc 2.38 dynamic linker (`/lib/ld-linux-x86-64.so.2`), Glibc C library (`libc.so.6`), and upstream BusyBox binary execute directly on the Lunix bare-metal kernel.
- **Dynamic ELF Interpreter & Loader Subsystem (`PT_INTERP`)**:
  - Extended ELF64 parser to extract `PT_INTERP` program headers (e.g. `/lib/ld-linux-x86-64.so.2`).
  - Reads interpreter binary from VFS and maps its segments at `INTERP_LOAD_BASE` (`0x0000_7FFF_E000_0000`).
  - Populates complete auxiliary vectors (`auxv`) on the user stack: `AT_BASE`, `AT_ENTRY`, `AT_PHDR`, `AT_PHENT`, `AT_PHNUM`, `AT_PAGESZ`, `AT_RANDOM`, `AT_EXECFN`, `AT_NULL`.
  - Dynamic linkers scan auxv, resolve shared library dependencies (`libcrypt.so.1`, `libm.so.6`, `libc.so.6`), and execute application entry points.
- **Linux Framebuffer Device Node (`/dev/fb0`, `/dev/fb/0`) & GOP MMIO Mapping**:
  - Registered character device nodes `/dev/fb0` and `/dev/fb/0` (`FbHandle`).
  - In `sys_mmap`, detects file descriptors opening `/dev/fb0` and performs direct page table mapping of UEFI GOP physical MMIO memory into userspace virtual memory.
  - Implemented Linux Framebuffer ioctls: `FBIOGET_VSCREENINFO` (`0x4600`), `FBIOPUT_VSCREENINFO` (`0x4601`), `FBIOGET_FSCREENINFO` (`0x4602`), VT mode switches (`VT_GETMODE`, `VT_SETMODE`).
  - Registered `/dev/input/mice`, `/dev/input/event0`, `/dev/tty0`..`/dev/tty2` device handles.
- **Advanced Linux Syscalls for Upstream Glibc**:
  - Implemented `sys_pread64` (17) preserving seek position for ELF notes/program headers.
  - Implemented `sys_writev` (20) & `sys_readv` (19) for scatter-gather vector I/O.
  - Implemented `sys_fstat` (5), `sys_select` (23), `sys_madvise` (28), `sys_gettid` (186), `sys_tgkill` (234).
  - Extended `FdTarget::VfsHandle` with 64-bit unique `inode_id` to prevent library deduplication collisions in Glibc `ld-linux`.
  - Updated `sys_uname` with release `6.8.0-tinycore` and version `(Lunix 0.1.0-hybrid)`.
- **C Runtime & System Control Syscalls**:
  - `sys_sysinfo` (99) returning live physical memory totals, free memory, system uptime, and process count in `struct sysinfo`.
  - `sys_set_tid_address` (218) for glibc/musl thread address space setup.
  - `sys_set_robust_list` (273) and `sys_get_robust_list` (274) for robust futex list management.
  - `sys_futex` (202) supporting `FUTEX_WAIT` and `FUTEX_WAKE` for fast user-space synchronization.
  - `sys_mount` (165), `sys_umount2` (166), `sys_rseq` (334), and `sys_rename` (82).
- **Multi-Hypervisor VM Disk Export**:
  - Automatically generates `target/lunix.vmdk` (VMware Workstation Pro / Player) and `target/lunix.vdi` (VirtualBox).
  - Run in VMware Workstation: Configure VM Settings -> Options -> Advanced -> **Firmware type: UEFI**.
  - Run in VirtualBox: Enable EFI under VM Settings -> System -> Motherboard -> **Enable EFI**.
- **Automated Verification**:
  - `tests/test_tinycore_upstream.py` verified 100% (19/19 checks passed).
  - `tests/test_milestone6.py` verified 100% (17/17 checks passed).
  - 100% passing across all Milestones 1 through 6 (108/108 checks passed).

### Phase 19: Milestone 7 — Linux Distribution Userspace Bootstrap & Hardware Address Space Isolation
- **Hardware Page Table User Memory Duplication (`clone_process_pml4`)**:
  - Implemented deep 4-level PML4 page directory walker duplicating user-space mappings (PML4 entries 0..255, `< 0x0000_8000_0000_0000`).
  - Allocates dedicated physical 4 KiB frames for all PDPT, PD, and PT levels and copies memory contents for user pages (stack, heap, code, data).
  - Preserves shared higher-half kernel memory mappings (`>= 0xFFFF_8000_0000_0000`) across all processes without duplicating kernel structures.
  - Resolves glibc `_Fork()` (`0x1200011` = `CLONE_CHILD_SETTID | CLONE_CHILD_CLEARTID | SIGCHLD`), giving the child process a dedicated, isolated address space while strictly preserving parent return addresses and stack frames.
- **Per-Thread Thread-Local Storage (TLS) `FS_BASE` MSR Isolation**:
  - Added `fs_base` tracking to `ThreadControlBlock` (`Thread`).
  - Extended scheduler context switch in `Scheduler::schedule()` to read `old_thread.fs_base = rdmsr(0xC000_0100)` and write `wrmsr(0xC000_0100, next_fs_base)`.
  - Integrated `set_current_thread_fs_base` into `sys_arch_prctl` and `exec_elf_replace` for instant synchronization with glibc/musl `pthread_t` runtime structures.
- **PID 1 `/sbin/init` & `/etc/init.d/rcS` Userspace Bootstrapper**:
  - Added kernel `tinycore` command and automated test orchestrator bootstrapping PID 1 `/sbin/init`.
  - Parses `/etc/inittab`, executes `/etc/init.d/rcS` sysinit scripts, mounts `/proc`, `/sys`, `/dev` via `/bin/mount`, and gracefully drops to shell upon completion.
  - Implemented `sys_wait4` status reaping with correct `wstatus` formatting (`WEXITSTATUS`), unblocking parent processes upon child exit.
- **Full Compatibility Suite**:
  - Linux dynamic ELF binaries with GNU Glibc 2.38 (`/lib/ld-linux-x86-64.so.2`, `libc.so.6`).
  - BusyBox multi-call userspace tools.
  - Windows NT Win32 PE32+ executables (`kernel32.dll`, Win32 Environment Block, In-Memory Registry, Named Pipes, Directory Find API).
  - 100% automated verification passing across all Linux and Windows test binaries.






