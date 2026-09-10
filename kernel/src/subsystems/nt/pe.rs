use crate::lunix_serial_println;
use crate::subsystems::nt::types::*;
use alloc::alloc::{alloc, Layout};


pub const IMAGE_DOS_SIGNATURE: u16 = 0x5A4D; // "MZ"
pub const IMAGE_NT_SIGNATURE: u32 = 0x00004550; // "PE\0\0"
pub const IMAGE_FILE_MACHINE_AMD64: u16 = 0x8664; // x86_64

pub const IMAGE_DIRECTORY_ENTRY_EXPORT: usize = 0;
pub const IMAGE_DIRECTORY_ENTRY_IMPORT: usize = 1;
pub const IMAGE_DIRECTORY_ENTRY_RESOURCE: usize = 2;
pub const IMAGE_DIRECTORY_ENTRY_EXCEPTION: usize = 3;
pub const IMAGE_DIRECTORY_ENTRY_SECURITY: usize = 4;
pub const IMAGE_DIRECTORY_ENTRY_BASERELOC: usize = 5;

pub const IMAGE_REL_BASED_ABSOLUTE: u16 = 0;
pub const IMAGE_REL_BASED_HIGH: u16 = 1;
pub const IMAGE_REL_BASED_LOW: u16 = 2;
pub const IMAGE_REL_BASED_HIGHLOW: u16 = 3;
pub const IMAGE_REL_BASED_HIGHADJ: u16 = 4;
pub const IMAGE_REL_BASED_DIR64: u16 = 10;

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct ImageDosHeader {
    pub e_magic: u16,
    pub e_cblp: u16,
    pub e_cp: u16,
    pub e_crlc: u16,
    pub e_cparhdr: u16,
    pub e_minalloc: u16,
    pub e_maxalloc: u16,
    pub e_ss: u16,
    pub e_sp: u16,
    pub e_csum: u16,
    pub e_ip: u16,
    pub e_cs: u16,
    pub e_lfarlc: u16,
    pub e_ovno: u16,
    pub e_res: [u16; 4],
    pub e_oemid: u16,
    pub e_oeminfo: u16,
    pub e_res2: [u16; 10],
    pub e_lfanew: i32, // Offset to PE Header
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct ImageFileHeader {
    pub machine: u16,
    pub number_of_sections: u16,
    pub time_date_stamp: u32,
    pub pointer_to_symbol_table: u32,
    pub number_of_symbols: u32,
    pub size_of_optional_header: u16,
    pub characteristics: u16,
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct ImageDataDirectory {
    pub virtual_address: u32,
    pub size: u32,
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct ImageOptionalHeader64 {
    pub magic: u16,
    pub major_linker_version: u8,
    pub minor_linker_version: u8,
    pub size_of_code: u32,
    pub size_of_initialized_data: u32,
    pub size_of_uninitialized_data: u32,
    pub address_of_entry_point: u32,
    pub base_of_code: u32,
    pub image_base: u64,
    pub section_alignment: u32,
    pub file_alignment: u32,
    pub major_os_version: u16,
    pub minor_os_version: u16,
    pub major_image_version: u16,
    pub minor_image_version: u16,
    pub major_subsystem_version: u16,
    pub minor_subsystem_version: u16,
    pub win32_version_value: u32,
    pub size_of_image: u32,
    pub size_of_headers: u32,
    pub check_sum: u32,
    pub subsystem: u16,
    pub dll_characteristics: u16,
    pub size_of_stack_reserve: u64,
    pub size_of_stack_commit: u64,
    pub size_of_heap_reserve: u64,
    pub size_of_heap_commit: u64,
    pub loader_flags: u32,
    pub number_of_rva_and_sizes: u32,
    pub data_directory: [ImageDataDirectory; 16],
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct ImageSectionHeader {
    pub name: [u8; 8],
    pub virtual_size: u32,
    pub virtual_address: u32,
    pub size_of_raw_data: u32,
    pub pointer_to_raw_data: u32,
    pub pointer_to_relocations: u32,
    pub pointer_to_linenumbers: u32,
    pub number_of_relocations: u16,
    pub number_of_linenumbers: u16,
    pub characteristics: u32,
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct ImageBaseRelocation {
    pub virtual_address: u32,
    pub size_of_block: u32,
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct ImageImportDescriptor {
    pub original_first_thunk: u32,
    pub time_date_stamp: u32,
    pub forwarder_chain: u32,
    pub name: u32,
    pub first_thunk: u32,
}

pub fn resolve_symbol(dll: &str, symbol: &str) -> Option<usize> {
    let dll_upper_match = dll.eq_ignore_ascii_case("ntoskrnl.exe")
        || dll.eq_ignore_ascii_case("ntoskrnl")
        || dll.eq_ignore_ascii_case("ntkrnlpa.exe")
        || dll.eq_ignore_ascii_case("ntkrnlmp.exe");

    let hal_match = dll.eq_ignore_ascii_case("hal.dll") || dll.eq_ignore_ascii_case("hal");

    if dll_upper_match {
        match symbol {
            "ExAllocatePoolWithTag" => Some(crate::subsystems::nt::ntoskrnl::ExAllocatePoolWithTag as *const () as usize),
            "ExFreePoolWithTag" => Some(crate::subsystems::nt::ntoskrnl::ExFreePoolWithTag as *const () as usize),
            "MmMapIoSpace" => Some(crate::subsystems::nt::ntoskrnl::MmMapIoSpace as *const () as usize),
            "MmUnmapIoSpace" => Some(crate::subsystems::nt::ntoskrnl::MmUnmapIoSpace as *const () as usize),
            "KeInitializeSpinLock" => Some(crate::subsystems::nt::ntoskrnl::KeInitializeSpinLock as *const () as usize),
            "KeAcquireSpinLock" => Some(crate::subsystems::nt::ntoskrnl::KeAcquireSpinLock as *const () as usize),
            "KeReleaseSpinLock" => Some(crate::subsystems::nt::ntoskrnl::KeReleaseSpinLock as *const () as usize),
            "IoCreateDevice" => Some(crate::subsystems::nt::ntoskrnl::IoCreateDevice as *const () as usize),
            "IoDeleteDevice" => Some(crate::subsystems::nt::ntoskrnl::IoDeleteDevice as *const () as usize),
            "IoCompleteRequest" => Some(crate::subsystems::nt::ntoskrnl::IoCompleteRequest as *const () as usize),
            "IoCallDriver" => Some(crate::subsystems::nt::ntoskrnl::IoCallDriver as *const () as usize),
            "IoCreateSymbolicLink" => Some(crate::subsystems::nt::ntoskrnl::IoCreateSymbolicLink as *const () as usize),
            "IoDeleteSymbolicLink" => Some(crate::subsystems::nt::ntoskrnl::IoDeleteSymbolicLink as *const () as usize),
            "DbgPrint" => Some(crate::subsystems::nt::ntoskrnl::DbgPrint as *const () as usize),
            "DbgPrintEx" => Some(crate::subsystems::nt::ntoskrnl::DbgPrintEx as *const () as usize),
            "RtlInitUnicodeString" => Some(crate::subsystems::nt::ntoskrnl::RtlInitUnicodeString as *const () as usize),
            "RtlInitAnsiString" => Some(crate::subsystems::nt::ntoskrnl::RtlInitAnsiString as *const () as usize),
            "KeStallExecutionProcessor" => Some(crate::subsystems::nt::ntoskrnl::KeStallExecutionProcessor as *const () as usize),
            _ => None,
        }
    } else if hal_match {
        match symbol {
            "READ_PORT_UCHAR" => Some(crate::subsystems::nt::hal::READ_PORT_UCHAR as *const () as usize),
            "WRITE_PORT_UCHAR" => Some(crate::subsystems::nt::hal::WRITE_PORT_UCHAR as *const () as usize),
            "READ_PORT_USHORT" => Some(crate::subsystems::nt::hal::READ_PORT_USHORT as *const () as usize),
            "WRITE_PORT_USHORT" => Some(crate::subsystems::nt::hal::WRITE_PORT_USHORT as *const () as usize),
            "READ_PORT_ULONG" => Some(crate::subsystems::nt::hal::READ_PORT_ULONG as *const () as usize),
            "WRITE_PORT_ULONG" => Some(crate::subsystems::nt::hal::WRITE_PORT_ULONG as *const () as usize),
            "HalGetBusDataByOffset" => Some(crate::subsystems::nt::hal::HalGetBusDataByOffset as *const () as usize),
            _ => None,
        }
    } else {
        None
    }
}

pub struct LoadedDriver {
    pub image_base: *mut u8,
    pub image_size: usize,
    pub driver_object: PDRIVER_OBJECT,
    pub entry_point: PDRIVER_INITIALIZE,
}

pub unsafe fn load_driver(data: &[u8]) -> Result<LoadedDriver, &'static str> {
    if data.len() < core::mem::size_of::<ImageDosHeader>() {
        return Err("File too small for DOS header");
    }

    let dos_header = &*(data.as_ptr() as *const ImageDosHeader);
    if dos_header.e_magic != IMAGE_DOS_SIGNATURE {
        return Err("Invalid DOS signature");
    }

    let pe_offset = dos_header.e_lfanew as usize;
    if pe_offset + 4 + core::mem::size_of::<ImageFileHeader>() > data.len() {
        return Err("Invalid PE offset");
    }

    let pe_sig = *(data.as_ptr().add(pe_offset) as *const u32);
    if pe_sig != IMAGE_NT_SIGNATURE {
        return Err("Invalid PE signature");
    }

    let file_header_ptr = data.as_ptr().add(pe_offset + 4) as *const ImageFileHeader;
    let file_header = *file_header_ptr;

    if file_header.machine != IMAGE_FILE_MACHINE_AMD64 {
        return Err("PE is not AMD64 / x86_64");
    }

    let opt_header_offset = pe_offset + 4 + core::mem::size_of::<ImageFileHeader>();
    let opt_header_ptr = data.as_ptr().add(opt_header_offset) as *const ImageOptionalHeader64;
    let opt_header = *opt_header_ptr;

    let image_size = opt_header.size_of_image as usize;
    let layout = Layout::from_size_align(image_size, 4096).map_err(|_| "Invalid layout")?;
    let image_mem = alloc(layout);
    if image_mem.is_null() {
        return Err("Failed to allocate virtual memory for PE driver");
    }

    // Zero allocated memory
    core::ptr::write_bytes(image_mem, 0, image_size);

    // Copy headers
    let header_size = (opt_header.size_of_headers as usize).min(data.len());
    core::ptr::copy_nonoverlapping(data.as_ptr(), image_mem, header_size);

    // Copy sections
    let sections_offset = opt_header_offset + (file_header.size_of_optional_header as usize);
    let section_headers = core::slice::from_raw_parts(
        data.as_ptr().add(sections_offset) as *const ImageSectionHeader,
        file_header.number_of_sections as usize,
    );

    for section in section_headers {
        let raw_ptr = section.pointer_to_raw_data as usize;
        let raw_size = section.size_of_raw_data as usize;
        let virt_addr = section.virtual_address as usize;
        let virt_size = section.virtual_size as usize;

        if raw_ptr + raw_size <= data.len() && virt_addr + virt_size <= image_size {
            let copy_len = raw_size.min(virt_size);
            if copy_len > 0 {
                core::ptr::copy_nonoverlapping(
                    data.as_ptr().add(raw_ptr),
                    image_mem.add(virt_addr),
                    copy_len,
                );
            }
        }
    }

    // Process Base Relocations
    let reloc_dir = opt_header.data_directory[IMAGE_DIRECTORY_ENTRY_BASERELOC];
    if reloc_dir.virtual_address != 0 && reloc_dir.size != 0 {
        let delta = (image_mem as u64).wrapping_sub(opt_header.image_base);
        if delta != 0 {
            let mut current_offset = reloc_dir.virtual_address as usize;
            let end_offset = current_offset + (reloc_dir.size as usize);

            while current_offset < end_offset {
                let block = &*(image_mem.add(current_offset) as *const ImageBaseRelocation);
                if block.size_of_block == 0 {
                    break;
                }

                let entries_count = (block.size_of_block as usize - core::mem::size_of::<ImageBaseRelocation>()) / 2;
                let entries_ptr = image_mem.add(current_offset + core::mem::size_of::<ImageBaseRelocation>()) as *const u16;

                for i in 0..entries_count {
                    let entry = *entries_ptr.add(i);
                    let reloc_type = entry >> 12;
                    let reloc_offset = (entry & 0x0FFF) as usize;
                    let target_rva = (block.virtual_address as usize) + reloc_offset;

                    if target_rva + 8 <= image_size {
                        let target_ptr = image_mem.add(target_rva);
                        match reloc_type {
                            IMAGE_REL_BASED_DIR64 => {
                                let val = *(target_ptr as *const u64);
                                *(target_ptr as *mut u64) = val.wrapping_add(delta);
                            }
                            IMAGE_REL_BASED_HIGHLOW => {
                                let val = *(target_ptr as *const u32);
                                *(target_ptr as *mut u32) = (val as u64).wrapping_add(delta) as u32;
                            }
                            IMAGE_REL_BASED_ABSOLUTE => {} // Padding
                            _ => {}
                        }
                    }
                }

                current_offset += block.size_of_block as usize;
            }
        }
    }

    // Process Imports (IAT Resolution)
    let import_dir = opt_header.data_directory[IMAGE_DIRECTORY_ENTRY_IMPORT];
    if import_dir.virtual_address != 0 && import_dir.size != 0 {
        let mut descriptor_ptr = image_mem.add(import_dir.virtual_address as usize) as *const ImageImportDescriptor;

        while (*descriptor_ptr).name != 0 {
            let desc = *descriptor_ptr;
            let dll_name_ptr = image_mem.add(desc.name as usize);
            let mut dll_len = 0;
            while *dll_name_ptr.add(dll_len) != 0 {
                dll_len += 1;
            }
            let dll_name = core::str::from_utf8(core::slice::from_raw_parts(dll_name_ptr, dll_len)).unwrap_or("");

            let thunk_rva = if desc.original_first_thunk != 0 {
                desc.original_first_thunk
            } else {
                desc.first_thunk
            };

            let mut thunk_ptr = image_mem.add(thunk_rva as usize) as *const usize;
            let mut iat_ptr = image_mem.add(desc.first_thunk as usize) as *mut usize;

            while *thunk_ptr != 0 {
                let thunk_val = *thunk_ptr;
                if (thunk_val & (1 << 63)) == 0 {
                    // Import by name
                    let import_by_name_ptr = image_mem.add((thunk_val & 0x7FFF_FFFF) as usize);
                    let func_name_ptr = import_by_name_ptr.add(2);
                    let mut func_len = 0;
                    while *func_name_ptr.add(func_len) != 0 {
                        func_len += 1;
                    }
                    let func_name = core::str::from_utf8(core::slice::from_raw_parts(func_name_ptr, func_len)).unwrap_or("");

                    if let Some(resolved_addr) = resolve_symbol(dll_name, func_name) {
                        *iat_ptr = resolved_addr;
                    } else {
                        lunix_serial_println!("[-] Unresolved Windows driver import: {}!{}", dll_name, func_name);
                    }
                }

                thunk_ptr = thunk_ptr.add(1);
                iat_ptr = iat_ptr.add(1);
            }

            descriptor_ptr = descriptor_ptr.add(1);
        }
    }

    let entry_point_addr = (image_mem as usize) + (opt_header.address_of_entry_point as usize);
    let entry_point: PDRIVER_INITIALIZE = core::mem::transmute(entry_point_addr);

    // Allocate DRIVER_OBJECT
    let driver_obj_layout = Layout::new::<DRIVER_OBJECT>();
    let driver_obj = alloc(driver_obj_layout) as PDRIVER_OBJECT;
    if driver_obj.is_null() {
        return Err("Failed to allocate DRIVER_OBJECT");
    }

    core::ptr::write_bytes(driver_obj as *mut u8, 0, core::mem::size_of::<DRIVER_OBJECT>());
    (*driver_obj).type_code = 4; // IO_TYPE_DRIVER
    (*driver_obj).size = core::mem::size_of::<DRIVER_OBJECT>() as i16;
    (*driver_obj).driver_start = image_mem as PVOID;
    (*driver_obj).driver_size = image_size as u32;
    (*driver_obj).driver_init = Some(entry_point);

    Ok(LoadedDriver {
        image_base: image_mem,
        image_size,
        driver_object: driver_obj,
        entry_point,
    })
}
