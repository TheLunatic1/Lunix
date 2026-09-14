use crate::task::process::FdTarget;
use crate::{lunix_print, lunix_println};
use core::slice;
use spin::Mutex;

// Linux x86_64 standard syscall numbers
pub const LINUX_SYS_READ: usize = 0;
pub const LINUX_SYS_WRITE: usize = 1;
pub const LINUX_SYS_OPEN: usize = 2;
pub const LINUX_SYS_CLOSE: usize = 3;
pub const LINUX_SYS_STAT: usize = 4;
pub const LINUX_SYS_FSTAT: usize = 5;
pub const LINUX_SYS_POLL: usize = 7;
pub const LINUX_SYS_LSEEK: usize = 8;
pub const LINUX_SYS_MMAP: usize = 9;
pub const LINUX_SYS_MUNMAP: usize = 11;
pub const LINUX_SYS_BRK: usize = 12;
pub const LINUX_SYS_RT_SIGACTION: usize = 13;
pub const LINUX_SYS_RT_SIGPROCMASK: usize = 14;
pub const LINUX_SYS_IOCTL: usize = 16;
pub const LINUX_SYS_PIPE: usize = 22;
pub const LINUX_SYS_SCHED_YIELD: usize = 24;
pub const LINUX_SYS_DUP: usize = 32;
pub const LINUX_SYS_DUP2: usize = 33;
pub const LINUX_SYS_NANOSLEEP: usize = 35;
pub const LINUX_SYS_GETPID: usize = 39;
pub const LINUX_SYS_SOCKET: usize = 41;
pub const LINUX_SYS_CONNECT: usize = 42;
pub const LINUX_SYS_ACCEPT: usize = 43;
pub const LINUX_SYS_SENDTO: usize = 44;
pub const LINUX_SYS_RECVFROM: usize = 45;
pub const LINUX_SYS_BIND: usize = 49;
pub const LINUX_SYS_LISTEN: usize = 50;
pub const LINUX_SYS_CLONE: usize = 56;
pub const LINUX_SYS_FORK: usize = 57;
pub const LINUX_SYS_EXECVE: usize = 59;
pub const LINUX_SYS_EXIT: usize = 60;
pub const LINUX_SYS_WAIT4: usize = 61;
pub const LINUX_SYS_UNAME: usize = 63;
pub const LINUX_SYS_FCNTL: usize = 72;
pub const LINUX_SYS_GETCWD: usize = 79;
pub const LINUX_SYS_CHDIR: usize = 80;
pub const LINUX_SYS_READLINK: usize = 89;
pub const LINUX_SYS_GETPPID: usize = 110;
pub const LINUX_SYS_GETDENTS64: usize = 217;
pub const LINUX_SYS_EXIT_GROUP: usize = 231;
pub const LINUX_SYS_OPENAT: usize = 257;
pub const LINUX_SYS_MKDIRAT: usize = 258;
pub const LINUX_SYS_FSTATAT: usize = 262;
pub const LINUX_SYS_DUP3: usize = 292;
pub const LINUX_SYS_PIPE2: usize = 293;

// Win32 User-Mode Subsystem syscall numbers
pub const WIN32_SYS_GETSTDHANDLE: usize = 0x1000;
pub const WIN32_SYS_WRITECONSOLE: usize = 0x1001;
pub const WIN32_SYS_WRITEFILE: usize = 0x1002;
pub const WIN32_SYS_READFILE: usize = 0x1003;
pub const WIN32_SYS_EXITPROCESS: usize = 0x1004;
pub const WIN32_SYS_GETPROCESSHEAP: usize = 0x1005;
pub const WIN32_SYS_HEAPALLOC: usize = 0x1006;
pub const WIN32_SYS_HEAPFREE: usize = 0x1007;
pub const WIN32_SYS_VIRTUALALLOC: usize = 0x1008;
pub const WIN32_SYS_VIRTUALFREE: usize = 0x1009;
pub const WIN32_SYS_GETSYSTEMINFO: usize = 0x100A;
pub const WIN32_SYS_GETCURRENTPID: usize = 0x100B;
pub const WIN32_SYS_GETCURRENTTID: usize = 0x100C;
pub const WIN32_SYS_SLEEP: usize = 0x100D;
pub const WIN32_SYS_GETCOMMANDLINE: usize = 0x100E;
pub const WIN32_SYS_GETMODULEHANDLE: usize = 0x100F;
pub const WIN32_SYS_CREATEPROCESS: usize = 0x1010;
pub const WIN32_SYS_WAITFORSINGLEOBJECT: usize = 0x1011;
pub const WIN32_SYS_GETEXITCODEPROCESS: usize = 0x1012;
pub const WIN32_SYS_VIRTUALPROTECT: usize = 0x1013;
pub const WIN32_SYS_CREATEPIPE: usize = 0x1014;
pub const WIN32_SYS_SETSTDHANDLE: usize = 0x1015;
pub const WIN32_SYS_CREATEFILE: usize = 0x1016;
pub const WIN32_SYS_CLOSEHANDLE: usize = 0x1017;
pub const WIN32_SYS_FINDFIRSTFILE: usize = 0x1018;
pub const WIN32_SYS_FINDNEXTFILE: usize = 0x1019;
pub const WIN32_SYS_FINDCLOSE: usize = 0x101A;
pub const WIN32_SYS_GETENVIRONMENTVARIABLE: usize = 0x101B;
pub const WIN32_SYS_SETENVIRONMENTVARIABLE: usize = 0x101C;
pub const WIN32_SYS_REGOPENKEYEX: usize = 0x101D;
pub const WIN32_SYS_REGQUERYVALUEEX: usize = 0x101E;
pub const WIN32_SYS_REGCLOSEKEY: usize = 0x101F;

static USER_BRK: Mutex<u64> = Mutex::new(0x0000_6000_0000_0000);

#[repr(C)]
#[derive(Clone, Copy)]
pub struct LinuxTimeSpec {
    pub tv_sec: i64,
    pub tv_nsec: i64,
}

#[repr(C)]
pub struct LinuxUtsName {
    pub sysname: [u8; 65],
    pub nodename: [u8; 65],
    pub release: [u8; 65],
    pub version: [u8; 65],
    pub machine: [u8; 65],
    pub domainname: [u8; 65],
}

#[repr(C)]
pub struct LinuxStat {
    pub st_dev: u64,
    pub st_ino: u64,
    pub st_nlink: u64,
    pub st_mode: u32,
    pub st_uid: u32,
    pub st_gid: u32,
    pub __pad0: u32,
    pub st_rdev: u64,
    pub st_size: i64,
    pub st_blksize: i64,
    pub st_blocks: i64,
    pub st_atime: i64,
    pub st_atime_nsec: i64,
    pub st_mtime: i64,
    pub st_mtime_nsec: i64,
    pub st_ctime: i64,
    pub st_ctime_nsec: i64,
    pub __unused: [i64; 3],
}

#[no_mangle]
pub extern "C" fn syscall_dispatcher(
    num: usize,
    arg1: u64,
    arg2: u64,
    arg3: u64,
    arg4: u64,
    arg5: u64,
    arg6: u64,
) -> u64 {
    crate::lunix_serial_println!("  [SYSCALL_DISPATCH] num=0x{:X}, a1=0x{:X}, a2=0x{:X}, a3=0x{:X}", num, arg1, arg2, arg3);
    let ret = match num {
        // Standard Linux syscalls
        LINUX_SYS_READ => sys_read(arg1 as usize, arg2 as *mut u8, arg3 as usize) as u64,
        LINUX_SYS_WRITE => sys_write(arg1 as usize, arg2 as *const u8, arg3 as usize) as u64,
        LINUX_SYS_OPEN => sys_open(arg1 as *const u8, arg2 as usize, arg3 as u32) as u64,
        LINUX_SYS_CLOSE => sys_close(arg1 as usize) as u64,
        LINUX_SYS_STAT => sys_stat(arg1 as *const u8, arg2 as usize) as u64,
        LINUX_SYS_FSTAT => 0, // Success
        LINUX_SYS_POLL => 1, // Ready
        LINUX_SYS_LSEEK => sys_lseek(arg1 as usize, arg2 as i64, arg3 as i32) as u64,
        LINUX_SYS_MMAP => sys_mmap(arg1, arg2, arg3 as u32, arg4 as u32) as u64,
        LINUX_SYS_MUNMAP => sys_munmap(arg1, arg2) as u64,
        LINUX_SYS_BRK => sys_brk(arg1),
        LINUX_SYS_RT_SIGACTION => 0, // Signal action set OK
        LINUX_SYS_RT_SIGPROCMASK => 0, // Signal mask set OK
        LINUX_SYS_IOCTL => sys_ioctl(arg1 as usize, arg2, arg3) as u64,
        LINUX_SYS_PIPE => sys_pipe2(arg1 as *mut [i32; 2], 0) as u64,
        LINUX_SYS_SCHED_YIELD => sys_yield() as u64,
        LINUX_SYS_DUP => sys_dup(arg1 as usize) as u64,
        LINUX_SYS_DUP2 => sys_dup2(arg1 as usize, arg2 as usize) as u64,
        LINUX_SYS_NANOSLEEP => sys_nanosleep(arg1 as *const LinuxTimeSpec) as u64,
        LINUX_SYS_GETPID => sys_getpid() as u64,
        LINUX_SYS_SOCKET => sys_socket(arg1 as i32, arg2 as i32, arg3 as i32) as u64,
        LINUX_SYS_CONNECT => sys_connect(arg1 as usize, arg2 as *const u8, arg3 as usize) as u64,
        LINUX_SYS_ACCEPT => 0,
        LINUX_SYS_SENDTO => sys_sendto(arg1 as usize, arg2 as *const u8, arg3 as usize, arg4 as u32, arg5 as *const u8) as u64,
        LINUX_SYS_RECVFROM => sys_recvfrom(arg1 as usize, arg2 as *mut u8, arg3 as usize) as u64,
        LINUX_SYS_BIND => sys_bind(arg1 as usize, arg2 as *const u8, arg3 as usize) as u64,
        LINUX_SYS_LISTEN => 0,
        LINUX_SYS_CLONE => sys_clone(arg1, arg2) as u64,
        LINUX_SYS_FORK => sys_clone(0, 0) as u64,
        LINUX_SYS_EXECVE => sys_execve(arg1 as *const u8, arg2 as *const *const u8, arg3 as *const *const u8) as u64,
        LINUX_SYS_EXIT => sys_exit(arg1 as i32),
        LINUX_SYS_WAIT4 => sys_wait4(arg1 as isize, arg2 as *mut i32, arg3 as i32) as u64,
        LINUX_SYS_UNAME => sys_uname(arg1 as *mut LinuxUtsName) as u64,
        LINUX_SYS_FCNTL => sys_fcntl(arg1 as usize, arg2 as usize, arg3) as u64,
        LINUX_SYS_GETCWD => sys_getcwd(arg1 as *mut u8, arg2 as usize) as u64,
        LINUX_SYS_CHDIR => sys_chdir(arg1 as *const u8, arg2 as usize) as u64,
        LINUX_SYS_READLINK => sys_readlink(arg1 as *const u8, arg2 as *mut u8, arg3 as usize) as u64,
        LINUX_SYS_GETPPID => sys_getppid() as u64,
        LINUX_SYS_GETDENTS64 => sys_getdents64(arg1 as usize, arg2 as *mut u8, arg3 as usize) as u64,
        LINUX_SYS_EXIT_GROUP => sys_exit(arg1 as i32),
        LINUX_SYS_OPENAT => sys_openat(arg1 as i32, arg2 as *const u8, arg3 as u32) as u64,
        LINUX_SYS_MKDIRAT => 0,
        LINUX_SYS_FSTATAT => sys_fstatat(arg1 as i32, arg2 as *const u8, arg3 as *mut LinuxStat, arg4 as u32) as u64,
        LINUX_SYS_DUP3 => sys_dup3(arg1 as usize, arg2 as usize, arg3 as i32) as u64,
        LINUX_SYS_PIPE2 => sys_pipe2(arg1 as *mut [i32; 2], arg2 as i32) as u64,

        // Win32 User-Mode Subsystem syscalls
        WIN32_SYS_GETSTDHANDLE => {
            crate::subsystems::nt::win32::sys_win32_get_std_handle(arg1 as i32)
        }
        WIN32_SYS_WRITECONSOLE | WIN32_SYS_WRITEFILE => {
            crate::subsystems::nt::win32::sys_win32_write_file(
                arg1,
                arg2 as *const u8,
                arg3 as u32,
                arg4 as *mut u32,
            ) as u64
        }
        WIN32_SYS_READFILE => {
            crate::subsystems::nt::win32::sys_win32_read_file(
                arg1,
                arg2 as *mut u8,
                arg3 as u32,
                arg4 as *mut u32,
            ) as u64
        }
        WIN32_SYS_EXITPROCESS => {
            crate::subsystems::nt::win32::sys_win32_exit_process(arg1 as u32);
        }
        WIN32_SYS_GETPROCESSHEAP => {
            crate::subsystems::nt::win32::sys_win32_get_process_heap()
        }
        WIN32_SYS_HEAPALLOC | WIN32_SYS_VIRTUALALLOC => {
            crate::subsystems::nt::win32::sys_win32_heap_alloc(arg1, arg2 as u32, arg3 as usize)
        }
        WIN32_SYS_HEAPFREE | WIN32_SYS_VIRTUALFREE => {
            crate::subsystems::nt::win32::sys_win32_heap_free(arg1, arg2 as u32, arg3) as u64
        }
        WIN32_SYS_GETSYSTEMINFO => {
            crate::subsystems::nt::win32::sys_win32_get_system_info(
                arg1 as *mut crate::subsystems::nt::win32::SystemInfo,
            );
            1
        }
        WIN32_SYS_GETCURRENTPID => {
            crate::subsystems::nt::win32::sys_win32_get_current_pid() as u64
        }
        WIN32_SYS_GETCURRENTTID => {
            crate::subsystems::nt::win32::sys_win32_get_current_tid() as u64
        }
        WIN32_SYS_SLEEP => {
            crate::subsystems::nt::win32::sys_win32_sleep(arg1 as u32);
            0
        }
        WIN32_SYS_GETCOMMANDLINE => {
            crate::subsystems::nt::win32::sys_win32_get_command_line()
        }
        WIN32_SYS_GETMODULEHANDLE => {
            0x0040_0000 // Image Base
        }
        WIN32_SYS_CREATEPROCESS => {
            crate::subsystems::nt::win32::sys_win32_create_process(
                arg1 as *const u8,
                arg2 as *const u8,
                arg3 as *mut crate::subsystems::nt::win32::ProcessInformation,
            ) as u64
        }
        WIN32_SYS_WAITFORSINGLEOBJECT => {
            crate::subsystems::nt::win32::sys_win32_wait_for_single_object(arg1, arg2 as u32) as u64
        }
        WIN32_SYS_GETEXITCODEPROCESS => {
            crate::subsystems::nt::win32::sys_win32_get_exit_code_process(arg1, arg2 as *mut u32) as u64
        }
        WIN32_SYS_VIRTUALPROTECT => {
            crate::subsystems::nt::win32::sys_win32_virtual_protect(
                arg1,
                arg2 as usize,
                arg3 as u32,
                arg4 as *mut u32,
            ) as u64
        }
        WIN32_SYS_CREATEPIPE => {
            crate::subsystems::nt::win32::sys_win32_create_pipe(
                arg1 as *mut u64,
                arg2 as *mut u64,
                arg3 as *const u8,
                arg4 as u32,
            ) as u64
        }
        WIN32_SYS_SETSTDHANDLE => {
            crate::subsystems::nt::win32::sys_win32_set_std_handle(arg1 as i32, arg2) as u64
        }
        WIN32_SYS_CREATEFILE => {
            crate::subsystems::nt::win32::sys_win32_create_file(
                arg1 as *const u8,
                arg2 as u32,
                arg3 as u32,
                arg4 as *const u8,
                arg5 as u32,
            )
        }
        WIN32_SYS_CLOSEHANDLE => {
            crate::subsystems::nt::win32::sys_win32_close_handle(arg1) as u64
        }
        WIN32_SYS_FINDFIRSTFILE => {
            crate::subsystems::nt::win32::sys_win32_find_first_file(
                arg1 as *const u8,
                arg2 as *mut crate::subsystems::nt::win32::Win32FindDataA,
            )
        }
        WIN32_SYS_FINDNEXTFILE => {
            crate::subsystems::nt::win32::sys_win32_find_next_file(
                arg1,
                arg2 as *mut crate::subsystems::nt::win32::Win32FindDataA,
            ) as u64
        }
        WIN32_SYS_FINDCLOSE => {
            crate::subsystems::nt::win32::sys_win32_find_close(arg1) as u64
        }
        WIN32_SYS_GETENVIRONMENTVARIABLE => {
            crate::subsystems::nt::win32::sys_win32_get_environment_variable(
                arg1 as *const u8,
                arg2 as *mut u8,
                arg3 as u32,
            ) as u64
        }
        WIN32_SYS_SETENVIRONMENTVARIABLE => {
            crate::subsystems::nt::win32::sys_win32_set_environment_variable(
                arg1 as *const u8,
                arg2 as *const u8,
            ) as u64
        }
        WIN32_SYS_REGOPENKEYEX => {
            crate::subsystems::nt::win32::sys_win32_reg_open_key_ex(
                arg1,
                arg2 as *const u8,
                arg3 as u32,
                arg4 as u32,
                arg5 as *mut u64,
            ) as u64
        }
        WIN32_SYS_REGQUERYVALUEEX => {
            crate::subsystems::nt::win32::sys_win32_reg_query_value_ex(
                arg1,
                arg2 as *const u8,
                arg3 as *mut u32,
                arg4 as *mut u32,
                arg5 as *mut u8,
                arg6 as *mut u32,
            ) as u64
        }
        WIN32_SYS_REGCLOSEKEY => {
            crate::subsystems::nt::win32::sys_win32_reg_close_key(arg1) as u64
        }

        _ => {
            lunix_println!("[SYSCALL] Unimplemented syscall number: {}", num);
            usize::MAX as u64 // -1 (ENOSYS)
        }
    };
    crate::lunix_serial_println!("  [SYSCALL_DISPATCH_RET] num=0x{:X} -> 0x{:X}", num, ret);
    ret
}

pub fn sys_exit(code: i32) -> ! {
    let pid = crate::task::scheduler::current_pid();
    crate::task::scheduler::set_process_exit_code(pid, code);
    lunix_println!("  [SYSCALL] Process (PID {}) exited with status code: {}", pid, code);
    crate::drivers::keyboard::print_prompt();
    crate::task::scheduler::exit_current_thread();
}

pub fn sys_getppid() -> isize {
    if let Some(proc_arc) = crate::task::scheduler::get_current_process() {
        proc_arc.lock().ppid as isize
    } else {
        1
    }
}

pub fn sys_write(fd: usize, buf: *const u8, len: usize) -> isize {
    crate::lunix_serial_println!("  [SYS_WRITE] fd={}, len={}", fd, len);
    if buf.is_null() || len == 0 {
        return 0;
    }

    let target_opt = if let Some(proc_arc) = crate::task::scheduler::get_current_process() {
        let proc = proc_arc.lock();
        if let Some(desc_arc) = proc.get_fd(fd) {
            let desc = desc_arc.lock();
            Some((desc.target.clone(), desc.flags))
        } else {
            None
        }
    } else {
        None
    };

    if let Some((target, flags)) = target_opt {
        match target {
            FdTarget::Stdout | FdTarget::Stderr => {
                let slice = unsafe { slice::from_raw_parts(buf, len) };
                if let Ok(s) = core::str::from_utf8(slice) {
                    lunix_print!("{}", s);
                } else {
                    for &byte in slice {
                        lunix_print!("{}", byte as char);
                    }
                }
                return len as isize;
            }
            FdTarget::PipeWrite(pipe) => {
                let non_blocking = (flags & 0x800) != 0;
                let slice = unsafe { slice::from_raw_parts(buf, len) };
                for _ in 0..100 {
                    let write_res = {
                        pipe.lock().write(slice, non_blocking)
                    };
                    match write_res {
                        Ok(n) => return n as isize,
                        Err(crate::task::pipe::PipeError::WouldBlock)
                        | Err(crate::task::pipe::PipeError::BufferFull) => {
                            if non_blocking {
                                return -11; // -EAGAIN
                            }
                            crate::task::scheduler::sleep_ms(2);
                        }
                        Err(crate::task::pipe::PipeError::BrokenPipe) => return -32, // -EPIPE
                    }
                }
                return -32;
            }
            FdTarget::File { .. } => {
                if let Some(proc_arc) = crate::task::scheduler::get_current_process() {
                    let proc = proc_arc.lock();
                    if let Some(desc_arc) = proc.get_fd(fd) {
                        let mut desc = desc_arc.lock();
                        if let FdTarget::File { ref mut offset, ref mut size, ref mut data, .. } = desc.target {
                            let slice = unsafe { slice::from_raw_parts(buf, len) };
                            let needed = *offset + len;
                            if needed > data.len() {
                                data.resize(needed, 0);
                            }
                            data[*offset..*offset + len].copy_from_slice(slice);
                            *offset += len;
                            if *offset > *size {
                                *size = *offset;
                            }
                            return len as isize;
                        }
                    }
                }
                return -9;
            }
            FdTarget::VfsHandle(ref handle) => {
                let slice = unsafe { slice::from_raw_parts(buf, len) };
                let mut h = handle.lock();
                match h.write(slice) {
                    Ok(n) => return n as isize,
                    Err(_) => return -1,
                }
            }
            FdTarget::Socket(sid) => {
                return sys_sendto(sid, buf, len, 0, core::ptr::null());
            }
            _ => return -9, // EBADF
        }
    }

    // Fallback for FDs 1 (stdout) and 2 (stderr)
    if fd == 1 || fd == 2 {
        let slice = unsafe { slice::from_raw_parts(buf, len) };
        if let Ok(s) = core::str::from_utf8(slice) {
            lunix_print!("{}", s);
        } else {
            for &byte in slice {
                lunix_print!("{}", byte as char);
            }
        }
        return len as isize;
    }

    -9 // EBADF
}

pub fn sys_read(fd: usize, buf: *mut u8, len: usize) -> isize {
    crate::lunix_serial_println!("  [SYS_READ] fd={}, len={}", fd, len);
    if buf.is_null() || len == 0 {
        return 0;
    }

    let target_opt = if let Some(proc_arc) = crate::task::scheduler::get_current_process() {
        let proc = proc_arc.lock();
        if let Some(desc_arc) = proc.get_fd(fd) {
            let desc = desc_arc.lock();
            Some((desc.target.clone(), desc.flags))
        } else {
            None
        }
    } else {
        None
    };

    if let Some((target, flags)) = target_opt {
        match target {
            FdTarget::Stdin => {
                let slice = unsafe { slice::from_raw_parts_mut(buf, len) };
                slice[0] = 0;
                return 1;
            }
            FdTarget::PipeRead(pipe) => {
                let non_blocking = (flags & 0x800) != 0;
                let slice = unsafe { slice::from_raw_parts_mut(buf, len) };
                for _ in 0..100 {
                    let read_res = {
                        pipe.lock().read(slice, non_blocking)
                    };
                    match read_res {
                        Ok(n) => return n as isize,
                        Err(crate::task::pipe::PipeError::WouldBlock) => {
                            if non_blocking {
                                return -11; // -EAGAIN
                            }
                            crate::task::scheduler::sleep_ms(2);
                        }
                        Err(crate::task::pipe::PipeError::BrokenPipe) => return 0, // EOF
                        Err(crate::task::pipe::PipeError::BufferFull) => return 0,
                    }
                }
                return 0;
            }
            FdTarget::File { .. } => {
                if let Some(proc_arc) = crate::task::scheduler::get_current_process() {
                    let proc = proc_arc.lock();
                    if let Some(desc_arc) = proc.get_fd(fd) {
                        let mut desc = desc_arc.lock();
                        if let FdTarget::File { ref mut offset, size, ref data, .. } = desc.target {
                            if *offset >= size {
                                return 0; // EOF
                            }
                            let avail = size - *offset;
                            let to_read = len.min(avail);
                            let slice = unsafe { slice::from_raw_parts_mut(buf, len) };
                            slice[..to_read].copy_from_slice(&data[*offset..*offset + to_read]);
                            *offset += to_read;
                            return to_read as isize;
                        }
                    }
                }
                return -9;
            }
            FdTarget::VfsHandle(ref handle) => {
                let slice = unsafe { slice::from_raw_parts_mut(buf, len) };
                let mut h = handle.lock();
                match h.read(slice) {
                    Ok(n) => return n as isize,
                    Err(_) => return -1,
                }
            }
            FdTarget::Socket(sid) => {
                return sys_recvfrom(sid, buf, len);
            }
            FdTarget::Directory { .. } => {
                return -21; // -EISDIR
            }
            _ => return -9, // EBADF
        }
    }

    if fd == 0 {
        let slice = unsafe { slice::from_raw_parts_mut(buf, len) };
        slice[0] = 0;
        return 1;
    }

    -9 // EBADF
}

pub fn sys_lseek(fd: usize, offset: i64, whence: i32) -> isize {
    crate::lunix_serial_println!("  [SYS_LSEEK] fd={}, offset={}, whence={}", fd, offset, whence);
    let seek_from = match whence {
        0 => {
            if offset < 0 {
                return -22; // -EINVAL
            }
            crate::fs::file::SeekFrom::Start(offset as u64)
        }
        1 => crate::fs::file::SeekFrom::Current(offset),
        2 => crate::fs::file::SeekFrom::End(offset),
        _ => return -22, // -EINVAL
    };

    if let Some(proc_arc) = crate::task::scheduler::get_current_process() {
        let proc = proc_arc.lock();
        if let Some(desc_arc) = proc.get_fd(fd) {
            let mut desc = desc_arc.lock();
            match desc.target {
                FdTarget::File { offset: ref mut f_off, size, .. } => {
                    let new_off: i64 = match seek_from {
                        crate::fs::file::SeekFrom::Start(s) => s as i64,
                        crate::fs::file::SeekFrom::Current(c) => *f_off as i64 + c,
                        crate::fs::file::SeekFrom::End(e) => size as i64 + e,
                    };
                    if new_off < 0 {
                        return -22; // -EINVAL
                    }
                    *f_off = new_off as usize;
                    return new_off as isize;
                }
                FdTarget::VfsHandle(ref handle) => {
                    let mut h = handle.lock();
                    match h.seek(seek_from) {
                        Ok(pos) => return pos as isize,
                        Err(_) => return -22, // -EINVAL
                    }
                }
                _ => return -29, // -ESPIPE
            }
        }
    }
    -9 // -EBADF
}

pub fn sys_open(path_ptr: *const u8, path_len: usize, flags: u32) -> isize {
    if path_ptr.is_null() || path_len == 0 {
        return -1;
    }

    let slice = unsafe { slice::from_raw_parts(path_ptr, path_len) };
    if let Ok(path) = core::str::from_utf8(slice) {
        if let Some(proc_arc) = crate::task::scheduler::get_current_process() {
            let mut proc = proc_arc.lock();
            // Check if directory
            if let Ok(entries) = crate::fs::vfs::read_dir(path) {
                if let Some(fd) = proc.allocate_fd(crate::task::process::FileDescriptor {
                    target: FdTarget::Directory {
                        path: alloc::string::String::from(path),
                        entries,
                        current_idx: 0,
                    },
                    flags,
                }) {
                    return fd as isize;
                }
            } else if let Ok(handle) = crate::fs::vfs::open(path) {
                if let Some(fd) = proc.allocate_fd(crate::task::process::FileDescriptor {
                    target: FdTarget::VfsHandle(alloc::sync::Arc::new(Mutex::new(handle))),
                    flags,
                }) {
                    return fd as isize;
                }
            } else if let Ok(data) = crate::fs::vfs::read_to_vec(path) {
                let size = data.len();
                if let Some(fd) = proc.allocate_fd(crate::task::process::FileDescriptor {
                    target: FdTarget::File {
                        path: alloc::string::String::from(path),
                        offset: 0,
                        size,
                        data,
                    },
                    flags,
                }) {
                    return fd as isize;
                }
            }
        }
    }

    -2 // -ENOENT
}

pub fn sys_close(fd: usize) -> isize {
    crate::lunix_serial_println!("  [SYS_CLOSE] fd={}", fd);
    if let Some(proc_arc) = crate::task::scheduler::get_current_process() {
        let mut proc = proc_arc.lock();
        if proc.close_fd(fd) {
            0
        } else {
            -9 // -EBADF
        }
    } else {
        0
    }
}

pub fn sys_stat(path_ptr: *const u8, path_len: usize) -> isize {
    if path_ptr.is_null() || path_len == 0 {
        return -1;
    }

    let slice = unsafe { slice::from_raw_parts(path_ptr, path_len) };
    if let Ok(path) = core::str::from_utf8(slice) {
        if let Ok(_inode) = crate::fs::vfs::stat(path) {
            return 0; // Success
        }
    }

    -2 // -ENOENT
}

pub fn sys_mmap(addr: u64, length: u64, _prot: u32, _flags: u32) -> u64 {
    let pages = (length + 4095) / 4096;
    let target_addr = if addr != 0 {
        addr
    } else {
        // Allocate in user heap range
        let mut brk_lock = USER_BRK.lock();
        let alloc_addr = *brk_lock;
        *brk_lock += pages * 4096;
        alloc_addr
    };

    for p in 0..pages {
        if let Some(frame) = crate::mm::pmm::alloc_frame() {
            let page_vaddr = x86_64::VirtAddr::new(target_addr + (p * 4096));
            let flags = x86_64::structures::paging::PageTableFlags::PRESENT
                | x86_64::structures::paging::PageTableFlags::WRITABLE
                | x86_64::structures::paging::PageTableFlags::USER_ACCESSIBLE;
            let _ = crate::mm::vmm::map_page(page_vaddr, frame, flags);
            unsafe {
                core::ptr::write_bytes(frame.as_u64() as *mut u8, 0, 4096);
            }
        }
    }

    target_addr
}

pub fn sys_munmap(_addr: u64, _length: u64) -> isize {
    0
}

pub fn sys_brk(brk: u64) -> u64 {
    let mut current_brk = USER_BRK.lock();
    if brk == 0 {
        return *current_brk;
    }

    if brk > *current_brk {
        let diff = brk - *current_brk;
        let pages = (diff + 4095) / 4096;
        for p in 0..pages {
            if let Some(frame) = crate::mm::pmm::alloc_frame() {
                let page_vaddr = x86_64::VirtAddr::new(*current_brk + (p * 4096));
                let flags = x86_64::structures::paging::PageTableFlags::PRESENT
                    | x86_64::structures::paging::PageTableFlags::WRITABLE
                    | x86_64::structures::paging::PageTableFlags::USER_ACCESSIBLE;
                let _ = crate::mm::vmm::map_page(page_vaddr, frame, flags);
            }
        }
        *current_brk = brk;
    }

    *current_brk
}

pub fn sys_yield() -> isize {
    crate::task::scheduler::yield_now();
    0
}

pub fn sys_sleep(ms: u64) -> isize {
    crate::task::scheduler::sleep_ms(ms);
    0
}

pub fn sys_nanosleep(req: *const LinuxTimeSpec) -> isize {
    if req.is_null() {
        return -1;
    }

    let spec = unsafe { &*req };
    let ms = (spec.tv_sec as u64 * 1000) + (spec.tv_nsec as u64 / 1_000_000);
    crate::task::scheduler::sleep_ms(ms.max(1));
    0
}

pub fn sys_getpid() -> isize {
    crate::task::scheduler::current_tid() as isize
}

pub fn sys_uname(buf: *mut LinuxUtsName) -> isize {
    if buf.is_null() {
        return -1;
    }

    unsafe {
        let uts = &mut *buf;
        copy_cstr(&mut uts.sysname, b"Lunix\0");
        copy_cstr(&mut uts.nodename, b"lunix-os\0");
        copy_cstr(&mut uts.release, b"0.1.0-hybrid\0");
        copy_cstr(&mut uts.version, b"#1 SMP PREEMPT 2026-09-11\0");
        copy_cstr(&mut uts.machine, b"x86_64\0");
        copy_cstr(&mut uts.domainname, b"(none)\0");
    }

    0
}

fn copy_cstr(dest: &mut [u8; 65], src: &[u8]) {
    for (i, &b) in src.iter().enumerate() {
        if i < 64 {
            dest[i] = b;
        }
    }
    dest[src.len().min(64)] = 0;
}

pub fn sys_ioctl(fd: usize, request: u64, arg: u64) -> isize {
    const TIOCGWINSZ: u64 = 0x5413;
    const TCGETS: u64 = 0x5401;
    const TCSETS: u64 = 0x5402;
    const TCSETSW: u64 = 0x5403;
    const TCSETSF: u64 = 0x5404;
    const FIONBIO: u64 = 0x5421;

    match request {
        TIOCGWINSZ => {
            if arg == 0 {
                return -14; // -EFAULT
            }
            #[repr(C)]
            struct WinSize {
                ws_row: u16,
                ws_col: u16,
                ws_xpixel: u16,
                ws_ypixel: u16,
            }
            unsafe {
                let ws = &mut *(arg as *mut WinSize);
                ws.ws_row = 25;
                ws.ws_col = 80;
                ws.ws_xpixel = 640;
                ws.ws_ypixel = 400;
            }
            0
        }
        TCGETS | TCSETS | TCSETSW | TCSETSF => 0,
        FIONBIO => {
            if arg != 0 {
                let val = unsafe { *(arg as *const i32) };
                if let Some(proc_arc) = crate::task::scheduler::get_current_process() {
                    let proc = proc_arc.lock();
                    if let Some(desc_arc) = proc.get_fd(fd) {
                        let mut desc = desc_arc.lock();
                        if val != 0 {
                            desc.flags |= 0x800; // O_NONBLOCK
                        } else {
                            desc.flags &= !0x800;
                        }
                    }
                }
            }
            0
        }
        _ => 0,
    }
}

pub fn sys_fcntl(fd: usize, cmd: usize, arg: u64) -> isize {
    const F_DUPFD: usize = 0;
    const F_GETFD: usize = 1;
    const F_SETFD: usize = 2;
    const F_GETFL: usize = 3;
    const F_SETFL: usize = 4;
    const F_DUPFD_CLOEXEC: usize = 1030;

    if let Some(proc_arc) = crate::task::scheduler::get_current_process() {
        let mut proc = proc_arc.lock();
        match cmd {
            F_DUPFD | F_DUPFD_CLOEXEC => {
                if let Some(newfd) = proc.dup_lowest_fd(fd, arg as usize) {
                    newfd as isize
                } else {
                    -9 // -EBADF
                }
            }
            F_GETFD => {
                if let Some(desc_arc) = proc.get_fd(fd) {
                    let desc = desc_arc.lock();
                    (desc.flags & 1) as isize // FD_CLOEXEC
                } else {
                    -9
                }
            }
            F_SETFD => {
                if let Some(desc_arc) = proc.get_fd(fd) {
                    let mut desc = desc_arc.lock();
                    desc.flags = (desc.flags & !1) | (arg as u32 & 1);
                    0
                } else {
                    -9
                }
            }
            F_GETFL => {
                if let Some(desc_arc) = proc.get_fd(fd) {
                    let desc = desc_arc.lock();
                    desc.flags as isize
                } else {
                    -9
                }
            }
            F_SETFL => {
                if let Some(desc_arc) = proc.get_fd(fd) {
                    let mut desc = desc_arc.lock();
                    desc.flags = arg as u32;
                    0
                } else {
                    -9
                }
            }
            _ => 0,
        }
    } else {
        -1
    }
}

pub fn sys_getcwd(buf: *mut u8, size: usize) -> isize {
    if buf.is_null() || size < 2 {
        return -1;
    }

    let cwd = b"/\0";
    let len = cwd.len().min(size);
    unsafe {
        core::ptr::copy_nonoverlapping(cwd.as_ptr(), buf, len);
    }
    buf as isize
}

pub fn sys_chdir(_path_ptr: *const u8, _path_len: usize) -> isize {
    0
}

pub fn sys_socket(_domain: i32, _sock_type: i32, _protocol: i32) -> isize {
    let mut lock = crate::net::NET_STACK.lock();
    if let Some(ref mut stack) = *lock {
        let sid = stack.next_socket_id;
        stack.next_socket_id += 1;
        let sock = crate::net::Socket {
            id: sid,
            domain: _domain,
            socket_type: _sock_type,
            protocol: _protocol,
            local_ip: crate::net::DEFAULT_IP,
            local_port: 0,
            remote_ip: [0, 0, 0, 0],
            remote_port: 0,
            state: crate::net::SocketState::Closed,
            recv_queue: alloc::vec::Vec::new(),
        };
        stack.sockets.insert(sid, sock);
        sid as isize
    } else {
        -1
    }
}

pub fn sys_bind(sockfd: usize, _addr: *const u8, _addrlen: usize) -> isize {
    let mut lock = crate::net::NET_STACK.lock();
    if let Some(ref mut stack) = *lock {
        if stack.sockets.contains_key(&sockfd) {
            0
        } else {
            -1
        }
    } else {
        -1
    }
}

pub fn sys_connect(sockfd: usize, _addr: *const u8, _addrlen: usize) -> isize {
    let mut lock = crate::net::NET_STACK.lock();
    if let Some(ref mut stack) = *lock {
        if let Some(sock) = stack.sockets.get_mut(&sockfd) {
            sock.state = crate::net::SocketState::Established;
            0
        } else {
            -1
        }
    } else {
        -1
    }
}

pub fn sys_sendto(_sockfd: usize, buf: *const u8, len: usize, _flags: u32, _dest_addr: *const u8) -> isize {
    if buf.is_null() || len == 0 {
        return 0;
    }
    let slice = unsafe { core::slice::from_raw_parts(buf, len) };
    let _ = crate::net::send_ipv4_packet(crate::net::DEFAULT_GATEWAY, crate::net::IP_PROTO_UDP, slice);
    len as isize
}

pub fn sys_recvfrom(_sockfd: usize, _buf: *mut u8, _len: usize) -> isize {
    0
}

#[repr(C, packed)]
pub struct LinuxDirent64 {
    pub d_ino: u64,
    pub d_off: i64,
    pub d_reclen: u16,
    pub d_type: u8,
}

pub fn sys_getdents64(fd: usize, dirp: *mut u8, count: usize) -> isize {
    if dirp.is_null() || count < 32 {
        return 0;
    }

    let (entries, start_idx) = if let Some(proc_arc) = crate::task::scheduler::get_current_process() {
        let proc = proc_arc.lock();
        if let Some(desc_arc) = proc.get_fd(fd) {
            let desc = desc_arc.lock();
            if let FdTarget::Directory { ref entries, current_idx, .. } = desc.target {
                (entries.clone(), current_idx)
            } else {
                let cwd = crate::drivers::keyboard::get_cwd();
                (crate::fs::vfs::read_dir(&cwd).unwrap_or_default(), 0)
            }
        } else {
            let cwd = crate::drivers::keyboard::get_cwd();
            (crate::fs::vfs::read_dir(&cwd).unwrap_or_default(), 0)
        }
    } else {
        let cwd = crate::drivers::keyboard::get_cwd();
        (crate::fs::vfs::read_dir(&cwd).unwrap_or_default(), 0)
    };

    if start_idx >= entries.len() {
        return 0; // EOF
    }

    let mut written = 0;
    let mut idx = start_idx;

    while idx < entries.len() {
        let entry = &entries[idx];
        let name_bytes = entry.name.as_bytes();
        let name_len = name_bytes.len();
        // Calculate record length: 19 bytes header + name_len + 1 null + padding to 8-byte alignment
        let raw_len = 19 + name_len + 1;
        let reclen = ((raw_len + 7) & !7) as u16;

        if written + (reclen as usize) > count {
            break;
        }

        let d_type = match entry.node_type {
            crate::fs::inode::INodeType::Directory => 4, // DT_DIR
            crate::fs::inode::INodeType::File => 8,      // DT_REG
            crate::fs::inode::INodeType::BlockDevice => 6, // DT_BLK
            crate::fs::inode::INodeType::CharDevice => 2,  // DT_CHR
            crate::fs::inode::INodeType::SymLink => 10,   // DT_LNK
        };

        unsafe {
            let out_ptr = dirp.add(written);
            core::ptr::write_bytes(out_ptr, 0, reclen as usize);

            // d_ino (8 bytes)
            *(out_ptr as *mut u64) = (idx + 1) as u64;
            // d_off (8 bytes)
            *(out_ptr.add(8) as *mut i64) = (idx + 1) as i64;
            // d_reclen (2 bytes)
            *(out_ptr.add(16) as *mut u16) = reclen;
            // d_type (1 byte)
            *out_ptr.add(18) = d_type;
            // d_name (name_len + 1 null bytes)
            core::ptr::copy_nonoverlapping(name_bytes.as_ptr(), out_ptr.add(19), name_len);
            *out_ptr.add(19 + name_len) = 0;
        }

        written += reclen as usize;
        idx += 1;
    }

    if let Some(proc_arc) = crate::task::scheduler::get_current_process() {
        let proc = proc_arc.lock();
        if let Some(desc_arc) = proc.get_fd(fd) {
            let mut desc = desc_arc.lock();
            if let FdTarget::Directory { ref mut current_idx, .. } = desc.target {
                *current_idx = idx;
            }
        }
    }

    written as isize
}

pub fn sys_execve(filename_ptr: *const u8, _argv_ptr: *const *const u8, _envp_ptr: *const *const u8) -> isize {
    if filename_ptr.is_null() {
        return -1;
    }

    let mut len = 0;
    unsafe {
        while *filename_ptr.add(len) != 0 && len < 256 {
            len += 1;
        }
    }

    let slice = unsafe { core::slice::from_raw_parts(filename_ptr, len) };
    if let Ok(path) = core::str::from_utf8(slice) {
        match crate::task::elf::exec_elf(path) {
            Ok(_) => {
                crate::task::scheduler::exit_current_thread();
            }
            Err(_) => -1,
        }
    } else {
        -1
    }
}

static CLONE_USER_STACK: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
static CLONE_USER_ENTRY: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

fn clone_runner_trampoline() {
    let stack = CLONE_USER_STACK.load(core::sync::atomic::Ordering::SeqCst);
    let entry = CLONE_USER_ENTRY.load(core::sync::atomic::Ordering::SeqCst);
    unsafe {
        crate::task::user::enter_user_mode_with_rax(entry, stack, 0);
    }
}

pub fn sys_clone(_flags: u64, stack: u64) -> isize {
    let current_pid = crate::task::scheduler::current_pid();
    let child_pid = crate::task::scheduler::allocate_pid();

    if let Some(parent_proc) = crate::task::scheduler::get_process(current_pid) {
        let child_proc = parent_proc.lock().clone_process(child_pid);
        crate::task::scheduler::register_process(child_proc);
        parent_proc.lock().children.push(child_pid);
    }

    let return_rip = crate::arch::x86_64::syscall::LAST_USER_RIP.load(core::sync::atomic::Ordering::SeqCst);
    let entry = if return_rip != 0 { return_rip } else { 0x400078 };
    CLONE_USER_ENTRY.store(entry, core::sync::atomic::Ordering::SeqCst);

    let user_stack = if stack != 0 {
        stack
    } else {
        let last_rsp = crate::arch::x86_64::syscall::LAST_USER_RSP.load(core::sync::atomic::Ordering::SeqCst);
        if last_rsp != 0 { last_rsp } else { crate::task::elf::USER_STACK_BASE + (crate::task::elf::USER_STACK_SIZE as u64) - 512 }
    };

    CLONE_USER_STACK.store(user_stack, core::sync::atomic::Ordering::SeqCst);
    let _tid = crate::task::scheduler::spawn_with_pid("user_fork", child_pid, clone_runner_trampoline, 6);
    child_pid as isize
}

pub fn sys_wait4(pid: isize, status_ptr: *mut i32, _options: i32) -> isize {
    let parent_pid = crate::task::scheduler::current_pid();

    for _ in 0..100 {
        if let Some((child_pid, exit_code)) = crate::task::scheduler::reap_child_process(parent_pid, pid) {
            if !status_ptr.is_null() {
                unsafe {
                    *status_ptr = (exit_code & 0xFF) << 8; // WEXITSTATUS format
                }
            }
            return child_pid as isize;
        }
        crate::task::scheduler::sleep_ms(10);
    }

    if !status_ptr.is_null() {
        unsafe {
            *status_ptr = 0;
        }
    }
    if pid > 0 { pid } else { 1 }
}

pub fn sys_openat(dfd: i32, filename_ptr: *const u8, flags: u32) -> isize {
    if filename_ptr.is_null() {
        return -1;
    }
    let mut len = 0;
    unsafe {
        while *filename_ptr.add(len) != 0 && len < 256 {
            len += 1;
        }
    }
    let slice = unsafe { core::slice::from_raw_parts(filename_ptr, len) };
    let path = match core::str::from_utf8(slice) {
        Ok(p) => p,
        Err(_) => return -1,
    };

    let resolved_path = if path.starts_with('/') || dfd == -100 {
        alloc::string::String::from(path)
    } else {
        let mut full = alloc::string::String::from("/");
        full.push_str(path);
        full
    };

    if let Some(proc_arc) = crate::task::scheduler::get_current_process() {
        let mut proc = proc_arc.lock();
        if let Ok(entries) = crate::fs::vfs::read_dir(&resolved_path) {
            if let Some(fd) = proc.allocate_fd(crate::task::process::FileDescriptor {
                target: FdTarget::Directory {
                    path: resolved_path,
                    entries,
                    current_idx: 0,
                },
                flags,
            }) {
                return fd as isize;
            }
        } else if let Ok(handle) = crate::fs::vfs::open(&resolved_path) {
            if let Some(fd) = proc.allocate_fd(crate::task::process::FileDescriptor {
                target: FdTarget::VfsHandle(alloc::sync::Arc::new(Mutex::new(handle))),
                flags,
            }) {
                return fd as isize;
            }
        } else if let Ok(data) = crate::fs::vfs::read_to_vec(&resolved_path) {
            let size = data.len();
            if let Some(fd) = proc.allocate_fd(crate::task::process::FileDescriptor {
                target: FdTarget::File {
                    path: resolved_path,
                    offset: 0,
                    size,
                    data,
                },
                flags,
            }) {
                return fd as isize;
            }
        }
    }

    -2 // -ENOENT
}

pub fn sys_fstatat(_dfd: i32, filename_ptr: *const u8, statbuf: *mut LinuxStat, _flags: u32) -> isize {
    if filename_ptr.is_null() || statbuf.is_null() {
        return -1;
    }
    let mut len = 0;
    unsafe {
        while *filename_ptr.add(len) != 0 && len < 256 {
            len += 1;
        }
    }
    let slice = unsafe { core::slice::from_raw_parts(filename_ptr, len) };
    if let Ok(path) = core::str::from_utf8(slice) {
        if let Ok(inode) = crate::fs::vfs::stat(path) {
            unsafe {
                let st = &mut *statbuf;
                st.st_dev = 1;
                st.st_ino = inode.id;
                st.st_nlink = 1;
                st.st_mode = if inode.node_type == crate::fs::inode::INodeType::Directory {
                    0o040755 // S_IFDIR | 0755
                } else {
                    0o100755 // S_IFREG | 0755
                };
                st.st_uid = 0;
                st.st_gid = 0;
                st.st_rdev = 0;
                st.st_size = inode.size as i64;
                st.st_blksize = 512;
                st.st_blocks = (inode.size as i64 + 511) / 512;
                st.st_atime = 1726000000;
                st.st_mtime = 1726000000;
                st.st_ctime = 1726000000;
            }
            return 0;
        }
    }
    -2 // -ENOENT
}

pub fn sys_readlink(path_ptr: *const u8, buf: *mut u8, bufsiz: usize) -> isize {
    if path_ptr.is_null() || buf.is_null() || bufsiz == 0 {
        return -1;
    }
    -1 // EINVAL (not a symlink)
}

pub fn sys_pipe2(pipefd_ptr: *mut [i32; 2], flags: i32) -> isize {
    if pipefd_ptr.is_null() {
        return -1; // EFAULT
    }
    let (reader, writer) = crate::task::pipe::create_pipe_pair();
    if let Some(proc_arc) = crate::task::scheduler::get_current_process() {
        let mut proc = proc_arc.lock();
        let read_fd = match proc.allocate_fd(crate::task::process::FileDescriptor {
            target: FdTarget::PipeRead(reader),
            flags: flags as u32,
        }) {
            Some(fd) => fd,
            None => return -24, // EMFILE
        };
        let write_fd = match proc.allocate_fd(crate::task::process::FileDescriptor {
            target: FdTarget::PipeWrite(writer),
            flags: flags as u32,
        }) {
            Some(fd) => fd,
            None => {
                proc.close_fd(read_fd);
                return -24; // EMFILE
            }
        };
        unsafe {
            (*pipefd_ptr)[0] = read_fd as i32;
            (*pipefd_ptr)[1] = write_fd as i32;
        }
        crate::lunix_serial_println!("  [SYS_PIPE2] read_fd={}, write_fd={}", read_fd, write_fd);
        0
    } else {
        -1
    }
}

pub fn sys_dup(oldfd: usize) -> isize {
    if let Some(proc_arc) = crate::task::scheduler::get_current_process() {
        let mut proc = proc_arc.lock();
        if let Some(newfd) = proc.dup_lowest_fd(oldfd, 0) {
            newfd as isize
        } else {
            -9 // -EBADF
        }
    } else {
        -1
    }
}

pub fn sys_dup2(oldfd: usize, newfd: usize) -> isize {
    if let Some(proc_arc) = crate::task::scheduler::get_current_process() {
        let mut proc = proc_arc.lock();
        if oldfd == newfd {
            if proc.get_fd(oldfd).is_some() {
                return newfd as isize;
            } else {
                return -9; // -EBADF
            }
        }
        if let Some(fd) = proc.dup_fd(oldfd, newfd) {
            fd as isize
        } else {
            -9 // -EBADF
        }
    } else {
        -1
    }
}

pub fn sys_dup3(oldfd: usize, newfd: usize, flags: i32) -> isize {
    if oldfd == newfd {
        return -22; // -EINVAL
    }
    let res = sys_dup2(oldfd, newfd);
    if res >= 0 && flags != 0 {
        if let Some(proc_arc) = crate::task::scheduler::get_current_process() {
            let proc = proc_arc.lock();
            if let Some(desc_arc) = proc.get_fd(newfd) {
                desc_arc.lock().flags = flags as u32;
            }
        }
    }
    res
}
