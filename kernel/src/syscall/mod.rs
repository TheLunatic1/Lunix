use crate::task::process::FdTarget;
use crate::{lunix_print, lunix_println};
use core::slice;
use spin::Mutex;

pub mod unix_socket;

pub static IN_MEMORY_FILES: spin::Mutex<alloc::collections::BTreeMap<alloc::string::String, alloc::sync::Arc<spin::Mutex<alloc::vec::Vec<u8>>>>> =
    spin::Mutex::new(alloc::collections::BTreeMap::new());

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
pub const LINUX_SYS_MPROTECT: usize = 10;
pub const LINUX_SYS_MUNMAP: usize = 11;
pub const LINUX_SYS_BRK: usize = 12;
pub const LINUX_SYS_RT_SIGACTION: usize = 13;
pub const LINUX_SYS_RT_SIGPROCMASK: usize = 14;
pub const LINUX_SYS_RT_SIGRETURN: usize = 15;
pub const LINUX_SYS_IOCTL: usize = 16;
pub const LINUX_SYS_PREAD64: usize = 17;
pub const LINUX_SYS_PWRITE64: usize = 18;
pub const LINUX_SYS_READV: usize = 19;
pub const LINUX_SYS_WRITEV: usize = 20;
pub const LINUX_SYS_ACCESS: usize = 21;
pub const LINUX_SYS_PIPE: usize = 22;
pub const LINUX_SYS_SELECT: usize = 23;
pub const LINUX_SYS_SCHED_YIELD: usize = 24;
pub const LINUX_SYS_MADVISE: usize = 28;
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
pub const LINUX_SYS_GETSOCKNAME: usize = 51;
pub const LINUX_SYS_GETPEERNAME: usize = 52;
pub const LINUX_SYS_SOCKETPAIR: usize = 53;
pub const LINUX_SYS_SETSOCKOPT: usize = 54;
pub const LINUX_SYS_GETSOCKOPT: usize = 55;
pub const LINUX_SYS_CLONE: usize = 56;
pub const LINUX_SYS_FORK: usize = 57;
pub const LINUX_SYS_VFORK: usize = 58;
pub const LINUX_SYS_EXECVE: usize = 59;
pub const LINUX_SYS_EXIT: usize = 60;
pub const LINUX_SYS_WAIT4: usize = 61;
pub const LINUX_SYS_KILL: usize = 62;
pub const LINUX_SYS_UNAME: usize = 63;
pub const LINUX_SYS_FCNTL: usize = 72;
pub const LINUX_SYS_GETCWD: usize = 79;
pub const LINUX_SYS_CHDIR: usize = 80;
pub const LINUX_SYS_RENAME: usize = 82;
pub const LINUX_SYS_MKDIR: usize = 83;
pub const LINUX_SYS_RMDIR: usize = 84;
pub const LINUX_SYS_UNLINK: usize = 87;
pub const LINUX_SYS_SYMLINK: usize = 88;
pub const LINUX_SYS_READLINK: usize = 89;
pub const LINUX_SYS_CHMOD: usize = 90;
pub const LINUX_SYS_FCHMOD: usize = 91;
pub const LINUX_SYS_CHOWN: usize = 92;
pub const LINUX_SYS_FCHOWN: usize = 93;
pub const LINUX_SYS_GETTIMEOFDAY: usize = 96;
pub const LINUX_SYS_GETRLIMIT: usize = 97;
pub const LINUX_SYS_SYSINFO: usize = 99;
pub const LINUX_SYS_GETUID: usize = 102;
pub const LINUX_SYS_GETGID: usize = 104;
pub const LINUX_SYS_SETUID: usize = 105;
pub const LINUX_SYS_SETGID: usize = 106;
pub const LINUX_SYS_GETEUID: usize = 107;
pub const LINUX_SYS_GETEGID: usize = 108;
pub const LINUX_SYS_SETPGID: usize = 109;
pub const LINUX_SYS_GETPPID: usize = 110;
pub const LINUX_SYS_GETPGRP: usize = 111;
pub const LINUX_SYS_SETSID: usize = 112;
pub const LINUX_SYS_GETGROUPS: usize = 115;
pub const LINUX_SYS_GETPGID: usize = 121;
pub const LINUX_SYS_STATFS: usize = 137;
pub const LINUX_SYS_FSTATFS: usize = 138;
pub const LINUX_SYS_ARCH_PRCTL: usize = 158;
pub const LINUX_SYS_SETRLIMIT: usize = 160;
pub const LINUX_SYS_MOUNT: usize = 165;
pub const LINUX_SYS_UMOUNT2: usize = 166;
pub const LINUX_SYS_REBOOT: usize = 169;
pub const LINUX_SYS_GETTID: usize = 186;
pub const LINUX_SYS_FUTEX: usize = 202;
pub const LINUX_SYS_GETDENTS64: usize = 217;
pub const LINUX_SYS_SET_TID_ADDRESS: usize = 218;
pub const LINUX_SYS_CLOCK_GETTIME: usize = 228;
pub const LINUX_SYS_CLOCK_NANOSLEEP: usize = 230;
pub const LINUX_SYS_EXIT_GROUP: usize = 231;
pub const LINUX_SYS_TGKILL: usize = 234;
pub const LINUX_SYS_OPENAT: usize = 257;
pub const LINUX_SYS_MKDIRAT: usize = 258;
pub const LINUX_SYS_FSTATAT: usize = 262;
pub const LINUX_SYS_UNLINKAT: usize = 263;
pub const LINUX_SYS_READLINKAT: usize = 267;
pub const LINUX_SYS_FACCESSAT: usize = 269;
pub const LINUX_SYS_SET_ROBUST_LIST: usize = 273;
pub const LINUX_SYS_GET_ROBUST_LIST: usize = 274;
pub const LINUX_SYS_ACCEPT4: usize = 288;
pub const LINUX_SYS_DUP3: usize = 292;
pub const LINUX_SYS_PIPE2: usize = 293;
pub const LINUX_SYS_PRLIMIT64: usize = 302;
pub const LINUX_SYS_GETRANDOM: usize = 318;
pub const LINUX_SYS_RSEQ: usize = 334;

// Linux arch_prctl codes
pub const ARCH_SET_GS: u64 = 0x1001;
pub const ARCH_SET_FS: u64 = 0x1002;
pub const ARCH_GET_FS: u64 = 0x1003;
pub const ARCH_GET_GS: u64 = 0x1004;

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
static USER_MMAP: Mutex<u64> = Mutex::new(0x0000_7000_0000_0000);

#[repr(C)]
#[derive(Clone, Copy)]
pub struct LinuxTimeSpec {
    pub tv_sec: i64,
    pub tv_nsec: i64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct LinuxTimeVal {
    pub tv_sec: i64,
    pub tv_usec: i64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct LinuxIoVec {
    pub iov_base: *const u8,
    pub iov_len: usize,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct LinuxRlimit {
    pub rlim_cur: u64,
    pub rlim_max: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct LinuxSysInfo {
    pub uptime: i64,
    pub loads: [u64; 3],
    pub totalram: u64,
    pub freeram: u64,
    pub sharedram: u64,
    pub bufferram: u64,
    pub totalswap: u64,
    pub freeswap: u64,
    pub procs: u16,
    pub pad: u16,
    pub totalhigh: u64,
    pub freehigh: u64,
    pub mem_unit: u32,
    pub _f: [u8; 8],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct LinuxStatFs {
    pub f_type: i64,
    pub f_bsize: i64,
    pub f_blocks: u64,
    pub f_bfree: u64,
    pub f_bavail: u64,
    pub f_files: u64,
    pub f_ffree: u64,
    pub f_fsid: [i32; 2],
    pub f_namelen: i64,
    pub f_frsize: i64,
    pub f_flags: i64,
    pub f_spare: [i64; 4],
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
        LINUX_SYS_OPEN => sys_open(arg1 as *const u8, arg2 as u32, arg3 as u32) as u64,
        LINUX_SYS_CLOSE => sys_close(arg1 as usize) as u64,
        LINUX_SYS_STAT => sys_stat(arg1 as *const u8, arg2 as *mut LinuxStat) as u64,
        LINUX_SYS_FSTAT => sys_fstat(arg1 as usize, arg2 as *mut LinuxStat) as u64,
        LINUX_SYS_POLL => sys_poll(arg1 as *mut LinuxPollFd, arg2 as usize, arg3 as i32) as u64,
        LINUX_SYS_LSEEK => sys_lseek(arg1 as usize, arg2 as i64, arg3 as i32) as u64,
        LINUX_SYS_MMAP => sys_mmap(arg1, arg2, arg3 as u32, arg4 as u32, arg5 as i32, arg6) as u64,
        LINUX_SYS_MPROTECT => 0, // Memory protection OK
        LINUX_SYS_MUNMAP => sys_munmap(arg1, arg2) as u64,
        LINUX_SYS_BRK => sys_brk(arg1),
        LINUX_SYS_RT_SIGACTION => 0, // Signal action set OK
        LINUX_SYS_RT_SIGPROCMASK => 0, // Signal mask set OK
        LINUX_SYS_RT_SIGRETURN => 0, // Signal return OK
        LINUX_SYS_IOCTL => sys_ioctl(arg1 as usize, arg2, arg3) as u64,
        LINUX_SYS_PREAD64 => sys_pread64(arg1 as usize, arg2 as *mut u8, arg3 as usize, arg4) as u64,
        LINUX_SYS_PWRITE64 => sys_write(arg1 as usize, arg2 as *const u8, arg3 as usize) as u64,
        LINUX_SYS_READV => sys_readv(arg1 as usize, arg2 as *const LinuxIoVec, arg3 as usize) as u64,
        LINUX_SYS_WRITEV => sys_writev(arg1 as usize, arg2 as *const LinuxIoVec, arg3 as usize) as u64,
        LINUX_SYS_ACCESS => sys_access(arg1 as *const u8, arg2 as u32) as u64,
        LINUX_SYS_PIPE => sys_pipe2(arg1 as *mut [i32; 2], 0) as u64,
        LINUX_SYS_SELECT => 0,
        LINUX_SYS_SCHED_YIELD => sys_yield() as u64,
        LINUX_SYS_MADVISE => 0,
        LINUX_SYS_DUP => sys_dup(arg1 as usize) as u64,
        LINUX_SYS_DUP2 => sys_dup2(arg1 as usize, arg2 as usize) as u64,
        LINUX_SYS_NANOSLEEP => sys_nanosleep(arg1 as *const LinuxTimeSpec) as u64,
        LINUX_SYS_GETPID => sys_getpid() as u64,
        LINUX_SYS_SOCKET => sys_socket(arg1 as i32, arg2 as i32, arg3 as i32) as u64,
        LINUX_SYS_CONNECT => sys_connect(arg1 as usize, arg2 as *const u8, arg3 as usize) as u64,
        LINUX_SYS_ACCEPT | LINUX_SYS_ACCEPT4 => sys_accept(arg1 as usize, arg2 as *mut u8, arg3 as *mut u32) as u64,
        LINUX_SYS_SENDTO => sys_sendto(arg1 as usize, arg2 as *const u8, arg3 as usize, arg4 as u32, arg5 as *const u8) as u64,
        LINUX_SYS_RECVFROM => sys_recvfrom(arg1 as usize, arg2 as *mut u8, arg3 as usize) as u64,
        LINUX_SYS_BIND => sys_bind(arg1 as usize, arg2 as *const u8, arg3 as usize) as u64,
        LINUX_SYS_LISTEN => sys_listen(arg1 as usize, arg2 as usize) as u64,
        LINUX_SYS_GETSOCKNAME | LINUX_SYS_GETPEERNAME => sys_getsockname(arg1 as usize, arg2 as *mut u8, arg3 as *mut u32) as u64,
        LINUX_SYS_SOCKETPAIR => sys_socketpair(arg1 as i32, arg2 as i32, arg3 as i32, arg4 as *mut [i32; 2]) as u64,
        LINUX_SYS_SETSOCKOPT => sys_setsockopt(arg1 as usize, arg2 as i32, arg3 as i32, arg4 as *const u8, arg5 as u32) as u64,
        LINUX_SYS_GETSOCKOPT => sys_getsockopt(arg1 as usize, arg2 as i32, arg3 as i32, arg4 as *mut u8, arg5 as *mut u32) as u64,
        LINUX_SYS_CLONE => sys_clone(arg1, arg2) as u64,
        LINUX_SYS_FORK => sys_clone(0, 0) as u64,
        LINUX_SYS_VFORK => sys_vfork() as u64,
        LINUX_SYS_EXECVE => sys_execve(arg1 as *const u8, arg2 as *const *const u8, arg3 as *const *const u8) as u64,
        LINUX_SYS_EXIT => sys_exit(arg1 as i32),
        LINUX_SYS_WAIT4 => sys_wait4(arg1 as isize, arg2 as *mut i32, arg3 as i32) as u64,
        LINUX_SYS_KILL => 0, // Kill OK
        LINUX_SYS_UNAME => sys_uname(arg1 as *mut LinuxUtsName) as u64,
        LINUX_SYS_FCNTL => sys_fcntl(arg1 as usize, arg2 as usize, arg3) as u64,
        LINUX_SYS_GETCWD => sys_getcwd(arg1 as *mut u8, arg2 as usize) as u64,
        LINUX_SYS_CHDIR => sys_chdir(arg1 as *const u8) as u64,
        LINUX_SYS_RENAME => sys_rename(arg1 as *const u8, arg2 as *const u8) as u64,
        LINUX_SYS_MKDIR => sys_mkdir(arg1 as *const u8, arg2 as u32) as u64,
        LINUX_SYS_RMDIR => sys_rmdir(arg1 as *const u8) as u64,
        LINUX_SYS_UNLINK => sys_unlink(arg1 as *const u8) as u64,
        LINUX_SYS_SYMLINK => 0,
        LINUX_SYS_READLINK => sys_readlink(arg1 as *const u8, arg2 as *mut u8, arg3 as usize) as u64,
        LINUX_SYS_CHMOD | LINUX_SYS_FCHMOD => 0,
        LINUX_SYS_CHOWN | LINUX_SYS_FCHOWN => 0,
        LINUX_SYS_GETTIMEOFDAY => sys_gettimeofday(arg1 as *mut LinuxTimeVal) as u64,
        LINUX_SYS_GETRLIMIT => sys_getrlimit(arg1 as u32, arg2 as *mut LinuxRlimit) as u64,
        LINUX_SYS_SYSINFO => sys_sysinfo(arg1 as *mut LinuxSysInfo) as u64,
        LINUX_SYS_GETUID | LINUX_SYS_GETEUID | LINUX_SYS_GETGID | LINUX_SYS_GETEGID => 0,
        LINUX_SYS_SETUID | LINUX_SYS_SETGID | LINUX_SYS_SETPGID => 0,
        LINUX_SYS_GETPPID => sys_getppid() as u64,
        LINUX_SYS_GETPGRP | LINUX_SYS_GETPGID | LINUX_SYS_SETSID => sys_getpid() as u64,
        LINUX_SYS_GETGROUPS => sys_getgroups(arg1 as usize, arg2 as *mut u32) as u64,
        LINUX_SYS_STATFS | LINUX_SYS_FSTATFS => sys_statfs(arg2 as *mut LinuxStatFs) as u64,
        LINUX_SYS_ARCH_PRCTL => sys_arch_prctl(arg1, arg2) as u64,
        LINUX_SYS_SETRLIMIT => 0,
        LINUX_SYS_MOUNT => sys_mount(arg1 as *const u8, arg2 as *const u8, arg3 as *const u8, arg4, arg5 as *const u8) as u64,
        LINUX_SYS_UMOUNT2 => sys_umount2(arg1 as *const u8, arg2 as i32) as u64,
        LINUX_SYS_REBOOT => 0,
        LINUX_SYS_GETTID => sys_getpid() as u64,
        LINUX_SYS_FUTEX => sys_futex(arg1 as *const u32, arg2 as i32, arg3 as u32, arg4, arg5, arg6 as u32) as u64,
        LINUX_SYS_GETDENTS64 => sys_getdents64(arg1 as usize, arg2 as *mut u8, arg3 as usize) as u64,
        LINUX_SYS_SET_TID_ADDRESS => sys_set_tid_address(arg1 as *mut i32) as u64,
        LINUX_SYS_CLOCK_GETTIME => sys_clock_gettime(arg1 as i32, arg2 as *mut LinuxTimeSpec) as u64,
        LINUX_SYS_CLOCK_NANOSLEEP => sys_clock_nanosleep(arg1 as i32, arg2 as i32, arg3 as *const LinuxTimeSpec, arg4 as *mut LinuxTimeSpec) as u64,
        LINUX_SYS_EXIT_GROUP => sys_exit(arg1 as i32),
        LINUX_SYS_TGKILL => 0,
        LINUX_SYS_OPENAT => sys_openat(arg1 as i32, arg2 as *const u8, arg3 as u32) as u64,
        LINUX_SYS_MKDIRAT => sys_mkdirat(arg1 as i32, arg2 as *const u8, arg3 as u32) as u64,
        LINUX_SYS_FSTATAT => sys_fstatat(arg1 as i32, arg2 as *const u8, arg3 as *mut LinuxStat, arg4 as u32) as u64,
        LINUX_SYS_UNLINKAT => sys_unlinkat(arg1 as i32, arg2 as *const u8, arg3 as u32) as u64,
        LINUX_SYS_READLINKAT => sys_readlinkat(arg1 as i32, arg2 as *const u8, arg3 as *mut u8, arg4 as usize) as u64,
        LINUX_SYS_FACCESSAT => sys_faccessat(arg1 as i32, arg2 as *const u8, arg3 as u32, arg4 as u32) as u64,
        LINUX_SYS_SET_ROBUST_LIST => sys_set_robust_list(arg1, arg2 as usize) as u64,
        LINUX_SYS_GET_ROBUST_LIST => sys_get_robust_list(arg1 as i32, arg2 as *mut u64, arg3 as *mut usize) as u64,
        LINUX_SYS_DUP3 => sys_dup3(arg1 as usize, arg2 as usize, arg3 as i32) as u64,
        LINUX_SYS_PIPE2 => sys_pipe2(arg1 as *mut [i32; 2], arg2 as i32) as u64,
        LINUX_SYS_PRLIMIT64 => sys_prlimit64(arg1 as i32, arg2 as u32, arg3 as *const LinuxRlimit, arg4 as *mut LinuxRlimit) as u64,
        LINUX_SYS_GETRANDOM => sys_getrandom(arg1 as *mut u8, arg2 as usize, arg3 as u32) as u64,
        LINUX_SYS_RSEQ => 0,

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
    if let Some(proc_arc) = crate::task::scheduler::get_current_process() {
        let mut proc = proc_arc.lock();
        if let Some(parent_tid) = proc.vfork_waiting_parent.take() {
            crate::task::scheduler::unblock_thread(parent_tid);
        }
    }
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
                        if let FdTarget::File { ref mut offset, ref buffer, .. } = desc.target {
                            let mut data = buffer.lock();
                            let slice = unsafe { slice::from_raw_parts(buf, len) };
                            let needed = *offset + len;
                            if needed > data.len() {
                                data.resize(needed, 0);
                            }
                            data[*offset..*offset + len].copy_from_slice(slice);
                            *offset += len;
                            return len as isize;
                        }
                    }
                }
                return -9;
            }
            FdTarget::VfsHandle { ref handle, .. } => {
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
            FdTarget::UnixSocket(ref sock) => {
                let non_blocking = (flags & 0x800) != 0;
                let slice = unsafe { slice::from_raw_parts(buf, len) };
                let sock_guard = sock.lock();
                return sock_guard.write(slice, non_blocking);
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
                        if let FdTarget::File { ref mut offset, ref buffer, .. } = desc.target {
                            let data = buffer.lock();
                            let size = data.len();
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
            FdTarget::VfsHandle { ref handle, .. } => {
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
            FdTarget::UnixSocket(ref sock) => {
                let non_blocking = (flags & 0x800) != 0;
                let slice = unsafe { slice::from_raw_parts_mut(buf, len) };
                let sock_guard = sock.lock();
                return sock_guard.read(slice, non_blocking);
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
                FdTarget::File { offset: ref mut f_off, ref buffer, .. } => {
                    let size = buffer.lock().len() as i64;
                    let new_off: i64 = match seek_from {
                        crate::fs::file::SeekFrom::Start(s) => s as i64,
                        crate::fs::file::SeekFrom::Current(c) => *f_off as i64 + c,
                        crate::fs::file::SeekFrom::End(e) => size + e,
                    };
                    if new_off < 0 {
                        return -22; // -EINVAL
                    }
                    *f_off = new_off as usize;
                    return new_off as isize;
                }
                FdTarget::VfsHandle { ref handle, .. } => {
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

pub fn sys_pread64(fd: usize, buf: *mut u8, count: usize, pos: u64) -> isize {
    crate::lunix_serial_println!("  [SYS_PREAD64] fd={}, count={}, pos={}", fd, count, pos);
    if buf.is_null() || count == 0 {
        return 0;
    }
    if let Some(proc_arc) = crate::task::scheduler::get_current_process() {
        let proc = proc_arc.lock();
        if let Some(desc_arc) = proc.get_fd(fd) {
            let desc = desc_arc.lock();
            match desc.target {
                FdTarget::File { ref buffer, .. } => {
                    let data = buffer.lock();
                    let off = pos as usize;
                    if off >= data.len() {
                        return 0;
                    }
                    let avail = data.len() - off;
                    let to_read = count.min(avail);
                    let slice = unsafe { core::slice::from_raw_parts_mut(buf, count) };
                    slice[..to_read].copy_from_slice(&data[off..off + to_read]);
                    return to_read as isize;
                }
                FdTarget::VfsHandle { ref handle, .. } => {
                    let mut h = handle.lock();
                    let orig_pos = h.seek(crate::fs::file::SeekFrom::Current(0)).unwrap_or(0);
                    let _ = h.seek(crate::fs::file::SeekFrom::Start(pos));
                    let slice = unsafe { core::slice::from_raw_parts_mut(buf, count) };
                    let bytes_read = h.read(slice).unwrap_or(0);
                    let _ = h.seek(crate::fs::file::SeekFrom::Start(orig_pos));
                    return bytes_read as isize;
                }
                _ => return -9,
            }
        }
    }
    -9
}

pub fn sys_writev(fd: usize, iov_ptr: *const LinuxIoVec, iovcnt: usize) -> isize {
    crate::lunix_serial_println!("  [SYS_WRITEV] fd={}, iovcnt={}", fd, iovcnt);
    if iov_ptr.is_null() || iovcnt == 0 {
        return 0;
    }
    let mut total_written = 0isize;
    let iovs = unsafe { core::slice::from_raw_parts(iov_ptr, iovcnt) };
    for iov in iovs {
        if !iov.iov_base.is_null() && iov.iov_len > 0 {
            let written = sys_write(fd, iov.iov_base, iov.iov_len);
            if written > 0 {
                total_written += written;
            }
        }
    }
    total_written
}

pub fn sys_readv(fd: usize, iov_ptr: *const LinuxIoVec, iovcnt: usize) -> isize {
    crate::lunix_serial_println!("  [SYS_READV] fd={}, iovcnt={}", fd, iovcnt);
    if iov_ptr.is_null() || iovcnt == 0 {
        return 0;
    }
    let mut total_read = 0isize;
    let iovs = unsafe { core::slice::from_raw_parts(iov_ptr, iovcnt) };
    for iov in iovs {
        if !iov.iov_base.is_null() && iov.iov_len > 0 {
            let read_bytes = sys_read(fd, iov.iov_base as *mut u8, iov.iov_len);
            if read_bytes > 0 {
                total_read += read_bytes;
            } else {
                break;
            }
        }
    }
    total_read
}

pub fn sys_open(path_ptr: *const u8, flags: u32, _mode: u32) -> isize {
    sys_openat(-100, path_ptr, flags)
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

pub fn sys_stat(path_ptr: *const u8, statbuf: *mut LinuxStat) -> isize {
    sys_fstatat(-100, path_ptr, statbuf, 0)
}

pub fn sys_mmap(addr: u64, length: u64, _prot: u32, _flags: u32, fd: i32, offset: u64) -> u64 {
    let pages = (length + 4095) / 4096;
    let target_addr = if addr != 0 {
        addr
    } else {
        // Allocate in user mmap range
        let mut mmap_lock = USER_MMAP.lock();
        let alloc_addr = *mmap_lock;
        *mmap_lock += pages * 4096;
        alloc_addr
    };

    let map_flags = x86_64::structures::paging::PageTableFlags::PRESENT
        | x86_64::structures::paging::PageTableFlags::WRITABLE
        | x86_64::structures::paging::PageTableFlags::USER_ACCESSIBLE;

    // Direct Framebuffer MMIO mapping check
    if fd >= 0 {
        if let Some(proc_arc) = crate::task::scheduler::get_current_process() {
            let proc = proc_arc.lock();
            if let Some(desc_arc) = proc.get_fd(fd as usize) {
                let desc = desc_arc.lock();
                if let FdTarget::VfsHandle { ref handle, .. } = desc.target {
                    let h = handle.lock();
                    let expected_fb_size = crate::display::console::get_framebuffer_info()
                        .map(|fb| (fb.stride * fb.height * fb.bytes_per_pixel) as u64)
                        .unwrap_or(0);
                    if expected_fb_size > 0 && h.size() == expected_fb_size {
                        if let Some(fb) = crate::display::console::get_framebuffer_info() {
                            let fb_phys = fb.base_address + offset;
                            for p in 0..pages {
                                let page_vaddr = x86_64::VirtAddr::new(target_addr + (p * 4096));
                                let phys_addr = x86_64::PhysAddr::new(fb_phys + (p * 4096));
                                let _ = crate::mm::vmm::map_page(page_vaddr, phys_addr, map_flags);
                            }
                            return target_addr;
                        }
                    }
                }
            }
        }
    }

    for p in 0..pages {
        let page_vaddr = x86_64::VirtAddr::new(target_addr + (p * 4096));
        if !crate::mm::vmm::is_page_mapped(page_vaddr) {
            if let Some(frame) = crate::mm::pmm::alloc_frame() {
                if let Err(e) = crate::mm::vmm::map_page(page_vaddr, frame, map_flags) {
                    crate::lunix_serial_println!("  [SYS_MMAP_ERR] failed to map page p={} (virt 0x{:X}): {}", p, page_vaddr.as_u64(), e);
                } else {
                    unsafe {
                        core::ptr::write_bytes(page_vaddr.as_mut_ptr::<u8>(), 0, 4096);
                    }
                }
            } else {
                crate::lunix_serial_println!("  [SYS_MMAP_ERR] PMM out of frames for page p={}", p);
            }
        }
        if p == 0 || p == 257 || p == 258 || p == 259 || p == pages - 1 {
            crate::lunix_serial_println!("  [SYS_MMAP_PAGE_CHECK] p={}/{}, virt 0x{:X}, mapped={}", p, pages, page_vaddr.as_u64(), crate::mm::vmm::is_page_mapped(page_vaddr));
        }
    }


    if fd < 0 {
        unsafe {
            core::ptr::write_bytes(target_addr as *mut u8, 0, length as usize);
        }
    } else {
        // File-backed mapping
        if let Some(proc_arc) = crate::task::scheduler::get_current_process() {
            let proc = proc_arc.lock();
            if let Some(desc_arc) = proc.get_fd(fd as usize) {
                let desc = desc_arc.lock();
                match desc.target {
                    FdTarget::VfsHandle { ref handle, .. } => {
                        let mut h = handle.lock();
                        let _ = h.seek(crate::fs::file::SeekFrom::Start(offset));
                        let dest_slice = unsafe { core::slice::from_raw_parts_mut(target_addr as *mut u8, length as usize) };
                        let bytes_read = h.read(dest_slice).unwrap_or(0);
                        if bytes_read < length as usize {
                            unsafe {
                                core::ptr::write_bytes(
                                    (target_addr as *mut u8).add(bytes_read),
                                    0,
                                    length as usize - bytes_read,
                                );
                            }
                        }
                    }
                    FdTarget::File { ref buffer, .. } => {
                        let data = buffer.lock();
                        let off = offset as usize;
                        if off < data.len() {
                            let copy_len = (data.len() - off).min(length as usize);
                            unsafe {
                                core::ptr::copy_nonoverlapping(
                                    data.as_ptr().add(off),
                                    target_addr as *mut u8,
                                    copy_len,
                                );
                                if copy_len < length as usize {
                                    core::ptr::write_bytes(
                                        (target_addr as *mut u8).add(copy_len),
                                        0,
                                        length as usize - copy_len,
                                    );
                                }
                            }
                        } else {
                            unsafe {
                                core::ptr::write_bytes(target_addr as *mut u8, 0, length as usize);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    crate::lunix_serial_println!("  [SYS_MMAP_DONE] target=0x{:X}, len=0x{:X}", target_addr, length);

    target_addr
}


pub fn sys_sysinfo(info: *mut LinuxSysInfo) -> isize {
    if info.is_null() {
        return -14; // -EFAULT
    }
    let ticks = crate::drivers::timer::get_ticks();
    let uptime_sec = (ticks / 1000) as i64;
    let (total_bytes, usable_bytes, used_bytes) = crate::mm::pmm::get_memory_stats();
    let free_bytes = usable_bytes.saturating_sub(used_bytes);
    let procs = crate::task::scheduler::PROCESS_TABLE.lock().len() as u16;

    unsafe {
        (*info).uptime = uptime_sec;
        (*info).loads = [65536 / 10, 65536 / 20, 65536 / 30];
        (*info).totalram = total_bytes as u64;
        (*info).freeram = free_bytes as u64;
        (*info).sharedram = 0;
        (*info).bufferram = 1024 * 1024;
        (*info).totalswap = 0;
        (*info).freeswap = 0;
        (*info).procs = procs.max(1);
        (*info).pad = 0;
        (*info).totalhigh = 0;
        (*info).freehigh = 0;
        (*info).mem_unit = 1;
    }
    0
}

pub fn sys_futex(uaddr: *const u32, op: i32, val: u32, _timeout: u64, _uaddr2: u64, _val3: u32) -> isize {
    let cmd = op & 0x7F; // Mask out FUTEX_PRIVATE_FLAG
    match cmd {
        0 => { // FUTEX_WAIT
            if uaddr.is_null() {
                return -14;
            }
            let current_val = unsafe { *uaddr };
            if current_val != val {
                return -11; // -EAGAIN
            }
            crate::task::scheduler::yield_now();
            0
        }
        1 => { // FUTEX_WAKE
            val as isize
        }
        _ => 0,
    }
}

pub fn sys_set_tid_address(_tidptr: *mut i32) -> isize {
    crate::task::scheduler::current_pid() as isize
}

pub fn sys_set_robust_list(_head: u64, _len: usize) -> isize {
    0
}

pub fn sys_get_robust_list(_pid: i32, head_ptr: *mut u64, len_ptr: *mut usize) -> isize {
    if !head_ptr.is_null() {
        unsafe { *head_ptr = 0; }
    }
    if !len_ptr.is_null() {
        unsafe { *len_ptr = 0; }
    }
    0
}

pub fn sys_mount(_dev_name: *const u8, _dir_name: *const u8, _type_name: *const u8, _flags: u64, _data: *const u8) -> isize {
    0 // Mount success
}

pub fn sys_umount2(_target: *const u8, _flags: i32) -> isize {
    0 // Umount success
}

pub fn sys_rename(_oldpath: *const u8, _newpath: *const u8) -> isize {
    0
}

pub fn sys_munmap(_addr: u64, _length: u64) -> isize {
    0
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FbBitfield {
    pub offset: u32,
    pub length: u32,
    pub msb_right: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FbVarScreenInfo {
    pub xres: u32,
    pub yres: u32,
    pub xres_virtual: u32,
    pub yres_virtual: u32,
    pub xoffset: u32,
    pub yoffset: u32,
    pub bits_per_pixel: u32,
    pub grayscale: u32,
    pub red: FbBitfield,
    pub green: FbBitfield,
    pub blue: FbBitfield,
    pub transp: FbBitfield,
    pub nonstd: u32,
    pub activate: u32,
    pub height: u32,
    pub width: u32,
    pub accel_flags: u32,
    pub pixclock: u32,
    pub left_margin: u32,
    pub right_margin: u32,
    pub upper_margin: u32,
    pub lower_margin: u32,
    pub hsync_len: u32,
    pub vsync_len: u32,
    pub sync: u32,
    pub vmode: u32,
    pub rotate: u32,
    pub colorspace: u32,
    pub reserved: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FbFixScreenInfo {
    pub id: [u8; 16],
    pub smem_start: u64,
    pub smem_len: u32,
    pub type_: u32,
    pub type_aux: u32,
    pub visual: u32,
    pub xpanstep: u16,
    pub ypanstep: u16,
    pub ywrapstep: u16,
    pub line_length: u32,
    pub mmio_start: u64,
    pub mmio_len: u32,
    pub accel: u32,
    pub capabilities: u16,
    pub reserved: [u16; 2],
}

pub fn sys_ioctl(fd: usize, request: u64, arg: u64) -> isize {
    const TIOCGWINSZ: u64 = 0x5413;
    const TCGETS: u64 = 0x5401;
    const TCSETS: u64 = 0x5402;
    const TCSETSW: u64 = 0x5403;
    const TCSETSF: u64 = 0x5404;
    const FIONBIO: u64 = 0x5421;

    // Framebuffer ioctls
    const FBIOGET_VSCREENINFO: u64 = 0x4600;
    const FBIOPUT_VSCREENINFO: u64 = 0x4601;
    const FBIOGET_FSCREENINFO: u64 = 0x4602;

    // Virtual terminal & keyboard mode ioctls
    const VT_OPENQRY: u64 = 0x5600;
    const VT_GETMODE: u64 = 0x5601;
    const VT_SETMODE: u64 = 0x5602;
    const VT_ACTIVATE: u64 = 0x5606;
    const VT_WAITACTIVE: u64 = 0x5607;
    const KDSETMODE: u64 = 0x4B3A;
    const KDGETMODE: u64 = 0x4B3B;

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
        FBIOGET_VSCREENINFO => {
            if arg == 0 {
                return -14;
            }
            let fb_info = crate::display::console::get_framebuffer_info();
            let width = fb_info.map(|f| f.width as u32).unwrap_or(1024);
            let height = fb_info.map(|f| f.height as u32).unwrap_or(768);
            unsafe {
                let vinfo = &mut *(arg as *mut FbVarScreenInfo);
                vinfo.xres = width;
                vinfo.yres = height;
                vinfo.xres_virtual = width;
                vinfo.yres_virtual = height;
                vinfo.xoffset = 0;
                vinfo.yoffset = 0;
                vinfo.bits_per_pixel = 32;
                vinfo.grayscale = 0;
                vinfo.red = FbBitfield { offset: 16, length: 8, msb_right: 0 };
                vinfo.green = FbBitfield { offset: 8, length: 8, msb_right: 0 };
                vinfo.blue = FbBitfield { offset: 0, length: 8, msb_right: 0 };
                vinfo.transp = FbBitfield { offset: 24, length: 8, msb_right: 0 };
                vinfo.activate = 0;
                vinfo.height = 0xFFFFFFFF;
                vinfo.width = 0xFFFFFFFF;
            }
            0
        }
        FBIOPUT_VSCREENINFO => 0,
        FBIOGET_FSCREENINFO => {
            if arg == 0 {
                return -14;
            }
            let fb_info = crate::display::console::get_framebuffer_info();
            let base = fb_info.map(|f| f.base_address).unwrap_or(0);
            let stride = fb_info.map(|f| f.stride as u32).unwrap_or(1024);
            let height = fb_info.map(|f| f.height as u32).unwrap_or(768);
            let bpp = fb_info.map(|f| f.bytes_per_pixel as u32).unwrap_or(4);
            let smem_len = stride * height * bpp;
            let line_length = stride * bpp;

            unsafe {
                let finfo = &mut *(arg as *mut FbFixScreenInfo);
                finfo.id = *b"lunix-fb\0\0\0\0\0\0\0\0";
                finfo.smem_start = base;
                finfo.smem_len = smem_len;
                finfo.type_ = 0; // FB_TYPE_PACKED_PIXELS
                finfo.type_aux = 0;
                finfo.visual = 2; // FB_VISUAL_TRUECOLOR
                finfo.xpanstep = 0;
                finfo.ypanstep = 0;
                finfo.ywrapstep = 0;
                finfo.line_length = line_length;
                finfo.mmio_start = 0;
                finfo.mmio_len = 0;
                finfo.accel = 0; // FB_ACCEL_NONE
            }
            0
        }
        VT_OPENQRY => {
            if arg != 0 {
                unsafe { *(arg as *mut i32) = 1; }
            }
            0
        }
        VT_GETMODE | VT_SETMODE | VT_ACTIVATE | VT_WAITACTIVE | KDSETMODE => 0,
        KDGETMODE => {
            if arg != 0 {
                unsafe { *(arg as *mut i32) = 0; } // KD_TEXT
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

pub fn sys_clock_nanosleep(_clockid: i32, _flags: i32, req: *const LinuxTimeSpec, _rem: *mut LinuxTimeSpec) -> isize {
    if req.is_null() {
        return -14; // -EFAULT
    }
    let spec = unsafe { &*req };
    let ms = (spec.tv_sec as u64 * 1000) + (spec.tv_nsec as u64 / 1_000_000);
    crate::task::scheduler::sleep_ms(ms.max(1));
    0
}

pub fn sys_getpid() -> isize {
    let pid = crate::task::scheduler::current_pid();
    if pid > 0 { pid as isize } else { 1 }
}


pub fn sys_uname(buf: *mut LinuxUtsName) -> isize {
    if buf.is_null() {
        return -1;
    }

    unsafe {
        let uts = &mut *buf;
        copy_cstr(&mut uts.sysname, b"Linux\0");
        copy_cstr(&mut uts.nodename, b"box\0");
        copy_cstr(&mut uts.release, b"6.8.0-tinycore\0");
        copy_cstr(&mut uts.version, b"#1 SMP PREEMPT 2026-09-14 (Lunix 0.1.0-hybrid)\0");
        copy_cstr(&mut uts.machine, b"x86_64\0");
        copy_cstr(&mut uts.domainname, b"(none)\0");
    }

    0
}

pub fn sys_getrandom(buf: *mut u8, buflen: usize, _flags: u32) -> isize {
    if buf.is_null() {
        return -14; // -EFAULT
    }
    let slice = unsafe { core::slice::from_raw_parts_mut(buf, buflen) };
    let mut seed = unsafe { core::arch::x86_64::_rdtsc() };
    for b in slice.iter_mut() {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        *b = (seed >> 33) as u8;
    }
    buflen as isize
}

fn copy_cstr(dest: &mut [u8; 65], src: &[u8]) {
    for (i, &b) in src.iter().enumerate() {
        if i < 64 {
            dest[i] = b;
        }
    }
    dest[src.len().min(64)] = 0;
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

pub fn sys_chdir(path_ptr: *const u8) -> isize {
    if path_ptr.is_null() {
        return -14;
    }
    let mut len = 0;
    unsafe {
        while *path_ptr.add(len) != 0 && len < 256 {
            len += 1;
        }
    }
    let slice = unsafe { core::slice::from_raw_parts(path_ptr, len) };
    if let Ok(path) = core::str::from_utf8(slice) {
        if let Some(proc_arc) = crate::task::scheduler::get_current_process() {
            let mut proc = proc_arc.lock();
            proc.cwd = alloc::string::String::from(path);
        }
    }
    0
}

pub fn sys_socket(domain: i32, sock_type: i32, protocol: i32) -> isize {
    crate::lunix_serial_println!("  [SYS_SOCKET] domain={}, type={}, proto={}", domain, sock_type, protocol);
    let base_type = sock_type & 0xFF;
    if domain == unix_socket::AF_UNIX || domain == unix_socket::AF_LOCAL {
        let sock_arc = unix_socket::UnixSocket::new(domain, base_type, protocol);
        if let Some(proc_arc) = crate::task::scheduler::get_current_process() {
            let mut proc = proc_arc.lock();
            if let Some(fd) = proc.allocate_fd(crate::task::process::FileDescriptor {
                target: FdTarget::UnixSocket(sock_arc),
                flags: if (sock_type & 0x800) != 0 { 0x800 } else { 0 },
            }) {
                return fd as isize;
            } else {
                return -24; // -EMFILE
            }
        }
    }

    let mut lock = crate::net::NET_STACK.lock();
    if let Some(ref mut stack) = *lock {
        let sid = stack.next_socket_id;
        stack.next_socket_id += 1;
        let sock = crate::net::Socket {
            id: sid,
            domain,
            socket_type: sock_type,
            protocol,
            local_ip: crate::net::DEFAULT_IP,
            local_port: 0,
            remote_ip: [0, 0, 0, 0],
            remote_port: 0,
            state: crate::net::SocketState::Closed,
            recv_queue: alloc::vec::Vec::new(),
        };
        stack.sockets.insert(sid, sock);
        if let Some(proc_arc) = crate::task::scheduler::get_current_process() {
            let mut proc = proc_arc.lock();
            if let Some(fd) = proc.allocate_fd(crate::task::process::FileDescriptor {
                target: FdTarget::Socket(sid),
                flags: 0,
            }) {
                return fd as isize;
            } else {
                return -24;
            }
        }
        sid as isize
    } else {
        -1
    }
}

pub fn sys_bind(sockfd: usize, addr: *const u8, addrlen: usize) -> isize {
    crate::lunix_serial_println!("  [SYS_BIND] fd={}, addr={:p}, len={}", sockfd, addr, addrlen);
    if let Some(proc_arc) = crate::task::scheduler::get_current_process() {
        let proc = proc_arc.lock();
        if let Some(desc_arc) = proc.get_fd(sockfd) {
            let desc = desc_arc.lock();
            if let FdTarget::UnixSocket(ref sock_arc) = desc.target {
                if !addr.is_null() && addrlen >= 2 {
                    let sockaddr = unsafe { &*(addr as *const unix_socket::SockAddrUn) };
                    let path = sockaddr.get_path();
                    return unix_socket::UnixSocket::bind(sock_arc, &path);
                }
                return -22; // -EINVAL
            }
        }
    }

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

pub fn sys_listen(sockfd: usize, backlog: usize) -> isize {
    crate::lunix_serial_println!("  [SYS_LISTEN] fd={}, backlog={}", sockfd, backlog);
    if let Some(proc_arc) = crate::task::scheduler::get_current_process() {
        let proc = proc_arc.lock();
        if let Some(desc_arc) = proc.get_fd(sockfd) {
            let desc = desc_arc.lock();
            if let FdTarget::UnixSocket(ref sock_arc) = desc.target {
                return sock_arc.lock().listen(backlog);
            }
        }
    }
    0
}

pub fn sys_connect(sockfd: usize, addr: *const u8, addrlen: usize) -> isize {
    crate::lunix_serial_println!("  [SYS_CONNECT] fd={}, addr={:p}, len={}", sockfd, addr, addrlen);
    if let Some(proc_arc) = crate::task::scheduler::get_current_process() {
        let proc = proc_arc.lock();
        if let Some(desc_arc) = proc.get_fd(sockfd) {
            let desc = desc_arc.lock();
            if let FdTarget::UnixSocket(ref sock_arc) = desc.target {
                if !addr.is_null() && addrlen >= 2 {
                    let sockaddr = unsafe { &*(addr as *const unix_socket::SockAddrUn) };
                    let path = sockaddr.get_path();
                    return unix_socket::UnixSocket::connect(sock_arc, &path);
                }
                return -22; // -EINVAL
            }
        }
    }

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

pub fn sys_accept(sockfd: usize, addr: *mut u8, addrlen: *mut u32) -> isize {
    crate::lunix_serial_println!("  [SYS_ACCEPT] fd={}", sockfd);
    if let Some(proc_arc) = crate::task::scheduler::get_current_process() {
        let (sock_opt, flags) = {
            let proc = proc_arc.lock();
            if let Some(desc_arc) = proc.get_fd(sockfd) {
                let desc = desc_arc.lock();
                if let FdTarget::UnixSocket(ref s) = desc.target {
                    (Some(s.clone()), desc.flags)
                } else {
                    (None, 0)
                }
            } else {
                (None, 0)
            }
        };

        if let Some(sock_arc) = sock_opt {
            let non_blocking = (flags & 0x800) != 0;
            for _ in 0..50 {
                match unix_socket::UnixSocket::accept(&sock_arc) {
                    Ok(client_sock) => {
                        let mut proc = proc_arc.lock();
                        if let Some(new_fd) = proc.allocate_fd(crate::task::process::FileDescriptor {
                            target: FdTarget::UnixSocket(client_sock),
                            flags: 0,
                        }) {
                            if !addr.is_null() && !addrlen.is_null() {
                                unsafe {
                                    *addrlen = core::mem::size_of::<unix_socket::SockAddrUn>() as u32;
                                    let sun = addr as *mut unix_socket::SockAddrUn;
                                    (*sun).sun_family = unix_socket::AF_UNIX as u16;
                                    (*sun).sun_path = [0; 108];
                                }
                            }
                            return new_fd as isize;
                        } else {
                            return -24; // -EMFILE
                        }
                    }
                    Err(-11) => {
                        if non_blocking {
                            return -11; // -EAGAIN
                        }
                        crate::task::scheduler::sleep_ms(2);
                    }
                    Err(e) => return e,
                }
            }
            return if non_blocking { -11 } else { -11 };
        }
    }
    -22 // -EINVAL
}

pub fn sys_getsockname(sockfd: usize, addr: *mut u8, addrlen: *mut u32) -> isize {
    if addr.is_null() || addrlen.is_null() {
        return -14; // -EFAULT
    }
    if let Some(proc_arc) = crate::task::scheduler::get_current_process() {
        let proc = proc_arc.lock();
        if let Some(desc_arc) = proc.get_fd(sockfd) {
            let desc = desc_arc.lock();
            if let FdTarget::UnixSocket(ref sock) = desc.target {
                let sock_guard = sock.lock();
                unsafe {
                    let sun = &mut *(addr as *mut unix_socket::SockAddrUn);
                    sun.sun_family = unix_socket::AF_UNIX as u16;
                    sun.sun_path = [0; 108];
                    if let Some(ref p) = sock_guard.path {
                        let bytes = p.as_bytes();
                        let copy_len = bytes.len().min(107);
                        sun.sun_path[..copy_len].copy_from_slice(&bytes[..copy_len]);
                    }
                    *addrlen = core::mem::size_of::<unix_socket::SockAddrUn>() as u32;
                }
                return 0;
            }
        }
    }
    0
}

pub fn sys_setsockopt(_sockfd: usize, _level: i32, _optname: i32, _optval: *const u8, _optlen: u32) -> isize {
    0
}

pub fn sys_getsockopt(_sockfd: usize, _level: i32, _optname: i32, _optval: *mut u8, optlen: *mut u32) -> isize {
    if !optlen.is_null() {
        unsafe { *optlen = 4; }
    }
    0
}

pub fn sys_socketpair(_domain: i32, sock_type: i32, _protocol: i32, sv: *mut [i32; 2]) -> isize {
    crate::lunix_serial_println!("  [SYS_SOCKETPAIR] type={}", sock_type);
    if sv.is_null() {
        return -14; // -EFAULT
    }

    let s2c = alloc::sync::Arc::new(spin::Mutex::new(crate::task::pipe::PipeBuffer::new()));
    let c2s = alloc::sync::Arc::new(spin::Mutex::new(crate::task::pipe::PipeBuffer::new()));

    let sock1 = alloc::sync::Arc::new(spin::Mutex::new(unix_socket::UnixSocket {
        id: unix_socket::NEXT_SOCKET_ID.fetch_add(1, core::sync::atomic::Ordering::Relaxed),
        domain: unix_socket::AF_UNIX,
        socket_type: unix_socket::SOCK_STREAM,
        protocol: 0,
        path: None,
        state: unix_socket::UnixSocketState::Connected {
            peer: None,
            rx_buffer: c2s.clone(),
            tx_buffer: s2c.clone(),
        },
        flags: if (sock_type & 0x800) != 0 { 0x800 } else { 0 },
    }));

    let sock2 = alloc::sync::Arc::new(spin::Mutex::new(unix_socket::UnixSocket {
        id: unix_socket::NEXT_SOCKET_ID.fetch_add(1, core::sync::atomic::Ordering::Relaxed),
        domain: unix_socket::AF_UNIX,
        socket_type: unix_socket::SOCK_STREAM,
        protocol: 0,
        path: None,
        state: unix_socket::UnixSocketState::Connected {
            peer: Some(sock1.clone()),
            rx_buffer: s2c,
            tx_buffer: c2s,
        },
        flags: if (sock_type & 0x800) != 0 { 0x800 } else { 0 },
    }));

    if let unix_socket::UnixSocketState::Connected { ref mut peer, .. } = sock1.lock().state {
        *peer = Some(sock2.clone());
    }

    if let Some(proc_arc) = crate::task::scheduler::get_current_process() {
        let mut proc = proc_arc.lock();
        if let Some(fd1) = proc.allocate_fd(crate::task::process::FileDescriptor {
            target: FdTarget::UnixSocket(sock1),
            flags: if (sock_type & 0x800) != 0 { 0x800 } else { 0 },
        }) {
            if let Some(fd2) = proc.allocate_fd(crate::task::process::FileDescriptor {
                target: FdTarget::UnixSocket(sock2),
                flags: if (sock_type & 0x800) != 0 { 0x800 } else { 0 },
            }) {
                unsafe {
                    (*sv)[0] = fd1 as i32;
                    (*sv)[1] = fd2 as i32;
                }
                return 0;
            } else {
                proc.close_fd(fd1);
                return -24; // -EMFILE
            }
        }
    }
    -24
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct LinuxPollFd {
    pub fd: i32,
    pub events: i16,
    pub revents: i16,
}

pub const POLLIN: i16 = 0x0001;
pub const POLLPRI: i16 = 0x0002;
pub const POLLOUT: i16 = 0x0004;
pub const POLLERR: i16 = 0x0008;
pub const POLLHUP: i16 = 0x0010;
pub const POLLNVAL: i16 = 0x0020;

pub fn sys_poll(fds: *mut LinuxPollFd, nfds: usize, timeout: i32) -> isize {
    if fds.is_null() || nfds == 0 {
        if timeout > 0 {
            crate::task::scheduler::sleep_ms(timeout.min(50).max(0) as u64);
        }
        return 0;
    }

    let poll_slice = unsafe { slice::from_raw_parts_mut(fds, nfds) };
    let proc_arc_opt = crate::task::scheduler::get_current_process();

    let mut ready_count = 0;
    for pfd in poll_slice.iter_mut() {
        pfd.revents = 0;
        if pfd.fd < 0 {
            continue;
        }

        if let Some(ref proc_arc) = proc_arc_opt {
            let proc = proc_arc.lock();
            if let Some(desc_arc) = proc.get_fd(pfd.fd as usize) {
                let desc = desc_arc.lock();
                match desc.target {
                    FdTarget::UnixSocket(ref sock) => {
                        let sock_guard = sock.lock();
                        if (pfd.events & POLLIN) != 0 && sock_guard.poll_read_ready() {
                            pfd.revents |= POLLIN;
                        }
                        if (pfd.events & POLLOUT) != 0 && sock_guard.poll_write_ready() {
                            pfd.revents |= POLLOUT;
                        }
                    }
                    FdTarget::PipeRead(ref pipe) => {
                        if (pfd.events & POLLIN) != 0 && pipe.lock().available_to_read() > 0 {
                            pfd.revents |= POLLIN;
                        }
                    }
                    FdTarget::PipeWrite(ref pipe) => {
                        if (pfd.events & POLLOUT) != 0 && pipe.lock().available_to_write() > 0 {
                            pfd.revents |= POLLOUT;
                        }
                    }
                    FdTarget::VfsHandle { .. } | FdTarget::Stdin | FdTarget::Stdout | FdTarget::Stderr => {
                        pfd.revents |= pfd.events & (POLLIN | POLLOUT);
                    }
                    _ => {
                        pfd.revents |= pfd.events & (POLLIN | POLLOUT);
                    }
                }
            } else {
                pfd.revents = POLLNVAL;
            }
        }

        if pfd.revents != 0 {
            ready_count += 1;
        }
    }

    if ready_count == 0 && timeout > 0 {
        crate::task::scheduler::sleep_ms(timeout.min(10).max(0) as u64);
    }

    ready_count
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

pub fn sys_execve(filename_ptr: *const u8, argv_ptr: *const *const u8, _envp_ptr: *const *const u8) -> isize {
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

    let mut args_vec: alloc::vec::Vec<alloc::string::String> = alloc::vec::Vec::new();
    if !argv_ptr.is_null() {
        let mut arg_idx = 0;
        unsafe {
            while arg_idx < 16 {
                let arg_p = *argv_ptr.add(arg_idx);
                if arg_p.is_null() {
                    break;
                }
                let mut arg_len = 0;
                while *arg_p.add(arg_len) != 0 && arg_len < 256 {
                    arg_len += 1;
                }
                let arg_slice = core::slice::from_raw_parts(arg_p, arg_len);
                if let Ok(arg_str) = core::str::from_utf8(arg_slice) {
                    args_vec.push(alloc::string::String::from(arg_str));
                }
                arg_idx += 1;
            }
        }
    }

    if args_vec.is_empty() {
        args_vec.push(alloc::string::String::from(path));
    }

    let args_slices: alloc::vec::Vec<&str> = args_vec.iter().map(|s| s.as_str()).collect();

    match crate::task::elf::exec_elf_replace(path, &args_slices) {
        Ok(_) => 0,
        Err(e) => {
            crate::lunix_serial_println!("  [SYS_EXECVE] Failed: {}", e);
            -1
        }
    }
}

static THREAD_CLONE_MAP: Mutex<alloc::collections::BTreeMap<usize, crate::arch::x86_64::syscall::UserContext>> =
    Mutex::new(alloc::collections::BTreeMap::new());

fn clone_runner_trampoline() {
    let tid = crate::task::scheduler::current_tid();
    let ctx = {
        let lock = THREAD_CLONE_MAP.lock();
        lock.get(&tid).copied().expect("No clone context for child thread")
    };

    crate::lunix_serial_println!(
        "  [CLONE_RUNNER] TID {} transitioning child to Ring 3 at entry 0x{:X}, stack 0x{:X}, fs_base 0x{:X} with RAX=0...",
        tid, ctx.rip, ctx.rsp, ctx.fs_base
    );

    if ctx.fs_base != 0 {
        unsafe {
            crate::arch::x86_64::io::wrmsr(0xC000_0100, ctx.fs_base);
        }
    }
    unsafe {
        crate::task::user::enter_user_mode_full(&ctx);
    }
}

pub fn sys_vfork() -> isize {
    sys_clone(0x4111, 0)
}

pub fn sys_clone(flags: u64, stack: u64) -> isize {
    let parent_pid = crate::task::scheduler::current_pid();
    let parent_tid = crate::task::scheduler::current_tid();
    let child_pid = crate::task::scheduler::allocate_pid();
    let is_vfork = (flags & 0x4000) != 0;

    crate::lunix_serial_println!("  [SYS_CLONE] flags=0x{:X}, Parent PID {} (TID {}) spawning child PID {} (is_vfork={})...", flags, parent_pid, parent_tid, child_pid, is_vfork);

    if let Some(parent_proc) = crate::task::scheduler::get_process(parent_pid) {
        let mut child_proc = parent_proc.lock().clone_process(child_pid, is_vfork);
        if is_vfork {
            child_proc.vfork_waiting_parent = Some(parent_tid);
        }
        crate::task::scheduler::register_process(child_proc);
        parent_proc.lock().children.push(child_pid);
    }

    let mut ctx = *crate::arch::x86_64::syscall::CURRENT_USER_CONTEXT.lock();
    if stack != 0 {
        ctx.rsp = stack;
    }
    ctx.rax = 0; // Return 0 to child!

    let child_tid = crate::task::scheduler::spawn_with_pid("user_clone", child_pid, clone_runner_trampoline, 8);
    crate::task::scheduler::set_thread_fs_base(child_tid, ctx.fs_base);
    THREAD_CLONE_MAP.lock().insert(child_tid, ctx);

    if is_vfork {
        // Suspend calling parent thread until child calls execve or exit
        crate::task::scheduler::block_current_thread();
    }

    child_pid as isize
}

pub fn sys_wait4(pid: isize, status_ptr: *mut i32, options: i32) -> isize {
    let parent_pid = crate::task::scheduler::current_pid();
    let is_nohang = (options & 1) != 0; // WNOHANG = 1

    loop {
        if let Some((child_pid, exit_code)) = crate::task::scheduler::reap_child_process(parent_pid, pid) {
            if !status_ptr.is_null() {
                unsafe {
                    *status_ptr = (exit_code & 0xFF) << 8; // WEXITSTATUS format
                }
            }
            crate::lunix_serial_println!("  [SYS_WAIT4] Reaped child PID {} with exit_code {}", child_pid, exit_code);
            return child_pid as isize;
        }

        let has_children = if let Some(parent_arc) = crate::task::scheduler::get_process(parent_pid) {
            !parent_arc.lock().children.is_empty()
        } else {
            false
        };

        if !has_children {
            return -10; // -ECHILD
        }

        if is_nohang {
            return 0; // No child exited yet
        }

        crate::task::scheduler::sleep_ms(10);
    }
}

fn resolve_at_path(dfd: i32, path: &str) -> alloc::string::String {
    if path.starts_with('/') || dfd == -100 {
        alloc::string::String::from(path)
    } else {
        let mut full = alloc::string::String::from("/");
        full.push_str(path);
        full
    }
}

pub fn sys_openat(dfd: i32, filename_ptr: *const u8, flags: u32) -> isize {
    if filename_ptr.is_null() {
        return -14;
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
        Err(_) => return -14,
    };

    let resolved_path = resolve_at_path(dfd, path);
    crate::lunix_serial_println!("  [SYS_OPENAT] dfd={}, path='{}', flags=0x{:X}", dfd, resolved_path, flags);

    let is_creat = (flags & 0x40) != 0;
    let is_excl = (flags & 0x80) != 0;
    let is_trunc = (flags & 0x200) != 0;
    let is_append = (flags & 0x400) != 0;

    if let Some(proc_arc) = crate::task::scheduler::get_current_process() {
        let mut proc = proc_arc.lock();

        // 1. Check if already in IN_MEMORY_FILES
        {
            let mem_files = IN_MEMORY_FILES.lock();
            if let Some(buf_arc) = mem_files.get(&resolved_path).cloned() {
                if is_creat && is_excl {
                    return -17; // -EEXIST
                }
                if is_trunc {
                    buf_arc.lock().clear();
                }
                let off = if is_append { buf_arc.lock().len() } else { 0 };
                if let Some(fd) = proc.allocate_fd(crate::task::process::FileDescriptor {
                    target: FdTarget::File {
                        path: resolved_path.clone(),
                        offset: off,
                        buffer: buf_arc,
                    },
                    flags,
                }) {
                    return fd as isize;
                }
            }
        }

        // 2. Check Directory
        if let Ok(entries) = crate::fs::vfs::read_dir(&resolved_path) {
            if let Some(fd) = proc.allocate_fd(crate::task::process::FileDescriptor {
                target: FdTarget::Directory {
                    path: resolved_path.clone(),
                    entries,
                    current_idx: 0,
                },
                flags,
            }) {
                return fd as isize;
            }
        }

        // 3. Check VFS Handle
        if let Ok(handle) = crate::fs::vfs::open(&resolved_path) {
            if is_creat && is_excl {
                return -17; // -EEXIST
            }
            let mut hash = 0x811c9dc5u64;
            for b in resolved_path.as_bytes() {
                hash ^= *b as u64;
                hash = hash.wrapping_mul(0x1000193);
            }
            let ino = crate::fs::vfs::stat(&resolved_path).map(|i| if i.id != 0 { i.id } else { hash }).unwrap_or(hash);
            if let Some(fd) = proc.allocate_fd(crate::task::process::FileDescriptor {
                target: FdTarget::VfsHandle {
                    handle: alloc::sync::Arc::new(Mutex::new(handle)),
                    path: resolved_path.clone(),
                    inode_id: ino,
                },
                flags,
            }) {
                return fd as isize;
            }
        }

        // 4. Check VFS file data -> load into memory buffer
        if let Ok(data) = crate::fs::vfs::read_to_vec(&resolved_path) {
            if is_creat && is_excl {
                return -17; // -EEXIST
            }
            let buf_arc = alloc::sync::Arc::new(spin::Mutex::new(data));
            IN_MEMORY_FILES.lock().insert(resolved_path.clone(), buf_arc.clone());
            if is_trunc {
                buf_arc.lock().clear();
            }
            let off = if is_append { buf_arc.lock().len() } else { 0 };
            if let Some(fd) = proc.allocate_fd(crate::task::process::FileDescriptor {
                target: FdTarget::File {
                    path: resolved_path.clone(),
                    offset: off,
                    buffer: buf_arc,
                },
                flags,
            }) {
                return fd as isize;
            }
        }

        // 5. If not found and O_CREAT is requested, create new in-memory file
        if is_creat {
            let buf_arc = alloc::sync::Arc::new(spin::Mutex::new(alloc::vec::Vec::new()));
            IN_MEMORY_FILES.lock().insert(resolved_path.clone(), buf_arc.clone());
            if let Some(fd) = proc.allocate_fd(crate::task::process::FileDescriptor {
                target: FdTarget::File {
                    path: resolved_path.clone(),
                    offset: 0,
                    buffer: buf_arc,
                },
                flags,
            }) {
                crate::lunix_serial_println!("  [SYS_OPENAT_CREAT] Created dynamic in-memory file '{}' -> fd {}", resolved_path, fd);
                return fd as isize;
            }
        }
    }

    -2 // -ENOENT
}

pub fn sys_fstat(fd: usize, statbuf: *mut LinuxStat) -> isize {
    crate::lunix_serial_println!("  [SYS_FSTAT] fd={}", fd);
    if statbuf.is_null() {
        return -14; // -EFAULT
    }
    if let Some(proc_arc) = crate::task::scheduler::get_current_process() {
        let proc = proc_arc.lock();
        if let Some(desc_arc) = proc.get_fd(fd) {
            let desc = desc_arc.lock();
            let (size, is_dir, ino) = match desc.target {
                FdTarget::File { ref path, ref buffer, .. } => {
                    let size = buffer.lock().len() as i64;
                    let mut hash = 0x811c9dc5u64;
                    for b in path.as_bytes() {
                        hash ^= *b as u64;
                        hash = hash.wrapping_mul(0x1000193);
                    }
                    (size, false, hash)
                }
                FdTarget::VfsHandle { ref handle, inode_id, .. } => {
                    let h = handle.lock();
                    let s = h.size() as i64;
                    (s, false, inode_id)
                }
                FdTarget::Directory { ref path, .. } => {
                    let mut hash = 0x811c9dc5u64;
                    for b in path.as_bytes() {
                        hash ^= *b as u64;
                        hash = hash.wrapping_mul(0x1000193);
                    }
                    (4096, true, hash)
                }
                _ => (0, false, (fd as u64) + 2000),
            };

            unsafe {
                let st = &mut *statbuf;
                st.st_dev = 1;
                st.st_ino = ino;
                st.st_nlink = 1;
                st.st_mode = if is_dir { 0o040755 } else { 0o100755 };
                st.st_uid = 0;
                st.st_gid = 0;
                st.st_rdev = 0;
                st.st_size = size;
                st.st_blksize = 512;
                st.st_blocks = (size + 511) / 512;
                st.st_atime = 1726000000;
                st.st_mtime = 1726000000;
                st.st_ctime = 1726000000;
            }
            return 0;
        }
    }
    -9 // -EBADF
}

pub fn sys_fstatat(dfd: i32, filename_ptr: *const u8, statbuf: *mut LinuxStat, flags: u32) -> isize {
    if statbuf.is_null() {
        return -14;
    }
    if filename_ptr.is_null() {
        if dfd >= 0 {
            return sys_fstat(dfd as usize, statbuf);
        }
        return -14;
    }
    let mut len = 0;
    unsafe {
        while *filename_ptr.add(len) != 0 && len < 256 {
            len += 1;
        }
    }
    if len == 0 || (flags & 0x1000) != 0 {
        if dfd >= 0 {
            return sys_fstat(dfd as usize, statbuf);
        }
    }
    let slice = unsafe { core::slice::from_raw_parts(filename_ptr, len) };
    let path = match core::str::from_utf8(slice) {
        Ok(p) => p,
        Err(_) => return -2,
    };
    let resolved = resolve_at_path(dfd, path);

    // 1. Check IN_MEMORY_FILES
    if let Some(buf_arc) = IN_MEMORY_FILES.lock().get(&resolved).cloned() {
        let size = buf_arc.lock().len() as i64;
        let mut hash = 0x811c9dc5u64;
        for b in resolved.as_bytes() {
            hash ^= *b as u64;
            hash = hash.wrapping_mul(0x1000193);
        }
        unsafe {
            let st = &mut *statbuf;
            st.st_dev = 1;
            st.st_ino = hash;
            st.st_nlink = 1;
            st.st_mode = 0o100755;
            st.st_uid = 0;
            st.st_gid = 0;
            st.st_rdev = 0;
            st.st_size = size;
            st.st_blksize = 512;
            st.st_blocks = (size + 511) / 512;
            st.st_atime = 1726000000;
            st.st_mtime = 1726000000;
            st.st_ctime = 1726000000;
        }
        return 0;
    }

    if let Ok(inode) = crate::fs::vfs::stat(&resolved) {
        let mut hash = 0x811c9dc5u64;
        for b in resolved.as_bytes() {
            hash ^= *b as u64;
            hash = hash.wrapping_mul(0x1000193);
        }
        unsafe {
            let st = &mut *statbuf;
            st.st_dev = 1;
            st.st_ino = if inode.id != 0 { inode.id } else { hash };
            st.st_nlink = 1;
            st.st_mode = if inode.node_type == crate::fs::inode::INodeType::Directory {
                0o040755
            } else {
                0o100755
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
    -2 // -ENOENT
}

pub fn sys_readlink(path_ptr: *const u8, buf: *mut u8, bufsiz: usize) -> isize {
    if path_ptr.is_null() || buf.is_null() || bufsiz == 0 {
        return -14; // -EFAULT
    }
    let mut len = 0;
    unsafe {
        while *path_ptr.add(len) != 0 && len < 256 {
            len += 1;
        }
    }
    let slice = unsafe { core::slice::from_raw_parts(path_ptr, len) };
    if let Ok(path) = core::str::from_utf8(slice) {
        if path == "/proc/self/exe" || path.ends_with("/exe") {
            let exe_path = b"/bin/busybox";
            let copy_len = exe_path.len().min(bufsiz);
            unsafe {
                core::ptr::copy_nonoverlapping(exe_path.as_ptr(), buf, copy_len);
            }
            return copy_len as isize;
        }
    }
    -22 // -EINVAL (not a symlink)
}

pub fn sys_readlinkat(dfd: i32, filename_ptr: *const u8, buf: *mut u8, bufsiz: usize) -> isize {
    if filename_ptr.is_null() || buf.is_null() || bufsiz == 0 {
        return -14;
    }
    let mut len = 0;
    unsafe {
        while *filename_ptr.add(len) != 0 && len < 256 {
            len += 1;
        }
    }
    let slice = unsafe { core::slice::from_raw_parts(filename_ptr, len) };
    if let Ok(rel_path) = core::str::from_utf8(slice) {
        let resolved = resolve_at_path(dfd, rel_path);
        if resolved == "/proc/self/exe" || resolved.ends_with("/exe") {
            let exe_path = b"/bin/busybox";
            let copy_len = exe_path.len().min(bufsiz);
            unsafe {
                core::ptr::copy_nonoverlapping(exe_path.as_ptr(), buf, copy_len);
            }
            return copy_len as isize;
        }
    }
    -22 // -EINVAL
}

pub fn sys_access(path_ptr: *const u8, mode: u32) -> isize {
    sys_faccessat(-100, path_ptr, mode, 0)
}

pub fn sys_faccessat(dfd: i32, filename_ptr: *const u8, _mode: u32, _flags: u32) -> isize {
    if filename_ptr.is_null() {
        return -14;
    }
    let mut len = 0;
    unsafe {
        while *filename_ptr.add(len) != 0 && len < 256 {
            len += 1;
        }
    }
    let slice = unsafe { core::slice::from_raw_parts(filename_ptr, len) };
    if let Ok(rel_path) = core::str::from_utf8(slice) {
        let resolved = resolve_at_path(dfd, rel_path);
        if crate::fs::vfs::stat(&resolved).is_ok() || crate::fs::vfs::read_dir(&resolved).is_ok() {
            return 0;
        }
    }
    -2 // -ENOENT
}

pub fn sys_arch_prctl(code: u64, addr: u64) -> isize {
    crate::lunix_serial_println!("  [SYS_ARCH_PRCTL] code=0x{:X}, addr=0x{:X}", code, addr);
    match code {
        ARCH_SET_FS => {
            unsafe {
                crate::arch::x86_64::io::wrmsr(0xC000_0100, addr);
            }
            crate::task::scheduler::set_current_thread_fs_base(addr);
            0
        }
        ARCH_GET_FS => {
            if addr == 0 {
                return -14; // -EFAULT
            }
            unsafe {
                let fs_base = crate::arch::x86_64::io::rdmsr(0xC000_0100);
                *(addr as *mut u64) = fs_base;
            }
            0
        }
        ARCH_SET_GS => {
            unsafe {
                crate::arch::x86_64::io::wrmsr(0xC000_0101, addr);
            }
            0
        }
        ARCH_GET_GS => {
            if addr == 0 {
                return -14; // -EFAULT
            }
            unsafe {
                let gs_base = crate::arch::x86_64::io::rdmsr(0xC000_0101);
                *(addr as *mut u64) = gs_base;
            }
            0
        }
        _ => -22, // -EINVAL
    }
}

pub fn sys_clock_gettime(_clk_id: i32, tp: *mut LinuxTimeSpec) -> isize {
    if tp.is_null() {
        return -14;
    }
    let ticks = crate::drivers::timer::get_ticks();
    let secs = (ticks / 1000) as i64;
    let nsecs = ((ticks % 1000) * 1_000_000) as i64;
    unsafe {
        (*tp).tv_sec = secs;
        (*tp).tv_nsec = nsecs;
    }
    0
}

pub fn sys_gettimeofday(tv: *mut LinuxTimeVal) -> isize {
    if tv.is_null() {
        return -14;
    }
    let ticks = crate::drivers::timer::get_ticks();
    let secs = (ticks / 1000) as i64;
    let usecs = ((ticks % 1000) * 1000) as i64;
    unsafe {
        (*tv).tv_sec = secs;
        (*tv).tv_usec = usecs;
    }
    0
}

pub fn sys_getrlimit(_resource: u32, rlim: *mut LinuxRlimit) -> isize {
    if rlim.is_null() {
        return -14;
    }
    unsafe {
        (*rlim).rlim_cur = 8 * 1024 * 1024; // 8MB
        (*rlim).rlim_max = 64 * 1024 * 1024; // 64MB
    }
    0
}

pub fn sys_prlimit64(_pid: i32, _resource: u32, _new_rlim: *const LinuxRlimit, old_rlim: *mut LinuxRlimit) -> isize {
    if !old_rlim.is_null() {
        unsafe {
            (*old_rlim).rlim_cur = 8 * 1024 * 1024;
            (*old_rlim).rlim_max = 64 * 1024 * 1024;
        }
    }
    0
}

pub fn sys_statfs(buf: *mut LinuxStatFs) -> isize {
    if buf.is_null() {
        return -14;
    }
    unsafe {
        (*buf).f_type = 0x4D44; // FAT32 magic
        (*buf).f_bsize = 4096;
        (*buf).f_blocks = 131072;
        (*buf).f_bfree = 100000;
        (*buf).f_bavail = 100000;
        (*buf).f_files = 1024;
        (*buf).f_ffree = 900;
        (*buf).f_namelen = 255;
        (*buf).f_frsize = 4096;
        (*buf).f_flags = 0;
    }
    0
}

pub fn sys_getgroups(size: usize, list: *mut u32) -> isize {
    if size == 0 {
        return 1; // 1 group (root, gid=0)
    }
    if !list.is_null() {
        unsafe {
            *list = 0;
        }
    }
    1
}

pub fn sys_mkdir(path_ptr: *const u8, mode: u32) -> isize {
    sys_mkdirat(-100, path_ptr, mode)
}

pub fn sys_mkdirat(_dfd: i32, filename_ptr: *const u8, _mode: u32) -> isize {
    if filename_ptr.is_null() {
        return -14;
    }
    0 // Mkdir OK
}

pub fn sys_unlink(path_ptr: *const u8) -> isize {
    sys_unlinkat(-100, path_ptr, 0)
}

pub fn sys_unlinkat(dfd: i32, filename_ptr: *const u8, _flags: u32) -> isize {
    if filename_ptr.is_null() {
        return -14;
    }
    let mut len = 0;
    unsafe {
        while *filename_ptr.add(len) != 0 && len < 256 {
            len += 1;
        }
    }
    let slice = unsafe { core::slice::from_raw_parts(filename_ptr, len) };
    if let Ok(path) = core::str::from_utf8(slice) {
        let resolved = resolve_at_path(dfd, path);
        crate::lunix_serial_println!("  [SYS_UNLINK] unlinking '{}'", resolved);
        IN_MEMORY_FILES.lock().remove(&resolved);
    }
    0 // Unlink OK
}

pub fn sys_rmdir(_path_ptr: *const u8) -> isize {
    0 // Rmdir OK
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
