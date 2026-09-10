# Lunix: Next-Generation Rust Operating System Kernel

**Lunix** is an independent, from-scratch 64-bit operating system kernel written in 100% pure Rust (`#![no_std]`), engineered with its own **custom Rust UEFI bootloader** and an architectural foundation designed to host **native Windows Driver Model (WDM / `.sys`) binaries**.

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                 Windows .SYS Drivers                        │
│            (PE32+ x86_64 Kernel Binaries)                   │
├─────────────────────────────────────────────────────────────┤
│         Lunix NT Compatibility Subsystem (Rust)             │
│  ┌───────────────────────┐   ┌───────────────────────────┐  │
│  │   PE/COFF .sys Loader │   │  IRP & Device Stack Mgmt  │  │
│  ├───────────────────────┤   ├───────────────────────────┤  │
│  │ ntoskrnl.exe API Shims│   │  HAL & IRQL Abstractions  │  │
│  └───────────────────────┘   └───────────────────────────┘  │
├─────────────────────────────────────────────────────────────┤
│                    Lunix Core Kernel                        │
│  ┌───────────────────────────────────────────────────────┐  │
│  │ Memory Mgmt (PMM Bitmap, VMM Paging, 16MB Heap)       │  │
│  │ Interrupts (IDT, Dual 8259 PIC, Exception Handlers)   │  │
│  │ CPU Management (GDT, TSS Double Fault Stack)          │  │
│  │ PCI Bus Scanner & Hardware Device Discovery           │  │
│  │ Display Subsystem (GOP Framebuffer + COM1 Serial)     │  │
│  └───────────────────────────────────────────────────────┘  │
├─────────────────────────────────────────────────────────────┤
│          Lunix Custom Rust UEFI Bootloader                  │
│       (GOP Init • Memory Map • Kernel Handoff)              │
├─────────────────────────────────────────────────────────────┤
│                   Hardware (x86_64)                         │
└─────────────────────────────────────────────────────────────┘
```

---

## Key Features

1. **Custom Rust UEFI Bootloader (`lunix-bootloader`)**:
   - Written in Rust using modern UEFI protocols (`x86_64-unknown-uefi`).
   - Discovers and configures the physical **Graphics Output Protocol (GOP)** display framebuffer.
   - Parses the EFI System Partition to dynamically allocate memory and load `\LUNIX\KERNEL.BIN`.
   - Extracts complete UEFI memory descriptors and passes a strongly-typed `BootInfo` structure across the ABI boundary into the kernel.
   - Cleanly calls `ExitBootServices` to transition the CPU from UEFI firmware into total Lunix ownership.

2. **Core Kernel Architecture (`lunix-kernel`)**:
   - **Higher-Half Long Mode Kernel**: Boots into 64-bit Ring 0 with hardware paging enabled.
   - **GDT & TSS**: Sets up Global Descriptor Tables and Task State Segment with an isolated double-fault stack.
   - **IDT & Interrupt Engine**: Handles CPU exceptions (Page Fault with CR2 logging, GPF, Divide-by-Zero, Double Faults) and hardware IRQs.
   - **Physical Memory Manager (PMM)**: Bitmap frame allocator tracking up to 4 GiB of physical memory in 4 KiB pages.
   - **Virtual Memory Manager (VMM)**: 4-level paging manager (PML4) supporting virtual address translation, page mapping, and MMIO memory ranges.
   - **Global Heap Allocator**: 16 MiB dynamic heap powered by `linked_list_allocator`, activating Rust's standard `alloc` crate (`Vec`, `String`, `Box`, `Arc`, `BTreeMap`).
   - **Display Console**: Graphical Framebuffer text console with bitmap fonts, smooth scrolling, status headers, and COM1 serial UART mirroring.
   - **Hardware Drivers**:
     - 8254 PIT Timer (1000 Hz system ticks, millisecond sleep, uptime calculation).
     - PS/2 Keyboard interrupt driver with US-104 key scancode decoding.
     - PCI Configuration Space Scanner reading buses 0..256 to discover attached hardware (GPUs, NICs, Storage controllers).

3. **Windows Driver Model (WDM) Subsystem (`subsystems/nt`)**:
   - **PE32+ Binary Parser**: Reads 64-bit PE/COFF headers, sections (`.text`, `.rdata`, `.data`), and base relocations from Windows `.sys` driver binaries.
   - **Microsoft x64 ABI (`extern "win64"`)**: Natively interops with Windows driver calling conventions directly from Rust without assembly trampolines.
   - **NT Kernel (`ntoskrnl.exe`) Shims**:
     - `ExAllocatePoolWithTag` & `ExFreePoolWithTag` (Memory allocation pools).
     - `MmMapIoSpace` & `MmUnmapIoSpace` (MMIO mapping for PCI devices and GPUs).
     - `KeInitializeSpinLock`, `KeAcquireSpinLock`, `KeReleaseSpinLock` (Spinlocks and IRQLs).
     - `IoCreateDevice`, `IoCompleteRequest`, `IoCallDriver` (I/O Request Packets).
   - **Hardware Abstraction Layer (`hal.dll`) Shims**:
     - `READ_PORT_UCHAR`, `WRITE_PORT_UCHAR`, `READ_PORT_ULONG`, `WRITE_PORT_ULONG`.
     - `HalGetBusDataByOffset` (PCI configuration access for Windows drivers).

---

## Project Structure

- [`common/`](file:///d:/REPOSITORIES/Lunix/common) - Shared `#![no_std]` definitions (`BootInfo`, `FramebufferInfo`, `MemoryMap`, `PixelFormat`).
- [`bootloader/`](file:///d:/REPOSITORIES/Lunix/bootloader) - Custom Rust UEFI bootloader binary (`lunix-bootloader.efi`).
- [`kernel/`](file:///d:/REPOSITORIES/Lunix/kernel) - Lunix 64-bit kernel (`lunix-kernel`).
- [`xtask/`](file:///d:/REPOSITORIES/Lunix/xtask) - Build automation tool creating FAT32 EFI images and running QEMU.

---

## Building & Running

### 1. Build the Complete OS Image
```powershell
cargo run --package xtask -- build
```
This compiles the bootloader, compiles the kernel, and creates `target/lunix.img` (a bootable 64MB FAT32 disk image containing `\EFI\BOOT\BOOTX64.EFI` and `\LUNIX\KERNEL.BIN`).

### 2. Launch in QEMU
```powershell
cargo run --package xtask -- run
```
Or run QEMU directly:
```powershell
qemu-system-x86_64 -bios <path-to-ovmf-edk2.fd> -drive format=raw,file=target/lunix.img -serial stdio -m 512M
```
