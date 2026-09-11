#![no_main]
#![no_std]

extern crate alloc;

use core::panic::PanicInfo;
use core::ptr::NonNull;
use core::slice;
use log::info;
use lunix_common::{
    BootInfo, FramebufferInfo, MemoryRegion, MemoryRegionType, PixelFormat,
    HIGHER_HALF_OFFSET, LUNIX_BOOT_MAGIC, PAGE_SIZE,
};

use uefi::boot::{AllocateType, MemoryType};
use uefi::mem::memory_map::MemoryMap as UefiMemoryMapTrait;
use uefi::prelude::*;
use uefi::proto::console::gop::{GraphicsOutput, PixelFormat as UefiPixelFormat};
use uefi::proto::media::file::{File, FileAttribute, FileMode};
use uefi::proto::media::fs::SimpleFileSystem;

#[global_allocator]
static ALLOCATOR: uefi::allocator::Allocator = uefi::allocator::Allocator;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    log::error!("{}", info);
    loop {}
}

#[no_mangle]
pub unsafe extern "C" fn wcslen(mut s: *const u16) -> usize {
    let mut len = 0;
    while *s != 0 {
        s = s.add(1);
        len += 1;
    }
    len
}

// ELF64 Structures
#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct Elf64Header {
    e_ident: [u8; 16],
    e_type: u16,
    e_machine: u16,
    e_version: u32,
    e_entry: u64,
    e_phoff: u64,
    e_shoff: u64,
    e_flags: u32,
    e_ehsize: u16,
    e_phentsize: u16,
    e_phnum: u16,
    e_shentsize: u16,
    e_shnum: u16,
    e_shstrndx: u16,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct Elf64ProgramHeader {
    p_type: u32,
    p_flags: u32,
    p_offset: u64,
    p_vaddr: u64,
    p_paddr: u64,
    p_filesz: u64,
    p_memsz: u64,
    p_align: u64,
}

const PT_LOAD: u32 = 1;

#[entry]
fn main() -> Status {
    uefi::helpers::init().unwrap();
    info!("===============================================");
    info!("   LUNIX CUSTOM RUST UEFI BOOTLOADER v0.1.0   ");
    info!("===============================================");

    // 1. Initialize GOP (Graphics Output Protocol)
    info!("[+] Locating Graphics Output Protocol (GOP)...");
    let gop_handle = match uefi::boot::get_handle_for_protocol::<GraphicsOutput>() {
        Ok(h) => h,
        Err(e) => {
            log::error!("[-] Failed to find GOP handle: {:?}", e);
            return Status::UNSUPPORTED;
        }
    };

    let mut gop = match uefi::boot::open_protocol_exclusive::<GraphicsOutput>(gop_handle) {
        Ok(g) => g,
        Err(e) => {
            log::error!("[-] Failed to open GOP: {:?}", e);
            return Status::UNSUPPORTED;
        }
    };

    let gop_mode = gop.current_mode_info();
    let (width, height) = gop_mode.resolution();
    let stride = gop_mode.stride();
    let pixel_format = match gop_mode.pixel_format() {
        UefiPixelFormat::Rgb => PixelFormat::Rgb,
        UefiPixelFormat::Bgr => PixelFormat::Bgr,
        UefiPixelFormat::Bitmask => PixelFormat::Bitmask,
        _ => PixelFormat::Unknown,
    };

    let mut fb = gop.frame_buffer();
    let fb_base = fb.as_mut_ptr() as u64;
    let fb_size = fb.size();

    info!(
        "[+] Display initialized: {}x{}, Stride: {}, Format: {:?}, Base: 0x{:X}",
        width, height, stride, pixel_format, fb_base
    );

    let fb_info = FramebufferInfo {
        base_address: fb_base,
        size: fb_size,
        width,
        height,
        stride,
        format: pixel_format,
        bytes_per_pixel: 4,
    };

    // 2. Load Kernel Binary from EFI System Partition
    info!("[+] Loading kernel binary from filesystem...");
    let fs_handle = match uefi::boot::get_handle_for_protocol::<SimpleFileSystem>() {
        Ok(h) => h,
        Err(e) => {
            log::error!("[-] Failed to find SimpleFileSystem: {:?}", e);
            return Status::NOT_FOUND;
        }
    };

    let mut fs = match uefi::boot::open_protocol_exclusive::<SimpleFileSystem>(fs_handle) {
        Ok(f) => f,
        Err(e) => {
            log::error!("[-] Failed to open SimpleFileSystem: {:?}", e);
            return Status::NOT_FOUND;
        }
    };

    let mut root_dir = match fs.open_volume() {
        Ok(d) => d,
        Err(e) => {
            log::error!("[-] Failed to open root volume: {:?}", e);
            return Status::NOT_FOUND;
        }
    };

    let kernel_file_handle = root_dir
        .open(
            cstr16!(r"\LUNIX\KERNEL.BIN"),
            FileMode::Read,
            FileAttribute::empty(),
        )
        .or_else(|_| {
            root_dir.open(
                cstr16!(r"\KERNEL.BIN"),
                FileMode::Read,
                FileAttribute::empty(),
            )
        });

    let mut kernel_file = match kernel_file_handle {
        Ok(f) => match f.into_regular_file() {
            Some(rf) => rf,
            None => {
                log::error!("[-] Kernel path is a directory, not a file");
                return Status::NOT_FOUND;
            }
        },
        Err(e) => {
            log::error!("[-] Failed to locate kernel binary: {:?}", e);
            return Status::NOT_FOUND;
        }
    };

    let mut info_buf = [0u8; 512];
    let file_info = match kernel_file.get_info::<uefi::proto::media::file::FileInfo>(&mut info_buf) {
        Ok(i) => i,
        Err(e) => {
            log::error!("[-] Failed to get kernel file info: {:?}", e);
            return Status::LOAD_ERROR;
        }
    };
    let kernel_file_size = file_info.file_size() as usize;
    info!("[+] Found kernel binary (Size: {} bytes)", kernel_file_size);

    // Allocate temporary buffer to read entire ELF file
    let temp_pages = (kernel_file_size + (PAGE_SIZE as usize) - 1) / (PAGE_SIZE as usize);
    let temp_buffer_ptr: NonNull<u8> = match uefi::boot::allocate_pages(
        AllocateType::AnyPages,
        MemoryType::LOADER_DATA,
        temp_pages,
    ) {
        Ok(ptr) => ptr,
        Err(e) => {
            log::error!("[-] Failed to allocate temp pages for ELF: {:?}", e);
            return Status::OUT_OF_RESOURCES;
        }
    };

    let temp_slice = unsafe {
        slice::from_raw_parts_mut(temp_buffer_ptr.as_ptr(), kernel_file_size)
    };

    if let Err(e) = kernel_file.read(temp_slice) {
        log::error!("[-] Failed to read kernel file: {:?}", e);
        return Status::LOAD_ERROR;
    }

    // Parse ELF Header
    let elf_header = unsafe { &*(temp_slice.as_ptr() as *const Elf64Header) };
    if &elf_header.e_ident[0..4] != b"\x7FELF" {
        log::error!("[-] Invalid ELF magic!");
        return Status::LOAD_ERROR;
    }

    info!(
        "[+] Valid ELF64 kernel found. Entry Point: 0x{:X}, Program Headers: {}",
        elf_header.e_entry, elf_header.e_phnum
    );

    // 3. Determine total span of ELF image
    let ph_offset = elf_header.e_phoff as usize;
    let ph_entsize = elf_header.e_phentsize as usize;
    let ph_num = elf_header.e_phnum as usize;

    let mut min_vaddr: u64 = u64::MAX;
    let mut max_vaddr: u64 = 0;

    for i in 0..ph_num {
        let ph_ptr = unsafe {
            temp_slice.as_ptr().add(ph_offset + i * ph_entsize) as *const Elf64ProgramHeader
        };
        let ph = unsafe { *ph_ptr };

        if ph.p_type == PT_LOAD {
            if ph.p_vaddr < min_vaddr {
                min_vaddr = ph.p_vaddr;
            }
            if ph.p_vaddr + ph.p_memsz > max_vaddr {
                max_vaddr = ph.p_vaddr + ph.p_memsz;
            }
        }
    }

    let total_kernel_size = (max_vaddr - min_vaddr) as usize;
    let total_pages = (total_kernel_size + PAGE_SIZE as usize - 1) / PAGE_SIZE as usize;

    // Log available memory regions
    if let Ok(mmap_storage) = uefi::boot::memory_map(MemoryType::LOADER_DATA) {
        for desc in mmap_storage.entries() {
            if desc.ty == MemoryType::CONVENTIONAL && desc.page_count >= total_pages as u64 {
                info!("  [MMAP] Available Conventional RAM: 0x{:X}..0x{:X} (Pages: {})", 
                    desc.phys_start, 
                    desc.phys_start + desc.page_count * PAGE_SIZE, 
                    desc.page_count
                );
            }
        }
    }

    // Allocate contiguous physical memory region for kernel at its target address (e.g. 0x1000000)
    let kernel_phys_ptr: NonNull<u8> = match uefi::boot::allocate_pages(
        AllocateType::Address(min_vaddr),
        MemoryType::LOADER_CODE,
        total_pages,
    ) {
        Ok(ptr) => ptr,
        Err(e) => {
            log::warn!("[-] Fixed allocation failed at 0x{:X}: {:?}, falling back to AnyPages", min_vaddr, e);
            match uefi::boot::allocate_pages(
                AllocateType::AnyPages,
                MemoryType::LOADER_CODE,
                total_pages,
            ) {
                Ok(ptr) => ptr,
                Err(e) => {
                    log::error!("[-] Failed to allocate {} pages for kernel: {:?}", total_pages, e);
                    return Status::OUT_OF_RESOURCES;
                }
            }
        }
    };

    let kernel_phys_base = kernel_phys_ptr.as_ptr() as u64;

    // Zero out the entire allocated kernel space
    unsafe {
        core::ptr::write_bytes(kernel_phys_ptr.as_ptr(), 0, total_pages * PAGE_SIZE as usize);
    }

    // Load each PT_LOAD segment into contiguous space at its relative offset
    for i in 0..ph_num {
        let ph_ptr = unsafe {
            temp_slice.as_ptr().add(ph_offset + i * ph_entsize) as *const Elf64ProgramHeader
        };
        let ph = unsafe { *ph_ptr };

        if ph.p_type == PT_LOAD {
            let offset_in_kernel = (ph.p_vaddr - min_vaddr) as usize;
            unsafe {
                let dest = kernel_phys_ptr.as_ptr().add(offset_in_kernel);
                let src = temp_slice.as_ptr().add(ph.p_offset as usize);
                core::ptr::copy_nonoverlapping(src, dest, ph.p_filesz as usize);
            }

            info!(
                "  [SEG] Loaded PT_LOAD: Offset=0x{:X}, Phys=0x{:X}, FileSize={}, MemSize={}",
                offset_in_kernel,
                kernel_phys_base + offset_in_kernel as u64,
                ph.p_filesz,
                ph.p_memsz
            );
        }
    }

    // Allocate pages for BootInfo
    let boot_info_size = core::mem::size_of::<BootInfo>();
    let boot_info_pages = (boot_info_size + PAGE_SIZE as usize - 1) / PAGE_SIZE as usize;

    let boot_info_ptr: NonNull<u8> = match uefi::boot::allocate_pages(
        AllocateType::AnyPages,
        MemoryType::LOADER_DATA,
        boot_info_pages,
    ) {
        Ok(ptr) => ptr,
        Err(_) => {
            log::error!("[-] Failed to allocate memory for BootInfo");
            return Status::OUT_OF_RESOURCES;
        }
    };
    let boot_info_addr = boot_info_ptr.as_ptr() as u64;

    // 4. Query ACPI RSDP pointer from UEFI Configuration Table
    let rsdp_addr: Option<u64> = uefi::system::with_config_table(|entries| {
        for entry in entries {
            if entry.guid == uefi::table::cfg::ACPI2_GUID || entry.guid == uefi::table::cfg::ACPI_GUID {
                return Some(entry.address as u64);
            }
        }
        None
    });

    if let Some(addr) = rsdp_addr {
        info!("[+] Discovered ACPI RSDP at physical address: 0x{:X}", addr);
    }

    // 5. Exit Boot Services & get memory map
    let final_mmap = unsafe { uefi::boot::exit_boot_services(MemoryType::LOADER_DATA) };

    let boot_info_ptr = boot_info_addr as *mut BootInfo;

    unsafe {
        (*boot_info_ptr).magic = LUNIX_BOOT_MAGIC;
        (*boot_info_ptr).framebuffer = fb_info;
        (*boot_info_ptr).memory_map.entry_count = 0;

        for desc in final_mmap.entries() {
            let region_type = match desc.ty {
                MemoryType::CONVENTIONAL => MemoryRegionType::Usable,
                MemoryType::LOADER_CODE => MemoryRegionType::BootloaderCode,
                MemoryType::LOADER_DATA => MemoryRegionType::BootloaderData,
                MemoryType::BOOT_SERVICES_CODE | MemoryType::BOOT_SERVICES_DATA => {
                    MemoryRegionType::Reserved
                }
                MemoryType::RUNTIME_SERVICES_CODE | MemoryType::RUNTIME_SERVICES_DATA => {
                    MemoryRegionType::Reserved
                }
                MemoryType::ACPI_RECLAIM => MemoryRegionType::AcpiReclaimable,
                MemoryType::ACPI_NON_VOLATILE => MemoryRegionType::AcpiNvs,
                MemoryType::MMIO => MemoryRegionType::Mmio,
                MemoryType::MMIO_PORT_SPACE => MemoryRegionType::MmioPortSpace,
                MemoryType::PAL_CODE => MemoryRegionType::PalCode,
                MemoryType::PERSISTENT_MEMORY => MemoryRegionType::PersistentMemory,
                MemoryType::UNUSABLE => MemoryRegionType::BadMemory,
                _ => MemoryRegionType::Unknown,
            };

            let region = MemoryRegion {
                phys_start: desc.phys_start,
                page_count: desc.page_count,
                region_type,
            };

            (*boot_info_ptr).memory_map.add_region(region);
        }

        (*boot_info_ptr).rsdp_addr = rsdp_addr;
        (*boot_info_ptr).kernel_phys_base = kernel_phys_base;
        (*boot_info_ptr).kernel_virt_base = kernel_phys_base;
        (*boot_info_ptr).kernel_size = total_kernel_size as u64;
        (*boot_info_ptr).physical_memory_offset = HIGHER_HALF_OFFSET;
    }

    // Calculate Entry Point
    let entry_point_addr = kernel_phys_base + (elf_header.e_entry - min_vaddr);

    type KernelEntry = extern "sysv64" fn(&'static BootInfo) -> !;
    let kernel_entry: KernelEntry = unsafe { core::mem::transmute(entry_point_addr) };
    let static_boot_info: &'static BootInfo = unsafe { &*(boot_info_addr as *const BootInfo) };

    // Jump to Lunix kernel!
    kernel_entry(static_boot_info);
}
