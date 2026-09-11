//! Win32 User-Mode Subsystem for Lunix OS
//!
//! Provides PE32+ Windows 64-bit executable loader, IAT resolution for kernel32.dll / ntdll.dll,
//! Win32 API shims, and user-mode execution via extern "win64" ABI.

use alloc::string::String;
use core::sync::atomic::Ordering;
use spin::Mutex;
use x86_64::VirtAddr;

use super::pe::*;
use crate::fs::vfs;
use crate::mm::pmm;
use crate::mm::vmm;
use crate::task::scheduler;
use crate::task::user::enter_user_mode;
use crate::{lunix_print, lunix_println};

// Win32 Standard Handles
pub const STD_INPUT_HANDLE: i32 = -10;
pub const STD_OUTPUT_HANDLE: i32 = -11;
pub const STD_ERROR_HANDLE: i32 = -12;

// Win32 System Info
#[repr(C)]
pub struct SystemInfo {
    pub processor_architecture: u16,
    pub reserved: u16,
    pub page_size: u32,
    pub minimum_application_address: u64,
    pub maximum_application_address: u64,
    pub active_processor_mask: u64,
    pub number_of_processors: u32,
    pub processor_type: u32,
    pub allocation_granularity: u32,
    pub processor_level: u16,
    pub processor_revision: u16,
}

static WIN32_USER_HEAP: Mutex<u64> = Mutex::new(0x0000_7000_0000_0000);
static WIN32_CMDLINE: Mutex<String> = Mutex::new(String::new());

// Win32 VDSO User-Mode Thunk Base (Ring 3 accessible)
pub const WIN32_VDSO_BASE: u64 = 0x0000_7FFF_1000_0000;

pub const THUNK_GET_STD_HANDLE: usize = 0 * 32;
pub const THUNK_WRITE_CONSOLE_A: usize = 1 * 32;
pub const THUNK_WRITE_FILE: usize = 2 * 32;
pub const THUNK_READ_FILE: usize = 3 * 32;
pub const THUNK_EXIT_PROCESS: usize = 4 * 32;
pub const THUNK_GET_PROCESS_HEAP: usize = 5 * 32;
pub const THUNK_HEAP_ALLOC: usize = 6 * 32;
pub const THUNK_HEAP_FREE: usize = 7 * 32;
pub const THUNK_VIRTUAL_ALLOC: usize = 8 * 32;
pub const THUNK_VIRTUAL_FREE: usize = 9 * 32;
pub const THUNK_GET_SYSTEM_INFO: usize = 10 * 32;
pub const THUNK_GET_CURRENT_PROCESS_ID: usize = 11 * 32;
pub const THUNK_GET_CURRENT_THREAD_ID: usize = 12 * 32;
pub const THUNK_SLEEP: usize = 13 * 32;
pub const THUNK_GET_COMMAND_LINE_A: usize = 14 * 32;
pub const THUNK_GET_MODULE_HANDLE_A: usize = 15 * 32;

fn emit_win32_thunk(buf: &mut [u8; 4096], offset: usize, syscall_id: u32) {
    // Generates a 64-bit user-mode thunk stub translating Win64 calling convention to Syscall ABI:
    // 1. mov r10, r9               -> 4D 89 CA (arg4)
    // 2. mov rsi, rdx              -> 48 89 D6 (arg2)
    // 3. mov rdx, r8               -> 4C 89 C2 (arg3)
    // 4. mov rdi, rcx              -> 48 89 CF (arg1)
    // 5. mov eax, syscall_id       -> B8 <4 bytes LE>
    // 6. syscall                   -> 0F 05
    // 7. ret                       -> C3
    let mut code = [
        0x4D, 0x89, 0xCA,             // 0..3:  mov r10, r9
        0x48, 0x89, 0xD6,             // 3..6:  mov rsi, rdx
        0x4C, 0x89, 0xC2,             // 6..9:  mov rdx, r8
        0x48, 0x89, 0xCF,             // 9..12: mov rdi, rcx
        0xB8, 0x00, 0x00, 0x00, 0x00, // 12..17: mov eax, imm32
        0x0F, 0x05,                   // 17..19: syscall
        0xC3,                         // 19..20: ret
    ];
    let id_bytes = syscall_id.to_le_bytes();
    code[13..17].copy_from_slice(&id_bytes);

    buf[offset..offset + code.len()].copy_from_slice(&code);
}

// -----------------------------------------------------------------------------
// Win32 Kernel-Side Syscall Implementations
// -----------------------------------------------------------------------------

pub fn sys_win32_get_std_handle(n_std_handle: i32) -> u64 {
    match n_std_handle {
        STD_INPUT_HANDLE => 0,
        STD_OUTPUT_HANDLE => 1,
        STD_ERROR_HANDLE => 2,
        _ => 1,
    }
}

pub fn sys_win32_write_console(
    _h_console: u64,
    lp_buffer: *const u8,
    n_chars_to_write: u32,
    lp_chars_written: *mut u32,
) -> u32 {
    if lp_buffer.is_null() || n_chars_to_write == 0 {
        return 1;
    }

    let slice = unsafe { core::slice::from_raw_parts(lp_buffer, n_chars_to_write as usize) };
    if let Ok(s) = core::str::from_utf8(slice) {
        lunix_print!("{}", s);
    } else {
        for &b in slice {
            lunix_print!("{}", b as char);
        }
    }

    if !lp_chars_written.is_null() {
        unsafe {
            *lp_chars_written = n_chars_to_write;
        }
    }

    1 // TRUE
}

pub fn sys_win32_read_file(
    _h_file: u64,
    lp_buffer: *mut u8,
    n_bytes_to_read: u32,
    lp_bytes_read: *mut u32,
) -> u32 {
    if !lp_buffer.is_null() && n_bytes_to_read > 0 {
        unsafe {
            *lp_buffer = 0;
        }
    }
    if !lp_bytes_read.is_null() {
        unsafe {
            *lp_bytes_read = 0;
        }
    }
    1
}

pub fn sys_win32_exit_process(exit_code: u32) -> ! {
    crate::lunix_serial_println!("  [WIN32_SYS] ExitProcess invoked with code: {}", exit_code);
    lunix_println!("  [WIN32] Process terminated cleanly via ExitProcess({})", exit_code);
    crate::drivers::keyboard::print_prompt();
    scheduler::exit_current_thread();
}

pub fn sys_win32_get_process_heap() -> u64 {
    0x10000 // Handle to process heap
}

pub fn sys_win32_heap_alloc(_h_heap: u64, _dw_flags: u32, dw_bytes: usize) -> u64 {
    let mut heap = WIN32_USER_HEAP.lock();
    let addr = *heap;
    let pages = (dw_bytes + 4095) / 4096;
    for p in 0..pages {
        if let Some(frame) = pmm::alloc_frame() {
            let page_vaddr = VirtAddr::new(addr + (p as u64 * 4096));
            let flags = x86_64::structures::paging::PageTableFlags::PRESENT
                | x86_64::structures::paging::PageTableFlags::WRITABLE
                | x86_64::structures::paging::PageTableFlags::USER_ACCESSIBLE;
            let _ = vmm::map_page(page_vaddr, frame, flags);
            unsafe {
                core::ptr::write_bytes(frame.as_u64() as *mut u8, 0, 4096);
            }
        }
    }
    *heap += pages as u64 * 4096;
    addr
}

pub fn sys_win32_heap_free(_h_heap: u64, _dw_flags: u32, _lp_mem: u64) -> u32 {
    1 // TRUE
}

pub fn sys_win32_get_system_info(lp_system_info: *mut SystemInfo) {
    if lp_system_info.is_null() {
        return;
    }

    let cores = crate::arch::x86_64::smp::CPU_COUNT.load(Ordering::Relaxed) as u32;
    unsafe {
        let info = &mut *lp_system_info;
        info.processor_architecture = 9; // PROCESSOR_ARCHITECTURE_AMD64
        info.reserved = 0;
        info.page_size = 4096;
        info.minimum_application_address = 0x10000;
        info.maximum_application_address = 0x0000_7FFF_FFFF_0000;
        info.active_processor_mask = (1 << cores) - 1;
        info.number_of_processors = cores;
        info.processor_type = 8664; // AMD64
        info.allocation_granularity = 65536;
        info.processor_level = 6;
        info.processor_revision = 0;
    }
}

pub fn sys_win32_get_current_pid() -> u32 {
    scheduler::current_tid() as u32
}

pub fn sys_win32_get_current_tid() -> u32 {
    scheduler::current_tid() as u32
}

pub fn sys_win32_sleep(dw_milliseconds: u32) {
    scheduler::sleep_ms(dw_milliseconds as u64);
}

pub fn sys_win32_get_command_line() -> u64 {
    let cmd = WIN32_CMDLINE.lock();
    cmd.as_ptr() as u64
}

// -----------------------------------------------------------------------------
// Win32 Symbol Resolver (maps imports to user-mode VDSO thunk entry points)
// -----------------------------------------------------------------------------

pub fn resolve_win32_symbol(dll: &str, symbol: &str) -> Option<usize> {
    let is_kernel32 = dll.eq_ignore_ascii_case("kernel32.dll")
        || dll.eq_ignore_ascii_case("kernel32")
        || dll.eq_ignore_ascii_case("api-ms-win-core-processenvironment-l1-1-0.dll");

    let is_ntdll = dll.eq_ignore_ascii_case("ntdll.dll") || dll.eq_ignore_ascii_case("ntdll");

    if is_kernel32 || is_ntdll {
        match symbol {
            "GetStdHandle" => Some((WIN32_VDSO_BASE as usize) + THUNK_GET_STD_HANDLE),
            "WriteConsoleA" => Some((WIN32_VDSO_BASE as usize) + THUNK_WRITE_CONSOLE_A),
            "WriteFile" => Some((WIN32_VDSO_BASE as usize) + THUNK_WRITE_FILE),
            "ReadFile" => Some((WIN32_VDSO_BASE as usize) + THUNK_READ_FILE),
            "ExitProcess" => Some((WIN32_VDSO_BASE as usize) + THUNK_EXIT_PROCESS),
            "GetProcessHeap" => Some((WIN32_VDSO_BASE as usize) + THUNK_GET_PROCESS_HEAP),
            "HeapAlloc" => Some((WIN32_VDSO_BASE as usize) + THUNK_HEAP_ALLOC),
            "HeapFree" => Some((WIN32_VDSO_BASE as usize) + THUNK_HEAP_FREE),
            "VirtualAlloc" => Some((WIN32_VDSO_BASE as usize) + THUNK_VIRTUAL_ALLOC),
            "VirtualFree" => Some((WIN32_VDSO_BASE as usize) + THUNK_VIRTUAL_FREE),
            "GetSystemInfo" => Some((WIN32_VDSO_BASE as usize) + THUNK_GET_SYSTEM_INFO),
            "GetCurrentProcessId" => Some((WIN32_VDSO_BASE as usize) + THUNK_GET_CURRENT_PROCESS_ID),
            "GetCurrentThreadId" => Some((WIN32_VDSO_BASE as usize) + THUNK_GET_CURRENT_THREAD_ID),
            "Sleep" => Some((WIN32_VDSO_BASE as usize) + THUNK_SLEEP),
            "GetCommandLineA" => Some((WIN32_VDSO_BASE as usize) + THUNK_GET_COMMAND_LINE_A),
            "GetModuleHandleA" => Some((WIN32_VDSO_BASE as usize) + THUNK_GET_MODULE_HANDLE_A),
            "RtlAllocateHeap" => Some((WIN32_VDSO_BASE as usize) + THUNK_HEAP_ALLOC),
            "RtlFreeHeap" => Some((WIN32_VDSO_BASE as usize) + THUNK_HEAP_FREE),
            "RtlExitUserProcess" => Some((WIN32_VDSO_BASE as usize) + THUNK_EXIT_PROCESS),
            _ => {
                lunix_serial_println!("  [WIN32] Unresolved export: {}!{}", dll, symbol);
                None
            }
        }
    } else {
        None
    }
}

// -----------------------------------------------------------------------------
// Win32 PE32+ User Executable Loader & Runner
// -----------------------------------------------------------------------------

#[derive(Clone, Copy)]
pub struct Win32Process {
    pub entry_point: u64,
    pub image_base: u64,
    pub image_size: usize,
    pub stack_top: u64,
}

static CURRENT_WIN32_EXEC: Mutex<Option<Win32Process>> = Mutex::new(None);

fn win32_runner_trampoline() {
    let proc = {
        let lock = CURRENT_WIN32_EXEC.lock();
        lock.expect("No Win32 process loaded for execution")
    };

    lunix_println!("  [WIN32] Launching process in Ring 3 via extern \"win64\" ABI...");
    unsafe {
        enter_user_mode(proc.entry_point, proc.stack_top);
    }
}

pub fn load_win32_exe(data: &[u8]) -> Result<Win32Process, &'static str> {
    if data.len() < core::mem::size_of::<ImageDosHeader>() {
        return Err("File too small for Win32 DOS header");
    }

    let dos_header = unsafe { &*(data.as_ptr() as *const ImageDosHeader) };
    if dos_header.e_magic != IMAGE_DOS_SIGNATURE {
        return Err("Invalid Win32 DOS signature ('MZ')");
    }

    let pe_offset = dos_header.e_lfanew as usize;
    if pe_offset + 4 + core::mem::size_of::<ImageFileHeader>() > data.len() {
        return Err("Invalid PE header offset");
    }

    let pe_sig = unsafe { *(data.as_ptr().add(pe_offset) as *const u32) };
    if pe_sig != IMAGE_NT_SIGNATURE {
        return Err("Invalid PE signature ('PE\\0\\0')");
    }

    let file_header_ptr = unsafe { data.as_ptr().add(pe_offset + 4) as *const ImageFileHeader };
    let file_header = unsafe { *file_header_ptr };

    if file_header.machine != IMAGE_FILE_MACHINE_AMD64 {
        return Err("Win32 binary is not 64-bit AMD64 / x86_64");
    }

    let opt_header_offset = pe_offset + 4 + core::mem::size_of::<ImageFileHeader>();
    let opt_header_ptr = unsafe { data.as_ptr().add(opt_header_offset) as *const ImageOptionalHeader64 };
    let opt_header = unsafe { *opt_header_ptr };

    let image_size = (opt_header.size_of_image as usize + 4095) & !4095;
    let base_vaddr = if opt_header.image_base != 0 {
        opt_header.image_base
    } else {
        0x0040_0000
    };

    // Allocate physical pages for the image
    let num_pages = image_size / 4096;
    for p in 0..num_pages {
        let vaddr = VirtAddr::new(base_vaddr + (p as u64 * 4096));
        if let Some(frame) = pmm::alloc_frame() {
            let flags = x86_64::structures::paging::PageTableFlags::PRESENT
                | x86_64::structures::paging::PageTableFlags::WRITABLE
                | x86_64::structures::paging::PageTableFlags::USER_ACCESSIBLE;
            let _ = vmm::map_page(vaddr, frame, flags);
            unsafe {
                core::ptr::write_bytes(frame.as_u64() as *mut u8, 0, 4096);
            }
        }
    }

    // Copy PE headers
    let header_size = (opt_header.size_of_headers as usize).min(data.len());
    unsafe {
        core::ptr::copy_nonoverlapping(data.as_ptr(), base_vaddr as *mut u8, header_size);
    }

    // Copy Sections
    let sections_offset = opt_header_offset + (file_header.size_of_optional_header as usize);
    let section_headers = unsafe {
        core::slice::from_raw_parts(
            data.as_ptr().add(sections_offset) as *const ImageSectionHeader,
            file_header.number_of_sections as usize,
        )
    };

    for section in section_headers {
        let raw_ptr = section.pointer_to_raw_data as usize;
        let raw_size = section.size_of_raw_data as usize;
        let virt_addr = section.virtual_address as usize;
        let virt_size = section.virtual_size as usize;

        if raw_ptr + raw_size <= data.len() && virt_addr + virt_size <= image_size {
            let copy_len = raw_size.min(virt_size);
            if copy_len > 0 {
                unsafe {
                    core::ptr::copy_nonoverlapping(
                        data.as_ptr().add(raw_ptr),
                        (base_vaddr + virt_addr as u64) as *mut u8,
                        copy_len,
                    );
                }
            }
        }
    }

    // Base Relocations
    let reloc_dir = opt_header.data_directory[IMAGE_DIRECTORY_ENTRY_BASERELOC];
    if reloc_dir.virtual_address != 0 && reloc_dir.size != 0 {
        let delta = base_vaddr.wrapping_sub(opt_header.image_base);
        if delta != 0 {
            let mut current_offset = reloc_dir.virtual_address as usize;
            let end_offset = current_offset + (reloc_dir.size as usize);

            while current_offset < end_offset {
                let block = unsafe { &*((base_vaddr + current_offset as u64) as *const ImageBaseRelocation) };
                if block.size_of_block == 0 {
                    break;
                }

                let entries_count = (block.size_of_block as usize - core::mem::size_of::<ImageBaseRelocation>()) / 2;
                let entries_ptr = (base_vaddr + current_offset as u64 + core::mem::size_of::<ImageBaseRelocation>() as u64) as *const u16;

                for i in 0..entries_count {
                    let entry = unsafe { *entries_ptr.add(i) };
                    let reloc_type = entry >> 12;
                    let reloc_offset = (entry & 0x0FFF) as usize;
                    let target_rva = (block.virtual_address as usize) + reloc_offset;

                    if target_rva + 8 <= image_size {
                        let target_ptr = (base_vaddr + target_rva as u64) as *mut u64;
                        if reloc_type == IMAGE_REL_BASED_DIR64 {
                            unsafe {
                                let val = *target_ptr;
                                *target_ptr = val.wrapping_add(delta);
                            }
                        }
                    }
                }

                current_offset += block.size_of_block as usize;
            }
        }
    }

    // Map and initialize Win32 User-Mode VDSO Thunk Page
    let vdso_vaddr = VirtAddr::new(WIN32_VDSO_BASE);
    if let Some(frame) = pmm::alloc_frame() {
        let flags = x86_64::structures::paging::PageTableFlags::PRESENT
            | x86_64::structures::paging::PageTableFlags::WRITABLE
            | x86_64::structures::paging::PageTableFlags::USER_ACCESSIBLE;
        let _ = vmm::map_page(vdso_vaddr, frame, flags);

        let mut vdso_buf = [0u8; 4096];
        emit_win32_thunk(&mut vdso_buf, THUNK_GET_STD_HANDLE, 0x1000);
        emit_win32_thunk(&mut vdso_buf, THUNK_WRITE_CONSOLE_A, 0x1001);
        emit_win32_thunk(&mut vdso_buf, THUNK_WRITE_FILE, 0x1002);
        emit_win32_thunk(&mut vdso_buf, THUNK_READ_FILE, 0x1003);
        emit_win32_thunk(&mut vdso_buf, THUNK_EXIT_PROCESS, 0x1004);
        emit_win32_thunk(&mut vdso_buf, THUNK_GET_PROCESS_HEAP, 0x1005);
        emit_win32_thunk(&mut vdso_buf, THUNK_HEAP_ALLOC, 0x1006);
        emit_win32_thunk(&mut vdso_buf, THUNK_HEAP_FREE, 0x1007);
        emit_win32_thunk(&mut vdso_buf, THUNK_VIRTUAL_ALLOC, 0x1008);
        emit_win32_thunk(&mut vdso_buf, THUNK_VIRTUAL_FREE, 0x1009);
        emit_win32_thunk(&mut vdso_buf, THUNK_GET_SYSTEM_INFO, 0x100A);
        emit_win32_thunk(&mut vdso_buf, THUNK_GET_CURRENT_PROCESS_ID, 0x100B);
        emit_win32_thunk(&mut vdso_buf, THUNK_GET_CURRENT_THREAD_ID, 0x100C);
        emit_win32_thunk(&mut vdso_buf, THUNK_SLEEP, 0x100D);
        emit_win32_thunk(&mut vdso_buf, THUNK_GET_COMMAND_LINE_A, 0x100E);
        emit_win32_thunk(&mut vdso_buf, THUNK_GET_MODULE_HANDLE_A, 0x100F);

        unsafe {
            core::ptr::copy_nonoverlapping(
                vdso_buf.as_ptr(),
                frame.as_u64() as *mut u8,
                4096,
            );
            core::ptr::copy_nonoverlapping(
                vdso_buf.as_ptr(),
                vdso_vaddr.as_u64() as *mut u8,
                4096,
            );
        }
    }

    // Process Import Table (IAT resolution)
    let import_dir = opt_header.data_directory[IMAGE_DIRECTORY_ENTRY_IMPORT];
    if import_dir.virtual_address != 0 && import_dir.size != 0 {
        let mut descriptor_ptr = (base_vaddr + import_dir.virtual_address as u64) as *const ImageImportDescriptor;

        unsafe {
            while (*descriptor_ptr).name != 0 {
                let desc = *descriptor_ptr;
                let dll_name_ptr = (base_vaddr + desc.name as u64) as *const u8;
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

                let mut thunk_ptr = (base_vaddr + thunk_rva as u64) as *const usize;
                let mut iat_ptr = (base_vaddr + desc.first_thunk as u64) as *mut usize;

                while *thunk_ptr != 0 {
                    let thunk_val = *thunk_ptr;
                    if (thunk_val & (1 << 63)) == 0 {
                        // Import by name
                        let import_by_name_ptr = (base_vaddr + (thunk_val & 0x7FFF_FFFF) as u64) as *const u8;
                        let func_name_ptr = import_by_name_ptr.add(2);
                        let mut func_len = 0;
                        while *func_name_ptr.add(func_len) != 0 {
                            func_len += 1;
                        }
                        let func_name = core::str::from_utf8(core::slice::from_raw_parts(func_name_ptr, func_len)).unwrap_or("");

                        if let Some(resolved_addr) = resolve_win32_symbol(dll_name, func_name) {
                            *iat_ptr = resolved_addr;
                        }
                    }

                    thunk_ptr = thunk_ptr.add(1);
                    iat_ptr = iat_ptr.add(1);
                }

                descriptor_ptr = descriptor_ptr.add(1);
            }
        }
    }

    // Allocate 64 KiB user stack
    let user_stack_base = 0x0000_7FFF_2000_0000u64;
    for p in 0..16 {
        let vaddr = VirtAddr::new(user_stack_base + (p * 4096));
        if let Some(frame) = pmm::alloc_frame() {
            let flags = x86_64::structures::paging::PageTableFlags::PRESENT
                | x86_64::structures::paging::PageTableFlags::WRITABLE
                | x86_64::structures::paging::PageTableFlags::USER_ACCESSIBLE;
            let _ = vmm::map_page(vaddr, frame, flags);
            unsafe {
                core::ptr::write_bytes(frame.as_u64() as *mut u8, 0, 4096);
            }
        }
    }

    let stack_top = user_stack_base + (16 * 4096) - 128; // Shadow space alignment
    let entry_point = base_vaddr + opt_header.address_of_entry_point as u64;

    Ok(Win32Process {
        entry_point,
        image_base: base_vaddr,
        image_size,
        stack_top,
    })
}

pub fn exec_win32_pe(path: &str) -> Result<usize, &'static str> {
    lunix_println!("[WIN32] Loading 64-bit Windows PE32+ executable: '{}'...", path);
    let bytes = vfs::read_to_vec(path).map_err(|_| "Failed to read Win32 executable from VFS")?;

    let process = load_win32_exe(&bytes)?;

    lunix_println!(
        "  [WIN32] Process mapped at Base: 0x{:X}, Entry Point: 0x{:X}, Size: {} KB",
        process.image_base,
        process.entry_point,
        process.image_size / 1024
    );

    *CURRENT_WIN32_EXEC.lock() = Some(process);

    lunix_println!("[+] Starting Windows process '{}' (PE32+)...", path);
    let tid = scheduler::spawn("win32_app", win32_runner_trampoline, 6);
    Ok(tid)
}
