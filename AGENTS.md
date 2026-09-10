# AGENTS.md — AI Agent Guide & Technical Reference for Lunix OS

> **Note for AI Assistants & Autonomous Agents**: This document provides a complete technical map of the **Lunix** codebase. Read this before modifying or extending the kernel, bootloader, or subsystems.

---

## 1. Project Overview & Philosophy

**Lunix** is an independent, from-scratch 64-bit operating system kernel and custom UEFI bootloader written in 100% pure Rust (`#![no_std]`, `#![no_main]`).

### Core Design Principles:
1. **Pure Rust Bare-Metal**: No Linux kernel code, no GRUB/Limine, and no C runtime.
2. **Direct UEFI Boot**: Boots via our own custom Rust UEFI bootloader (`lunix-bootloader.efi`).
3. **Windows NT Driver Subsystem**: Implements a native Windows Driver Model (WDM) compatibility layer executing `.sys` PE32+ binaries via Rust's native `extern "win64"` ABI.
4. **Safety & Hardened Diagnostics**: Pure Rust memory managers (PMM/VMM/Heap), lockless UART exception handlers to prevent secondary fault loops, and strict descriptor management.

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
│   ├── build.rs                  # Passes linker script flag
│   ├── linker.ld                 # Kernel linker script (Base: 0x2000000)
│   └── src/
│       ├── main.rs               # _start naked entry and kmain initialization sequence
│       ├── arch/x86_64/
│       │   ├── gdt.rs            # GDT, TSS with Double-Fault IST stack (SyncUnsafeCell)
│       │   ├── idt.rs            # IDT with hardened lockless UART exception handlers
│       │   ├── pic.rs            # Dual 8259 PIC driver
│       │   ├── serial.rs         # COM1 UART (0x3F8) port I/O direct logger
│       │   ├── fpu.rs            # x87 / SSE FPU initialization (CR0/CR4/MXCSR)
│       │   └── io.rs             # Port I/O primitives (inb/outb/inw/outw/inl/outl)
│       ├── display/
│       │   ├── font.rs           # 8x16 IBM VGA bitmap font table
│       │   ├── framebuffer.rs    # 32-bit dword-optimized scanlines, 64-bit word scrolling
│       │   └── console.rs        # Console struct & formatted dual-output macros (lunix_println!)
│       ├── mm/
│       │   ├── pmm.rs            # Physical Frame Allocator (Bitmap tracking 4GB RAM)
│       │   ├── vmm.rs            # 4-Level PML4 Page Table manager & MMIO mappers
│       │   └── heap.rs           # 16 MiB dynamic heap powered by linked_list_allocator
│       ├── drivers/
│       │   ├── pci.rs            # PCI configuration space scanner (Buses 0..1, Devices 0..31)
│       │   ├── timer.rs          # PIT 8254 Timer (1000 Hz / 1ms ticks)
│       │   └── keyboard.rs       # PS/2 Keyboard scancode set 1 & interactive shell
│       └── subsystems/nt/        # [Windows NT Subsystem]
│           ├── types.rs          # NT types (NTSTATUS, UNICODE_STRING, DRIVER_OBJECT, IRP)
│           ├── pe.rs             # PE32+ parser, section mapper, 64-bit base relocations, IAT resolver
│           ├── ntoskrnl.rs       # ntoskrnl.exe DDI shims (ExAllocatePoolWithTag, IoCreateDevice, DbgPrint)
│           ├── hal.rs            # hal.dll DDI shims (READ_PORT_*, HalGetBusDataByOffset)
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

### C. GDT / IDT Initialization & Static Mutation
- The kernel uses `SyncUnsafeCell` for `GDT`, `TSS`, and `IDT` to satisfy Rust 2024 edition static safety without deprecation warnings.
- Inside `gdt::init()`, `*gdt = GlobalDescriptorTable::new();` must be explicitly executed to ensure descriptor index `1` is properly set even after `.bss` zeroing.

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
qemu-system-x86_64 -bios tools/qemu/share/edk2-x86_64-code.fd -drive format=raw,file=target/lunix.img -serial stdio -display none -m 512M
```

---

## 5. Interactive Shell Commands

The kernel boots into an interactive graphical console with an IBM VGA font display. Supported shell commands:
- `help`: Lists all kernel shell commands
- `info`: Displays OS kernel and CPU architecture info
- `mem`: Live breakdown of physical RAM, usable RAM, used frames, and heap size
- `pci`: Scans and prints PCI bus hardware devices
- `nt`: Displays Windows NT Subsystem and loaded driver status
- `clear`: Clears the framebuffer console screen and redraws the accent header
- `reboot`: Soft reboots the computer via keyboard controller reset
- `panic`: Tests the kernel panic handler and exception frame display

---

## 6. Future Expansion Roadmap

1. **APIC & ACPI**: MADT parsing, local APIC timer replacing legacy PIT, and IOAPIC interrupt routing.
2. **SMP Multi-Core**: Spawning Application Processors (APs) via INIT-SIPI-SIPI sequences.
3. **VFS & Filesystems**: Implementing an abstract Virtual Filesystem (VFS) with FAT32 / Ext2 read/write drivers.
4. **User Mode (Ring 3)**: TSS privilege level 3 transitions, `syscall` / `sysret` instruction setup, and ELF user space process loading.
5. **Real Windows Driver Testing**: Loading external `.sys` network or serial drivers from the FAT32 filesystem and executing them through the NT subsystem.
