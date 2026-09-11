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
pub const LINUX_SYS_GETCWD: usize = 79;
pub const LINUX_SYS_CHDIR: usize = 80;
pub const LINUX_SYS_READLINK: usize = 89;
pub const LINUX_SYS_GETPPID: usize = 110;
pub const LINUX_SYS_GETDENTS64: usize = 217;
pub const LINUX_SYS_EXIT_GROUP: usize = 231;
pub const LINUX_SYS_OPENAT: usize = 257;
pub const LINUX_SYS_MKDIRAT: usize = 258;
pub const LINUX_SYS_FSTATAT: usize = 262;
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

#[no_mangle]
pub extern "C" fn syscall_dispatcher(
    num: usize,
    arg1: u64,
    arg2: u64,
    arg3: u64,
    arg4: u64,
    arg5: u64,
    _arg6: u64,
) -> u64 {
    crate::lunix_serial_println!("  [SYSCALL_DISPATCH] num=0x{:X}", num);
    match num {
        // Standard Linux syscalls
        LINUX_SYS_READ => sys_read(arg1 as usize, arg2 as *mut u8, arg3 as usize) as u64,
        LINUX_SYS_WRITE => sys_write(arg1 as usize, arg2 as *const u8, arg3 as usize) as u64,
        LINUX_SYS_OPEN => sys_open(arg1 as *const u8, arg2 as usize, arg3 as u32) as u64,
        LINUX_SYS_CLOSE => sys_close(arg1 as usize) as u64,
        LINUX_SYS_STAT => sys_stat(arg1 as *const u8, arg2 as usize) as u64,
        LINUX_SYS_FSTAT => 0, // Success
        LINUX_SYS_POLL => 1, // Ready
        LINUX_SYS_LSEEK => 0, // Success (offset 0)
        LINUX_SYS_MMAP => sys_mmap(arg1, arg2, arg3 as u32, arg4 as u32) as u64,
        LINUX_SYS_MUNMAP => sys_munmap(arg1, arg2) as u64,
        LINUX_SYS_BRK => sys_brk(arg1),
        LINUX_SYS_RT_SIGACTION => 0, // Signal action set OK
        LINUX_SYS_RT_SIGPROCMASK => 0, // Signal mask set OK
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
        LINUX_SYS_GETCWD => sys_getcwd(arg1 as *mut u8, arg2 as usize) as u64,
        LINUX_SYS_CHDIR => sys_chdir(arg1 as *const u8, arg2 as usize) as u64,
        LINUX_SYS_READLINK => sys_readlink(arg1 as *const u8, arg2 as *mut u8, arg3 as usize) as u64,
        LINUX_SYS_GETPPID => 1, // Parent is PID 1
        LINUX_SYS_GETDENTS64 => sys_getdents64(arg1 as usize, arg2 as *mut u8, arg3 as usize) as u64,
        LINUX_SYS_EXIT_GROUP => sys_exit(arg1 as i32),
        LINUX_SYS_OPENAT => sys_openat(arg1 as i32, arg2 as *const u8, arg3 as u32) as u64,
        LINUX_SYS_MKDIRAT => 0,
        LINUX_SYS_FSTATAT => 0,
        LINUX_SYS_PIPE2 => sys_pipe2(arg1 as *mut [i32; 2], arg2 as i32) as u64,

        // Win32 User-Mode Subsystem syscalls
        WIN32_SYS_GETSTDHANDLE => {
            crate::subsystems::nt::win32::sys_win32_get_std_handle(arg1 as i32)
        }
        WIN32_SYS_WRITECONSOLE | WIN32_SYS_WRITEFILE => {
            crate::subsystems::nt::win32::sys_win32_write_console(
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

        _ => {
            lunix_println!("[SYSCALL] Unimplemented syscall number: {}", num);
            usize::MAX as u64 // -1 (ENOSYS)
        }
    }
}

pub fn sys_exit(code: i32) -> ! {
    lunix_println!("  [SYSCALL] Process exited with status code: {}", code);
    crate::drivers::keyboard::print_prompt();
    crate::task::scheduler::exit_current_thread();
}

pub fn sys_write(fd: usize, buf: *const u8, len: usize) -> isize {
    if buf.is_null() || len == 0 {
        return 0;
    }

    let slice = unsafe { slice::from_raw_parts(buf, len) };

    // FDs 1 (stdout) and 2 (stderr)
    if fd == 1 || fd == 2 {
        if let Ok(s) = core::str::from_utf8(slice) {
            lunix_print!("{}", s);
            return len as isize;
        } else {
            for &byte in slice {
                lunix_print!("{}", byte as char);
            }
            return len as isize;
        }
    }

    -1 // EBADF
}

pub fn sys_read(fd: usize, buf: *mut u8, len: usize) -> isize {
    if buf.is_null() || len == 0 {
        return 0;
    }

    if fd == 0 {
        // Stdin: non-blocking or single byte
        let slice = unsafe { slice::from_raw_parts_mut(buf, len) };
        slice[0] = 0;
        return 1;
    }

    -1
}

pub fn sys_open(path_ptr: *const u8, path_len: usize, _flags: u32) -> isize {
    if path_ptr.is_null() || path_len == 0 {
        return -1;
    }

    let slice = unsafe { slice::from_raw_parts(path_ptr, path_len) };
    if let Ok(path) = core::str::from_utf8(slice) {
        if let Ok(_node) = crate::fs::vfs::open(path) {
            return 3; // Assigned FD
        }
    }

    -1
}

pub fn sys_close(_fd: usize) -> isize {
    0
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

    -1
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

pub fn sys_getdents64(_fd: usize, dirp: *mut u8, count: usize) -> isize {
    if dirp.is_null() || count < 32 {
        return 0;
    }

    let cwd = crate::drivers::keyboard::get_cwd();
    let entries = match crate::fs::vfs::read_dir(&cwd) {
        Ok(e) => e,
        Err(_) => return -1,
    };

    let mut written = 0;
    let mut offset = 1;

    for entry in entries {
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
            *(out_ptr as *mut u64) = offset as u64;
            // d_off (8 bytes)
            *(out_ptr.add(8) as *mut i64) = offset as i64;
            // d_reclen (2 bytes)
            *(out_ptr.add(16) as *mut u16) = reclen;
            // d_type (1 byte)
            *out_ptr.add(18) = d_type;
            // d_name (name_len + 1 null bytes)
            core::ptr::copy_nonoverlapping(name_bytes.as_ptr(), out_ptr.add(19), name_len);
            *out_ptr.add(19 + name_len) = 0;
        }

        written += reclen as usize;
        offset += 1;
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
            Ok(_) => 0,
            Err(_) => -1,
        }
    } else {
        -1
    }
}

static CLONE_USER_STACK: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

fn clone_runner_trampoline() {
    let stack = CLONE_USER_STACK.load(core::sync::atomic::Ordering::SeqCst);
    unsafe {
        crate::task::user::enter_user_mode(0x400000, stack);
    }
}

pub fn sys_clone(_flags: u64, stack: u64) -> isize {
    if stack != 0 {
        CLONE_USER_STACK.store(stack, core::sync::atomic::Ordering::SeqCst);
        let tid = crate::task::scheduler::spawn("user_clone", clone_runner_trampoline, 6);
        tid as isize
    } else {
        0 // Child in fork
    }
}

pub fn sys_wait4(_pid: isize, status_ptr: *mut i32, _options: i32) -> isize {
    if !status_ptr.is_null() {
        unsafe {
            *status_ptr = 0; // WEXITSTATUS = 0
        }
    }
    1
}

pub fn sys_openat(_dfd: i32, filename_ptr: *const u8, flags: u32) -> isize {
    if filename_ptr.is_null() {
        return -1;
    }
    let mut len = 0;
    unsafe {
        while *filename_ptr.add(len) != 0 && len < 256 {
            len += 1;
        }
    }
    sys_open(filename_ptr, len, flags)
}

pub fn sys_readlink(path_ptr: *const u8, buf: *mut u8, bufsiz: usize) -> isize {
    if path_ptr.is_null() || buf.is_null() || bufsiz == 0 {
        return -1;
    }
    -1 // EINVAL (not a symlink)
}

pub fn sys_pipe2(pipefd_ptr: *mut [i32; 2], _flags: i32) -> isize {
    if pipefd_ptr.is_null() {
        return -1;
    }
    unsafe {
        (*pipefd_ptr)[0] = 3; // Reader
        (*pipefd_ptr)[1] = 4; // Writer
    }
    0
}

pub fn sys_dup(oldfd: usize) -> isize {
    (oldfd + 1) as isize
}

pub fn sys_dup2(_oldfd: usize, newfd: usize) -> isize {
    newfd as isize
}


