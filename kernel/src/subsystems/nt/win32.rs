//! Win32 User-Mode Subsystem for Lunix OS
//!
//! Provides PE32+ Windows 64-bit executable loader, IAT resolution for kernel32.dll / ntdll.dll,
//! Win32 API shims, anonymous pipes, file streaming, directory searching, and user-mode execution via extern "win64" ABI.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicUsize, Ordering};
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

// Win32 File Attributes
pub const FILE_ATTRIBUTE_READONLY: u32 = 0x01;
pub const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;
pub const FILE_ATTRIBUTE_ARCHIVE: u32 = 0x20;
pub const FILE_ATTRIBUTE_NORMAL: u32 = 0x80;

// Win32 Registry Constants
pub const HKEY_CLASSES_ROOT: u64 = 0x8000_0000;
pub const HKEY_CURRENT_USER: u64 = 0x8000_0001;
pub const HKEY_LOCAL_MACHINE: u64 = 0x8000_0002;
pub const HKEY_USERS: u64 = 0x8000_0003;

pub const REG_NONE: u32 = 0;
pub const REG_SZ: u32 = 1;
pub const REG_EXPAND_SZ: u32 = 2;
pub const REG_BINARY: u32 = 3;
pub const REG_DWORD: u32 = 4;
pub const REG_MULTI_SZ: u32 = 7;
pub const REG_QWORD: u32 = 11;

pub const ERROR_SUCCESS: u32 = 0;
pub const ERROR_FILE_NOT_FOUND: u32 = 2;
pub const ERROR_INVALID_PARAMETER: u32 = 87;
pub const ERROR_MORE_DATA: u32 = 234;

// Win32 Find Data
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Win32FindDataA {
    pub dw_file_attributes: u32,
    pub ft_creation_time: [u32; 2],
    pub ft_last_access_time: [u32; 2],
    pub ft_last_write_time: [u32; 2],
    pub n_file_size_high: u32,
    pub n_file_size_low: u32,
    pub dw_reserved0: u32,
    pub dw_reserved1: u32,
    pub c_file_name: [u8; 260],
    pub c_alternate_file_name: [u8; 14],
}

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

// Win32 Process Information & Startup Info
#[repr(C)]
pub struct ProcessInformation {
    pub h_process: u64,
    pub h_thread: u64,
    pub dw_process_id: u32,
    pub dw_thread_id: u32,
}

#[repr(C)]
pub struct StartupInfoA {
    pub cb: u32,
    pub lp_reserved: *const u8,
    pub lp_desktop: *const u8,
    pub lp_title: *const u8,
    pub dw_x: u32,
    pub dw_y: u32,
    pub dw_x_size: u32,
    pub dw_y_size: u32,
    pub dw_x_count_chars: u32,
    pub dw_y_count_chars: u32,
    pub dw_fill_attribute: u32,
    pub dw_flags: u32,
    pub w_show_window: u16,
    pub cb_reserved2: u16,
    pub lp_reserved2: *const u8,
    pub h_std_input: u64,
    pub h_std_output: u64,
    pub h_std_error: u64,
}

struct Win32FindSearch {
    id: usize,
    entries: Vec<crate::fs::file::DirectoryEntry>,
    current_idx: usize,
}

static WIN32_USER_HEAP: Mutex<u64> = Mutex::new(0x0000_7000_0000_0000);
static WIN32_CMDLINE: Mutex<String> = Mutex::new(String::new());
static WIN32_SEARCHES: Mutex<Vec<Win32FindSearch>> = Mutex::new(Vec::new());
static NEXT_SEARCH_ID: AtomicUsize = AtomicUsize::new(0x3000);

// In-Memory Environment & Registry Storage
static WIN32_ENV: Mutex<Option<BTreeMap<String, String>>> = Mutex::new(None);
static WIN32_REGISTRY: Mutex<Option<BTreeMap<String, BTreeMap<String, (u32, Vec<u8>)>>>> = Mutex::new(None);
static WIN32_OPEN_REG_KEYS: Mutex<BTreeMap<u64, String>> = Mutex::new(BTreeMap::new());
static NEXT_REG_HANDLE: AtomicUsize = AtomicUsize::new(0x4000);

fn get_win32_env_map() -> spin::MutexGuard<'static, Option<BTreeMap<String, String>>> {
    let mut lock = WIN32_ENV.lock();
    if lock.is_none() {
        let mut map = BTreeMap::new();
        map.insert(String::from("OS"), String::from("Windows_NT"));
        map.insert(String::from("PROCESSOR_ARCHITECTURE"), String::from("AMD64"));
        map.insert(String::from("NUMBER_OF_PROCESSORS"), String::from("2"));
        map.insert(String::from("PATH"), String::from("C:\\Windows\\system32;C:\\Windows;C:\\bin"));
        map.insert(String::from("SYSTEMROOT"), String::from("C:\\Windows"));
        map.insert(String::from("WINDIR"), String::from("C:\\Windows"));
        map.insert(String::from("USERPROFILE"), String::from("C:\\Users\\Lunix"));
        map.insert(String::from("USERNAME"), String::from("LunixUser"));
        map.insert(String::from("COMPUTERNAME"), String::from("LUNIX-PC"));
        map.insert(String::from("TEMP"), String::from("C:\\Temp"));
        map.insert(String::from("TMP"), String::from("C:\\Temp"));
        map.insert(String::from("PROMPT"), String::from("$P$G"));
        *lock = Some(map);
    }
    lock
}

fn get_win32_registry() -> spin::MutexGuard<'static, Option<BTreeMap<String, BTreeMap<String, (u32, Vec<u8>)>>>> {
    let mut lock = WIN32_REGISTRY.lock();
    if lock.is_none() {
        let mut reg = BTreeMap::new();

        // Populate HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion
        let mut cv = BTreeMap::new();
        cv.insert(String::from("PRODUCTNAME"), (REG_SZ, b"Lunix NT 10.0 (x86_64 Hybrid OS)\0".to_vec()));
        cv.insert(String::from("CURRENTVERSION"), (REG_SZ, b"10.0\0".to_vec()));
        cv.insert(String::from("CURRENTBUILD"), (REG_SZ, b"26100\0".to_vec()));
        cv.insert(String::from("REGISTEREDOWNER"), (REG_SZ, b"Lunix Administrator\0".to_vec()));
        cv.insert(String::from("SYSTEMROOT"), (REG_SZ, b"C:\\Windows\0".to_vec()));
        reg.insert(String::from("HKLM\\SOFTWARE\\MICROSOFT\\WINDOWS NT\\CURRENTVERSION"), cv);

        // Populate HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Environment
        let mut env_key = BTreeMap::new();
        env_key.insert(String::from("OS"), (REG_SZ, b"Windows_NT\0".to_vec()));
        env_key.insert(String::from("PROCESSOR_ARCHITECTURE"), (REG_SZ, b"AMD64\0".to_vec()));
        env_key.insert(String::from("NUMBER_OF_PROCESSORS"), (REG_SZ, b"2\0".to_vec()));
        env_key.insert(String::from("PATH"), (REG_SZ, b"C:\\Windows\\system32;C:\\Windows\0".to_vec()));
        reg.insert(String::from("HKLM\\SYSTEM\\CURRENTCONTROLSET\\CONTROL\\SESSION MANAGER\\ENVIRONMENT"), env_key);

        *lock = Some(reg);
    }
    lock
}

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
pub const THUNK_CREATE_PROCESS_A: usize = 16 * 32;
pub const THUNK_WAIT_FOR_SINGLE_OBJECT: usize = 17 * 32;
pub const THUNK_GET_EXIT_CODE_PROCESS: usize = 18 * 32;
pub const THUNK_VIRTUAL_PROTECT: usize = 19 * 32;
pub const THUNK_CREATE_PIPE: usize = 20 * 32;
pub const THUNK_SET_STD_HANDLE: usize = 21 * 32;
pub const THUNK_CREATE_FILE_A: usize = 22 * 32;
pub const THUNK_CLOSE_HANDLE: usize = 23 * 32;
pub const THUNK_FIND_FIRST_FILE_A: usize = 24 * 32;
pub const THUNK_FIND_NEXT_FILE_A: usize = 25 * 32;
pub const THUNK_FIND_CLOSE: usize = 26 * 32;
pub const THUNK_GET_ENVIRONMENT_VARIABLE_A: usize = 27 * 32;
pub const THUNK_SET_ENVIRONMENT_VARIABLE_A: usize = 28 * 32;
pub const THUNK_REG_OPEN_KEY_EX_A: usize = 29 * 32;
pub const THUNK_REG_QUERY_VALUE_EX_A: usize = 30 * 32;
pub const THUNK_REG_CLOSE_KEY: usize = 31 * 32;

fn emit_win32_thunk(buf: &mut [u8; 4096], offset: usize, syscall_id: u32) {
    // Generates a 64-bit user-mode thunk stub translating Win64 calling convention to Syscall ABI:
    // 1. mov r10, r9               -> 4D 89 CA (arg4)
    // 2. mov rsi, rdx              -> 48 89 D6 (arg2)
    // 3. mov rdx, r8               -> 4C 89 C2 (arg3)
    // 4. mov rdi, rcx              -> 48 89 CF (arg1)
    // 5. mov r8, [rsp + 0x28]      -> 4C 8B 44 24 28 (arg5 from win64 stack)
    // 6. mov r9, [rsp + 0x30]      -> 4C 8B 4C 24 30 (arg6 from win64 stack)
    // 7. mov eax, syscall_id       -> B8 <4 bytes LE>
    // 8. syscall                   -> 0F 05
    // 9. ret                       -> C3
    let mut code = [
        0x4D, 0x89, 0xCA,             // 0..3:  mov r10, r9
        0x48, 0x89, 0xD6,             // 3..6:  mov rsi, rdx
        0x4C, 0x89, 0xC2,             // 6..9:  mov rdx, r8
        0x48, 0x89, 0xCF,             // 9..12: mov rdi, rcx
        0x4C, 0x8B, 0x44, 0x24, 0x28, // 12..17: mov r8, [rsp + 0x28]
        0x4C, 0x8B, 0x4C, 0x24, 0x30, // 17..22: mov r9, [rsp + 0x30]
        0xB8, 0x00, 0x00, 0x00, 0x00, // 22..27: mov eax, imm32
        0x0F, 0x05,                   // 27..29: syscall
        0xC3,                         // 29..30: ret
    ];
    let id_bytes = syscall_id.to_le_bytes();
    code[23..27].copy_from_slice(&id_bytes);

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

pub fn sys_win32_write_file(
    h_file: u64,
    lp_buffer: *const u8,
    n_bytes_to_write: u32,
    lp_bytes_written: *mut u32,
) -> u32 {
    let res = crate::syscall::sys_write(h_file as usize, lp_buffer, n_bytes_to_write as usize);
    if res >= 0 {
        if !lp_bytes_written.is_null() {
            unsafe {
                *lp_bytes_written = res as u32;
            }
        }
        1 // TRUE
    } else {
        0 // FALSE
    }
}

pub fn sys_win32_read_file(
    h_file: u64,
    lp_buffer: *mut u8,
    n_bytes_to_read: u32,
    lp_bytes_read: *mut u32,
) -> u32 {
    let res = crate::syscall::sys_read(h_file as usize, lp_buffer, n_bytes_to_read as usize);
    if res >= 0 {
        if !lp_bytes_read.is_null() {
            unsafe {
                *lp_bytes_read = res as u32;
            }
        }
        1 // TRUE
    } else {
        0 // FALSE
    }
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

pub fn sys_win32_create_process(
    lp_app_name: *const u8,
    lp_cmd_line: *const u8,
    lp_proc_info: *mut ProcessInformation,
) -> u32 {
    let app_str = if !lp_app_name.is_null() {
        let mut len = 0;
        unsafe {
            while *lp_app_name.add(len) != 0 && len < 256 {
                len += 1;
            }
            core::str::from_utf8(core::slice::from_raw_parts(lp_app_name, len)).unwrap_or("")
        }
    } else if !lp_cmd_line.is_null() {
        let mut len = 0;
        unsafe {
            while *lp_cmd_line.add(len) != 0 && len < 256 {
                len += 1;
            }
            core::str::from_utf8(core::slice::from_raw_parts(lp_cmd_line, len)).unwrap_or("")
        }
    } else {
        return 0; // FALSE
    };

    match exec_win32_pe(app_str) {
        Ok(tid) => {
            if !lp_proc_info.is_null() {
                unsafe {
                    let info = &mut *lp_proc_info;
                    info.h_process = tid as u64;
                    info.h_thread = tid as u64;
                    info.dw_process_id = tid as u32;
                    info.dw_thread_id = tid as u32;
                }
            }
            1 // TRUE
        }
        Err(_) => 0, // FALSE
    }
}

pub fn sys_win32_wait_for_single_object(h_handle: u64, _dw_milliseconds: u32) -> u32 {
    let target_tid = h_handle as usize;
    for _ in 0..100 {
        let is_dead = {
            let threads = scheduler::list_threads();
            if let Some((_, _, state, _)) = threads.iter().find(|(tid, _, _, _)| *tid == target_tid) {
                *state == crate::task::thread::ThreadState::Dead
            } else {
                true // Process already reaped
            }
        };

        if is_dead {
            return 0; // WAIT_OBJECT_0
        }
        scheduler::sleep_ms(10);
    }
    0 // WAIT_OBJECT_0
}

pub fn sys_win32_get_exit_code_process(h_process: u64, lp_exit_code: *mut u32) -> u32 {
    let pid = h_process as usize;
    if !lp_exit_code.is_null() {
        if let Some(proc_arc) = scheduler::get_process(pid) {
            let proc = proc_arc.lock();
            let code = proc.exit_code.unwrap_or(0) as u32;
            unsafe {
                *lp_exit_code = code;
            }
        } else {
            unsafe {
                *lp_exit_code = 0;
            }
        }
    }
    1 // TRUE
}

pub fn sys_win32_virtual_protect(
    _lp_address: u64,
    _dw_size: usize,
    _fl_new_protect: u32,
    lpfl_old_protect: *mut u32,
) -> u32 {
    if !lpfl_old_protect.is_null() {
        unsafe {
            *lpfl_old_protect = 0x04; // PAGE_READWRITE
        }
    }
    1 // TRUE
}

pub fn sys_win32_create_pipe(
    ph_read_pipe: *mut u64,
    ph_write_pipe: *mut u64,
    _lp_pipe_attributes: *const u8,
    _n_size: u32,
) -> u32 {
    if ph_read_pipe.is_null() || ph_write_pipe.is_null() {
        return 0; // FALSE
    }
    let (reader, writer) = crate::task::pipe::create_pipe_pair();
    if let Some(proc_arc) = scheduler::get_current_process() {
        let mut proc = proc_arc.lock();
        if let Some(read_fd) = proc.allocate_fd(crate::task::process::FileDescriptor {
            target: crate::task::process::FdTarget::PipeRead(reader),
            flags: 0,
        }) {
            if let Some(write_fd) = proc.allocate_fd(crate::task::process::FileDescriptor {
                target: crate::task::process::FdTarget::PipeWrite(writer),
                flags: 0,
            }) {
                unsafe {
                    *ph_read_pipe = read_fd as u64;
                    *ph_write_pipe = write_fd as u64;
                }
                return 1; // TRUE
            } else {
                proc.close_fd(read_fd);
            }
        }
    }
    0 // FALSE
}

pub fn sys_win32_set_std_handle(n_std_handle: i32, h_handle: u64) -> u32 {
    let target_fd = match n_std_handle {
        STD_INPUT_HANDLE => 0,
        STD_OUTPUT_HANDLE => 1,
        STD_ERROR_HANDLE => 2,
        _ => return 0,
    };
    if let Some(proc_arc) = scheduler::get_current_process() {
        let mut proc = proc_arc.lock();
        if proc.dup_fd(h_handle as usize, target_fd).is_some() {
            return 1; // TRUE
        }
    }
    0
}

pub fn sys_win32_create_file(
    lp_file_name: *const u8,
    _dw_desired_access: u32,
    _dw_share_mode: u32,
    _lp_sec: *const u8,
    _dw_disp: u32,
) -> u64 {
    if lp_file_name.is_null() {
        return u64::MAX;
    }
    let mut len = 0;
    unsafe {
        while *lp_file_name.add(len) != 0 && len < 256 {
            len += 1;
        }
    }
    let slice = unsafe { core::slice::from_raw_parts(lp_file_name, len) };
    if let Ok(path) = core::str::from_utf8(slice) {
        if let Some(proc_arc) = scheduler::get_current_process() {
            let mut proc = proc_arc.lock();
            if let Ok(data) = vfs::read_to_vec(path) {
                let size = data.len();
                if let Some(fd) = proc.allocate_fd(crate::task::process::FileDescriptor {
                    target: crate::task::process::FdTarget::File {
                        path: alloc::string::String::from(path),
                        offset: 0,
                        size,
                        data,
                    },
                    flags: 0,
                }) {
                    return fd as u64;
                }
            }
        }
    }
    u64::MAX // INVALID_HANDLE_VALUE
}

pub fn sys_win32_close_handle(h_object: u64) -> u32 {
    let fd = h_object as usize;
    if let Some(proc_arc) = scheduler::get_current_process() {
        let mut proc = proc_arc.lock();
        if proc.close_fd(fd) {
            return 1; // TRUE
        }
    }
    if sys_win32_find_close(h_object) == 1 {
        return 1;
    }
    0 // FALSE
}

fn sanitize_find_path(raw: &str) -> &str {
    let p = raw.trim();
    if p == "*" || p == "*.*" || p == "/*" || p == "/*.*" {
        return "/";
    }
    let p = p.strip_suffix("/*.*").unwrap_or(p);
    let p = p.strip_suffix("/*").unwrap_or(p);
    let p = p.strip_suffix("\\*.*").unwrap_or(p);
    let p = p.strip_suffix("\\*").unwrap_or(p);
    if p.is_empty() {
        "/"
    } else {
        p
    }
}

pub fn sys_win32_find_first_file(
    lp_file_name: *const u8,
    lp_find_data: *mut Win32FindDataA,
) -> u64 {
    if lp_file_name.is_null() || lp_find_data.is_null() {
        return u64::MAX;
    }

    let mut len = 0;
    unsafe {
        while *lp_file_name.add(len) != 0 && len < 256 {
            len += 1;
        }
    }
    let slice = unsafe { core::slice::from_raw_parts(lp_file_name, len) };
    let raw_path = match core::str::from_utf8(slice) {
        Ok(s) => s,
        Err(_) => return u64::MAX,
    };

    let search_path = sanitize_find_path(raw_path);
    let entries = match vfs::read_dir(search_path) {
        Ok(e) => e,
        Err(_) => return u64::MAX,
    };

    if entries.is_empty() {
        return u64::MAX;
    }

    let first = &entries[0];
    unsafe {
        fill_find_data(lp_find_data, first);
    }

    let search_id = NEXT_SEARCH_ID.fetch_add(1, Ordering::SeqCst);
    let mut searches = WIN32_SEARCHES.lock();
    searches.push(Win32FindSearch {
        id: search_id,
        entries,
        current_idx: 1,
    });

    search_id as u64
}

pub fn sys_win32_find_next_file(h_find_file: u64, lp_find_data: *mut Win32FindDataA) -> u32 {
    if lp_find_data.is_null() {
        return 0; // FALSE
    }

    let search_id = h_find_file as usize;
    let mut searches = WIN32_SEARCHES.lock();
    if let Some(search) = searches.iter_mut().find(|s| s.id == search_id) {
        if search.current_idx < search.entries.len() {
            let entry = &search.entries[search.current_idx];
            unsafe {
                fill_find_data(lp_find_data, entry);
            }
            search.current_idx += 1;
            1 // TRUE
        } else {
            0 // FALSE (no more files)
        }
    } else {
        0 // FALSE
    }
}

pub fn sys_win32_find_close(h_find_file: u64) -> u32 {
    let search_id = h_find_file as usize;
    let mut searches = WIN32_SEARCHES.lock();
    if let Some(pos) = searches.iter().position(|s| s.id == search_id) {
        searches.swap_remove(pos);
        1 // TRUE
    } else {
        0 // FALSE
    }
}

unsafe fn fill_find_data(dest: *mut Win32FindDataA, entry: &crate::fs::file::DirectoryEntry) {
    let data = &mut *dest;
    core::ptr::write_bytes(dest as *mut u8, 0, core::mem::size_of::<Win32FindDataA>());
    data.dw_file_attributes = if entry.node_type == crate::fs::inode::INodeType::Directory {
        FILE_ATTRIBUTE_DIRECTORY
    } else {
        FILE_ATTRIBUTE_NORMAL
    };
    data.n_file_size_low = (entry.size & 0xFFFF_FFFF) as u32;
    data.n_file_size_high = ((entry.size >> 32) & 0xFFFF_FFFF) as u32;
    let name_bytes = entry.name.as_bytes();
    let copy_len = name_bytes.len().min(259);
    core::ptr::copy_nonoverlapping(name_bytes.as_ptr(), data.c_file_name.as_mut_ptr(), copy_len);
    data.c_file_name[copy_len] = 0;
}

pub fn sys_win32_get_environment_variable(
    lp_name: *const u8,
    lp_buffer: *mut u8,
    n_size: u32,
) -> u32 {
    if lp_name.is_null() {
        return 0;
    }
    let mut len = 0;
    unsafe {
        while *lp_name.add(len) != 0 && len < 256 {
            len += 1;
        }
    }
    let name_str = match core::str::from_utf8(unsafe { core::slice::from_raw_parts(lp_name, len) }) {
        Ok(s) => s.to_ascii_uppercase(),
        Err(_) => return 0,
    };

    let guard = get_win32_env_map();
    let map = guard.as_ref().unwrap();

    if let Some(val) = map.get(&name_str) {
        let val_bytes = val.as_bytes();
        let needed_with_null = val_bytes.len() + 1;

        if lp_buffer.is_null() || (n_size as usize) < needed_with_null {
            return needed_with_null as u32;
        }

        unsafe {
            core::ptr::copy_nonoverlapping(val_bytes.as_ptr(), lp_buffer, val_bytes.len());
            *lp_buffer.add(val_bytes.len()) = 0;
        }

        val_bytes.len() as u32
    } else {
        0
    }
}

pub fn sys_win32_set_environment_variable(
    lp_name: *const u8,
    lp_value: *const u8,
) -> u32 {
    if lp_name.is_null() {
        return 0; // FALSE
    }
    let mut len = 0;
    unsafe {
        while *lp_name.add(len) != 0 && len < 256 {
            len += 1;
        }
    }
    let name_str = match core::str::from_utf8(unsafe { core::slice::from_raw_parts(lp_name, len) }) {
        Ok(s) => s.to_ascii_uppercase(),
        Err(_) => return 0,
    };

    let mut guard = get_win32_env_map();
    let map = guard.as_mut().unwrap();

    if lp_value.is_null() {
        map.remove(&name_str);
    } else {
        let mut val_len = 0;
        unsafe {
            while *lp_value.add(val_len) != 0 && val_len < 1024 {
                val_len += 1;
            }
        }
        let val_str = match core::str::from_utf8(unsafe { core::slice::from_raw_parts(lp_value, val_len) }) {
            Ok(s) => String::from(s),
            Err(_) => return 0,
        };
        map.insert(name_str, val_str);
    }

    1 // TRUE
}

pub fn sys_win32_reg_open_key_ex(
    h_key: u64,
    lp_subkey: *const u8,
    _options: u32,
    _sam: u32,
    phk_result: *mut u64,
) -> u32 {
    if phk_result.is_null() {
        return ERROR_INVALID_PARAMETER;
    }

    let base_path = match h_key {
        HKEY_CLASSES_ROOT => String::from("HKCR"),
        HKEY_CURRENT_USER => String::from("HKCU"),
        HKEY_LOCAL_MACHINE => String::from("HKLM"),
        HKEY_USERS => String::from("HKU"),
        other => {
            let open_keys = WIN32_OPEN_REG_KEYS.lock();
            match open_keys.get(&other) {
                Some(p) => p.clone(),
                None => return ERROR_FILE_NOT_FOUND,
            }
        }
    };

    let subkey_str = if !lp_subkey.is_null() {
        let mut len = 0;
        unsafe {
            while *lp_subkey.add(len) != 0 && len < 512 {
                len += 1;
            }
        }
        core::str::from_utf8(unsafe { core::slice::from_raw_parts(lp_subkey, len) }).unwrap_or("")
    } else {
        ""
    };

    let mut full_path = base_path;
    let trimmed = subkey_str.trim_matches('\\');
    if !trimmed.is_empty() {
        full_path.push('\\');
        full_path.push_str(trimmed);
    }
    let upper_path = full_path.to_ascii_uppercase();

    // Verify key exists in registry or is a prefix of known keys
    let guard = get_win32_registry();
    let reg = guard.as_ref().unwrap();

    let key_exists = reg.keys().any(|k| k == &upper_path || k.starts_with(&upper_path));
    if !key_exists {
        return ERROR_FILE_NOT_FOUND;
    }

    let handle = NEXT_REG_HANDLE.fetch_add(1, Ordering::SeqCst) as u64;
    WIN32_OPEN_REG_KEYS.lock().insert(handle, upper_path);

    unsafe {
        *phk_result = handle;
        *(phk_result as *mut u32) = handle as u32;
    }

    ERROR_SUCCESS
}

pub fn sys_win32_reg_query_value_ex(
    h_key: u64,
    lp_val_name: *const u8,
    _reserved: *mut u32,
    lp_type: *mut u32,
    lp_data: *mut u8,
    lpcb_data: *mut u32,
) -> u32 {
    let key_path = match h_key {
        HKEY_CLASSES_ROOT => String::from("HKCR"),
        HKEY_CURRENT_USER => String::from("HKCU"),
        HKEY_LOCAL_MACHINE => String::from("HKLM"),
        HKEY_USERS => String::from("HKU"),
        other => {
            let open_keys = WIN32_OPEN_REG_KEYS.lock();
            match open_keys.get(&other) {
                Some(p) => p.clone(),
                None => return ERROR_FILE_NOT_FOUND,
            }
        }
    };

    let val_name = if !lp_val_name.is_null() {
        let mut len = 0;
        unsafe {
            while *lp_val_name.add(len) != 0 && len < 256 {
                len += 1;
            }
        }
        match core::str::from_utf8(unsafe { core::slice::from_raw_parts(lp_val_name, len) }) {
            Ok(s) => s.to_ascii_uppercase(),
            Err(_) => return ERROR_FILE_NOT_FOUND,
        }
    } else {
        String::new()
    };

    let guard = get_win32_registry();
    let reg = guard.as_ref().unwrap();

    let values_map = match reg.get(&key_path) {
        Some(m) => m,
        None => return ERROR_FILE_NOT_FOUND,
    };

    let (vtype, vdata) = match values_map.get(&val_name) {
        Some(pair) => pair,
        None => return ERROR_FILE_NOT_FOUND,
    };

    if !lp_type.is_null() {
        unsafe {
            *lp_type = *vtype;
        }
    }

    if !lpcb_data.is_null() {
        let buf_size = unsafe { *lpcb_data } as usize;
        unsafe {
            *lpcb_data = vdata.len() as u32;
        }

        if !lp_data.is_null() {
            if buf_size < vdata.len() {
                return ERROR_MORE_DATA;
            }
            unsafe {
                core::ptr::copy_nonoverlapping(vdata.as_ptr(), lp_data, vdata.len());
            }
        }
    }

    ERROR_SUCCESS
}

pub fn sys_win32_reg_close_key(h_key: u64) -> u32 {
    let mut open_keys = WIN32_OPEN_REG_KEYS.lock();
    open_keys.remove(&h_key);
    ERROR_SUCCESS
}

// -----------------------------------------------------------------------------
// Win32 Symbol Resolver (maps imports to user-mode VDSO thunk entry points)
// -----------------------------------------------------------------------------

pub fn resolve_win32_symbol(dll: &str, symbol: &str) -> Option<usize> {
    let is_kernel32 = dll.eq_ignore_ascii_case("kernel32.dll")
        || dll.eq_ignore_ascii_case("kernel32")
        || dll.eq_ignore_ascii_case("api-ms-win-core-processenvironment-l1-1-0.dll")
        || dll.eq_ignore_ascii_case("api-ms-win-core-processthreads-l1-1-0.dll")
        || dll.eq_ignore_ascii_case("api-ms-win-core-memory-l1-1-0.dll")
        || dll.eq_ignore_ascii_case("api-ms-win-core-file-l1-1-0.dll")
        || dll.eq_ignore_ascii_case("api-ms-win-core-handle-l1-1-0.dll");

    let is_advapi32 = dll.eq_ignore_ascii_case("advapi32.dll")
        || dll.eq_ignore_ascii_case("advapi32")
        || dll.eq_ignore_ascii_case("api-ms-win-core-registry-l1-1-0.dll");

    let is_ntdll = dll.eq_ignore_ascii_case("ntdll.dll") || dll.eq_ignore_ascii_case("ntdll");

    if is_kernel32 || is_advapi32 || is_ntdll {
        match symbol {
            "GetStdHandle" => Some((WIN32_VDSO_BASE as usize) + THUNK_GET_STD_HANDLE),
            "WriteConsoleA" => Some((WIN32_VDSO_BASE as usize) + THUNK_WRITE_CONSOLE_A),
            "WriteFile" => Some((WIN32_VDSO_BASE as usize) + THUNK_WRITE_FILE),
            "ReadFile" => Some((WIN32_VDSO_BASE as usize) + THUNK_READ_FILE),
            "ExitProcess" => Some((WIN32_VDSO_BASE as usize) + THUNK_EXIT_PROCESS),
            "GetProcessHeap" => Some((WIN32_VDSO_BASE as usize) + THUNK_GET_PROCESS_HEAP),
            "HeapAlloc" => Some((WIN32_VDSO_BASE as usize) + THUNK_HEAP_ALLOC),
            "HeapFree" => Some((WIN32_VDSO_BASE as usize) + THUNK_HEAP_FREE),
            "VirtualAlloc" | "VirtualAllocEx" => Some((WIN32_VDSO_BASE as usize) + THUNK_VIRTUAL_ALLOC),
            "VirtualFree" | "VirtualFreeEx" => Some((WIN32_VDSO_BASE as usize) + THUNK_VIRTUAL_FREE),
            "VirtualProtect" | "VirtualProtectEx" => Some((WIN32_VDSO_BASE as usize) + THUNK_VIRTUAL_PROTECT),
            "GetSystemInfo" => Some((WIN32_VDSO_BASE as usize) + THUNK_GET_SYSTEM_INFO),
            "GetCurrentProcessId" => Some((WIN32_VDSO_BASE as usize) + THUNK_GET_CURRENT_PROCESS_ID),
            "GetCurrentThreadId" => Some((WIN32_VDSO_BASE as usize) + THUNK_GET_CURRENT_THREAD_ID),
            "Sleep" => Some((WIN32_VDSO_BASE as usize) + THUNK_SLEEP),
            "GetCommandLineA" => Some((WIN32_VDSO_BASE as usize) + THUNK_GET_COMMAND_LINE_A),
            "GetModuleHandleA" => Some((WIN32_VDSO_BASE as usize) + THUNK_GET_MODULE_HANDLE_A),
            "CreateProcessA" => Some((WIN32_VDSO_BASE as usize) + THUNK_CREATE_PROCESS_A),
            "WaitForSingleObject" => Some((WIN32_VDSO_BASE as usize) + THUNK_WAIT_FOR_SINGLE_OBJECT),
            "GetExitCodeProcess" => Some((WIN32_VDSO_BASE as usize) + THUNK_GET_EXIT_CODE_PROCESS),
            "CreatePipe" => Some((WIN32_VDSO_BASE as usize) + THUNK_CREATE_PIPE),
            "SetStdHandle" => Some((WIN32_VDSO_BASE as usize) + THUNK_SET_STD_HANDLE),
            "CreateFileA" | "CreateFileW" => Some((WIN32_VDSO_BASE as usize) + THUNK_CREATE_FILE_A),
            "CloseHandle" => Some((WIN32_VDSO_BASE as usize) + THUNK_CLOSE_HANDLE),
            "FindFirstFileA" | "FindFirstFileExA" => Some((WIN32_VDSO_BASE as usize) + THUNK_FIND_FIRST_FILE_A),
            "FindNextFileA" => Some((WIN32_VDSO_BASE as usize) + THUNK_FIND_NEXT_FILE_A),
            "FindClose" => Some((WIN32_VDSO_BASE as usize) + THUNK_FIND_CLOSE),
            "GetEnvironmentVariableA" => Some((WIN32_VDSO_BASE as usize) + THUNK_GET_ENVIRONMENT_VARIABLE_A),
            "SetEnvironmentVariableA" => Some((WIN32_VDSO_BASE as usize) + THUNK_SET_ENVIRONMENT_VARIABLE_A),
            "RegOpenKeyExA" => Some((WIN32_VDSO_BASE as usize) + THUNK_REG_OPEN_KEY_EX_A),
            "RegQueryValueExA" => Some((WIN32_VDSO_BASE as usize) + THUNK_REG_QUERY_VALUE_EX_A),
            "RegCloseKey" => Some((WIN32_VDSO_BASE as usize) + THUNK_REG_CLOSE_KEY),
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
        emit_win32_thunk(&mut vdso_buf, THUNK_CREATE_PROCESS_A, 0x1010);
        emit_win32_thunk(&mut vdso_buf, THUNK_WAIT_FOR_SINGLE_OBJECT, 0x1011);
        emit_win32_thunk(&mut vdso_buf, THUNK_GET_EXIT_CODE_PROCESS, 0x1012);
        emit_win32_thunk(&mut vdso_buf, THUNK_VIRTUAL_PROTECT, 0x1013);
        emit_win32_thunk(&mut vdso_buf, THUNK_CREATE_PIPE, 0x1014);
        emit_win32_thunk(&mut vdso_buf, THUNK_SET_STD_HANDLE, 0x1015);
        emit_win32_thunk(&mut vdso_buf, THUNK_CREATE_FILE_A, 0x1016);
        emit_win32_thunk(&mut vdso_buf, THUNK_CLOSE_HANDLE, 0x1017);
        emit_win32_thunk(&mut vdso_buf, THUNK_FIND_FIRST_FILE_A, 0x1018);
        emit_win32_thunk(&mut vdso_buf, THUNK_FIND_NEXT_FILE_A, 0x1019);
        emit_win32_thunk(&mut vdso_buf, THUNK_FIND_CLOSE, 0x101A);
        emit_win32_thunk(&mut vdso_buf, THUNK_GET_ENVIRONMENT_VARIABLE_A, 0x101B);
        emit_win32_thunk(&mut vdso_buf, THUNK_SET_ENVIRONMENT_VARIABLE_A, 0x101C);
        emit_win32_thunk(&mut vdso_buf, THUNK_REG_OPEN_KEY_EX_A, 0x101D);
        emit_win32_thunk(&mut vdso_buf, THUNK_REG_QUERY_VALUE_EX_A, 0x101E);
        emit_win32_thunk(&mut vdso_buf, THUNK_REG_CLOSE_KEY, 0x101F);

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
                            lunix_serial_println!("  [WIN32_IAT] {}!{} -> 0x{:X} at {:p}", dll_name, func_name, resolved_addr, iat_ptr);
                            *iat_ptr = resolved_addr;
                        } else {
                            lunix_serial_println!("  [WIN32_IAT] UNRESOLVED: {}!{} at {:p}", dll_name, func_name, iat_ptr);
                        }
                    }

                    thunk_ptr = thunk_ptr.add(1);
                    iat_ptr = iat_ptr.add(1);
                }

                descriptor_ptr = descriptor_ptr.add(1);
            }
        }
    }

    // Allocate 128 KiB user stack
    let user_stack_base = 0x0000_7FFF_2000_0000u64;
    let stack_pages = 32;
    for p in 0..stack_pages {
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

    let stack_top = user_stack_base + (stack_pages * 4096) - 512; // Shadow space & alignment
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

    let pid = scheduler::allocate_pid();
    let (cr3_frame, _) = x86_64::registers::control::Cr3::read();
    let ppid = scheduler::current_pid();
    let proc = crate::task::process::Process::new_user(pid, ppid, path, cr3_frame.start_address().as_u64());
    scheduler::register_process(proc);

    if ppid != pid {
        if let Some(parent_proc) = scheduler::get_process(ppid) {
            parent_proc.lock().children.push(pid);
        }
    }

    lunix_println!("[+] Starting Windows process '{}' (PID: {}, PPID: {})...", path, pid, ppid);
    let tid = scheduler::spawn_with_pid("win32_app", pid, win32_runner_trampoline, 6);
    Ok(tid)
}
