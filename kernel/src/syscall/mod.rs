use crate::task::process::FdTarget;
use crate::{lunix_print, lunix_println};
use core::slice;
use spin::Mutex;

<<<<<<< HEAD
pub mod epoll;
pub mod futex;
=======
>>>>>>> 67740566240f8bc3cf3fa1dbde7456fa0a3e6ea6
pub mod unix_socket;

pub static IN_MEMORY_FILES: spin::Mutex<alloc::collections::BTreeMap<alloc::string::String, alloc::sync::Arc<spin::Mutex<alloc::vec::Vec<u8>>>>> =
    spin::Mutex::new(alloc::collections::BTreeMap::new());

<<<<<<< HEAD
pub static SYMLINKS: spin::Mutex<alloc::collections::BTreeMap<alloc::string::String, alloc::string::String>> =
    spin::Mutex::new(alloc::collections::BTreeMap::new());

=======
>>>>>>> 67740566240f8bc3cf3fa1dbde7456fa0a3e6ea6
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
pub const LINUX_SYS_TRUNCATE: usize = 76;
pub const LINUX_SYS_FTRUNCATE: usize = 77;
pub const LINUX_SYS_GETCWD: usize = 79;
pub const LINUX_SYS_CHDIR: usize = 80;
pub const LINUX_SYS_RENAME: usize = 82;
pub const LINUX_SYS_MKDIR: usize = 83;
pub const LINUX_SYS_RMDIR: usize = 84;
pub const LINUX_SYS_LINK: usize = 86;
pub const LINUX_SYS_UNLINK: usize = 87;
pub const LINUX_SYS_SYMLINK: usize = 88;
pub const LINUX_SYS_READLINK: usize = 89;
pub const LINUX_SYS_CHMOD: usize = 90;
pub const LINUX_SYS_FCHMOD: usize = 91;
pub const LINUX_SYS_CHOWN: usize = 92;
pub const LINUX_SYS_FCHOWN: usize = 93;
pub const LINUX_SYS_UMASK: usize = 95;
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
pub const LINUX_SYS_SETREUID: usize = 113;
pub const LINUX_SYS_SETREGID: usize = 114;
pub const LINUX_SYS_GETGROUPS: usize = 115;
pub const LINUX_SYS_SETGROUPS: usize = 116;
pub const LINUX_SYS_SETRESUID: usize = 117;
pub const LINUX_SYS_GETRESUID: usize = 118;
pub const LINUX_SYS_SETRESGID: usize = 119;
pub const LINUX_SYS_GETRESGID: usize = 120;
pub const LINUX_SYS_GETPGID: usize = 121;
pub const LINUX_SYS_CAPGET: usize = 125;
pub const LINUX_SYS_CAPSET: usize = 126;
pub const LINUX_SYS_MKNOD: usize = 133;
pub const LINUX_SYS_STATFS: usize = 137;
pub const LINUX_SYS_FSTATFS: usize = 138;
pub const LINUX_SYS_ARCH_PRCTL: usize = 158;
pub const LINUX_SYS_SETRLIMIT: usize = 160;
pub const LINUX_SYS_MOUNT: usize = 165;
pub const LINUX_SYS_UMOUNT2: usize = 166;
pub const LINUX_SYS_REBOOT: usize = 169;
pub const LINUX_SYS_GETTID: usize = 186;
pub const LINUX_SYS_FUTEX: usize = 202;
pub const LINUX_SYS_EPOLL_CREATE: usize = 213;
pub const LINUX_SYS_GETDENTS64: usize = 217;
pub const LINUX_SYS_SET_TID_ADDRESS: usize = 218;
pub const LINUX_SYS_CLOCK_GETTIME: usize = 228;
pub const LINUX_SYS_CLOCK_NANOSLEEP: usize = 230;
pub const LINUX_SYS_EXIT_GROUP: usize = 231;
pub const LINUX_SYS_EPOLL_WAIT: usize = 232;
pub const LINUX_SYS_EPOLL_CTL: usize = 233;
pub const LINUX_SYS_EPOLL_PWAIT: usize = 281;
pub const LINUX_SYS_EVENTFD: usize = 284;
pub const LINUX_SYS_TGKILL: usize = 234;
pub const LINUX_SYS_OPENAT: usize = 257;
pub const LINUX_SYS_MKDIRAT: usize = 258;
pub const LINUX_SYS_MKNODAT: usize = 259;
pub const LINUX_SYS_FSTATAT: usize = 262;
pub const LINUX_SYS_UNLINKAT: usize = 263;
pub const LINUX_SYS_LINKAT: usize = 265;
pub const LINUX_SYS_SYMLINKAT: usize = 266;
pub const LINUX_SYS_READLINKAT: usize = 267;
pub const LINUX_SYS_FACCESSAT: usize = 269;
pub const LINUX_SYS_SET_ROBUST_LIST: usize = 273;
pub const LINUX_SYS_GET_ROBUST_LIST: usize = 274;
<<<<<<< HEAD
pub const LINUX_SYS_TIMERFD_CREATE: usize = 283;
pub const LINUX_SYS_FALLOCATE: usize = 285;
pub const LINUX_SYS_TIMERFD_SETTIME: usize = 286;
pub const LINUX_SYS_TIMERFD_GETTIME: usize = 287;
pub const LINUX_SYS_ACCEPT4: usize = 288;
pub const LINUX_SYS_SIGNALFD4: usize = 289;
pub const LINUX_SYS_EVENTFD2: usize = 290;
pub const LINUX_SYS_EPOLL_CREATE1: usize = 291;
=======
pub const LINUX_SYS_ACCEPT4: usize = 288;
>>>>>>> 67740566240f8bc3cf3fa1dbde7456fa0a3e6ea6
pub const LINUX_SYS_DUP3: usize = 292;
pub const LINUX_SYS_PIPE2: usize = 293;
pub const LINUX_SYS_PRLIMIT64: usize = 302;
pub const LINUX_SYS_GETRANDOM: usize = 318;
pub const LINUX_SYS_RSEQ: usize = 334;
pub const LINUX_SYS_SENDMSG: usize = 46;
pub const LINUX_SYS_RECVMSG: usize = 47;
pub const LINUX_SYS_SHUTDOWN: usize = 48;
pub const LINUX_SYS_SETITIMER: usize = 38;
pub const LINUX_SYS_PRCTL: usize = 157;
pub const LINUX_SYS_SETFSUID: usize = 122;
pub const LINUX_SYS_SETFSGID: usize = 123;
pub const LINUX_SYS_FADVISE64: usize = 221;
pub const LINUX_SYS_PSELECT6: usize = 270;
pub const LINUX_SYS_PPOLL: usize = 271;
pub const LINUX_SYS_FACCESSAT2: usize = 439;
pub const LINUX_SYS_TIME: usize = 201;
pub const LINUX_SYS_STATX: usize = 332;

// Linux arch_prctl codes
pub const ARCH_SET_GS: u64 = 0x1001;
pub const ARCH_SET_FS: u64 = 0x1002;
pub const ARCH_GET_FS: u64 = 0x1003;
pub const ARCH_GET_GS: u64 = 0x1004;

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
    crate::lunix_strace!("  [SYSCALL_DISPATCH] num=0x{:X}, a1=0x{:X}, a2=0x{:X}, a3=0x{:X}", num, arg1, arg2, arg3);
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
        LINUX_SYS_SELECT => sys_select(arg1 as usize, arg2 as *mut u64, arg3 as *mut u64, arg4 as *mut u64, arg5 as *const LinuxTimeVal) as u64,
        LINUX_SYS_PSELECT6 => sys_pselect6(arg1 as usize, arg2 as *mut u64, arg3 as *mut u64, arg4 as *mut u64, arg5 as *const LinuxTimeSpec) as u64,
        LINUX_SYS_PPOLL => sys_ppoll(arg1 as *mut LinuxPollFd, arg2 as usize, arg3 as *const LinuxTimeSpec) as u64,
        LINUX_SYS_FACCESSAT2 => sys_faccessat(arg1 as i32, arg2 as *const u8, arg3 as u32, arg4 as u32) as u64,
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
        LINUX_SYS_RECVFROM => sys_recvfrom_flags(arg1 as usize, arg2 as *mut u8, arg3 as usize, arg4 as i32) as u64,
        LINUX_SYS_BIND => sys_bind(arg1 as usize, arg2 as *const u8, arg3 as usize) as u64,
        LINUX_SYS_LISTEN => sys_listen(arg1 as usize, arg2 as usize) as u64,
        LINUX_SYS_GETSOCKNAME | LINUX_SYS_GETPEERNAME => sys_getsockname(arg1 as usize, arg2 as *mut u8, arg3 as *mut u32) as u64,
        LINUX_SYS_SOCKETPAIR => sys_socketpair(arg1 as i32, arg2 as i32, arg3 as i32, arg4 as *mut [i32; 2]) as u64,
        LINUX_SYS_SETSOCKOPT => sys_setsockopt(arg1 as usize, arg2 as i32, arg3 as i32, arg4 as *const u8, arg5 as u32) as u64,
        LINUX_SYS_GETSOCKOPT => sys_getsockopt(arg1 as usize, arg2 as i32, arg3 as i32, arg4 as *mut u8, arg5 as *mut u32) as u64,
<<<<<<< HEAD
        LINUX_SYS_CLONE => sys_clone(arg1, arg2, arg3, arg4, arg5) as u64,
        LINUX_SYS_FORK => sys_clone(0, 0, 0, 0, 0) as u64,
=======
        LINUX_SYS_CLONE => sys_clone(arg1, arg2) as u64,
        LINUX_SYS_FORK => sys_clone(0, 0) as u64,
>>>>>>> 67740566240f8bc3cf3fa1dbde7456fa0a3e6ea6
        LINUX_SYS_VFORK => sys_vfork() as u64,
        LINUX_SYS_EXECVE => sys_execve(arg1 as *const u8, arg2 as *const *const u8, arg3 as *const *const u8) as u64,
        LINUX_SYS_EXIT => sys_exit_thread(arg1 as i32),
        LINUX_SYS_WAIT4 => sys_wait4(arg1 as isize, arg2 as *mut i32, arg3 as i32) as u64,
        LINUX_SYS_KILL => sys_kill(arg1 as isize, arg2 as i32) as u64,
        LINUX_SYS_UNAME => sys_uname(arg1 as *mut LinuxUtsName) as u64,
        LINUX_SYS_FCNTL => sys_fcntl(arg1 as usize, arg2 as usize, arg3) as u64,
        LINUX_SYS_TRUNCATE => sys_truncate(arg1 as *const u8, arg2 as usize) as u64,
        LINUX_SYS_FTRUNCATE => sys_ftruncate(arg1 as usize, arg2 as usize) as u64,
        LINUX_SYS_GETCWD => sys_getcwd(arg1 as *mut u8, arg2 as usize) as u64,
        LINUX_SYS_CHDIR => sys_chdir(arg1 as *const u8) as u64,
        LINUX_SYS_RENAME => sys_rename(arg1 as *const u8, arg2 as *const u8) as u64,
        LINUX_SYS_MKDIR => sys_mkdir(arg1 as *const u8, arg2 as u32) as u64,
        LINUX_SYS_RMDIR => sys_rmdir(arg1 as *const u8) as u64,
        LINUX_SYS_LINK => sys_link(arg1 as *const u8, arg2 as *const u8) as u64,
        LINUX_SYS_UNLINK => sys_unlink(arg1 as *const u8) as u64,
        LINUX_SYS_SYMLINK => sys_symlink(arg1 as *const u8, arg2 as *const u8) as u64,
        LINUX_SYS_READLINK => sys_readlink(arg1 as *const u8, arg2 as *mut u8, arg3 as usize) as u64,
        LINUX_SYS_CHMOD | LINUX_SYS_FCHMOD => 0,
        LINUX_SYS_CHOWN | LINUX_SYS_FCHOWN => 0,
        LINUX_SYS_UMASK => sys_umask(arg1 as u32) as u64,
        LINUX_SYS_GETTIMEOFDAY => sys_gettimeofday(arg1 as *mut LinuxTimeVal) as u64,
        LINUX_SYS_GETRLIMIT => sys_getrlimit(arg1 as u32, arg2 as *mut LinuxRlimit) as u64,
        LINUX_SYS_SYSINFO => sys_sysinfo(arg1 as *mut LinuxSysInfo) as u64,
        LINUX_SYS_GETUID => sys_getuid() as u64,
        LINUX_SYS_GETEUID => sys_geteuid() as u64,
        LINUX_SYS_GETGID => sys_getgid() as u64,
        LINUX_SYS_GETEGID => sys_getegid() as u64,
        // Supplementary groups are not tracked (everything runs as root): accept the call.
        LINUX_SYS_SETGROUPS => 0,
        LINUX_SYS_SETUID => sys_setuid(arg1 as u32) as u64,
        LINUX_SYS_SETGID => sys_setgid(arg1 as u32) as u64,
        LINUX_SYS_SETPGID => 0,
        LINUX_SYS_GETPPID => sys_getppid() as u64,
        LINUX_SYS_GETPGRP | LINUX_SYS_GETPGID | LINUX_SYS_SETSID => sys_getpid() as u64,
        LINUX_SYS_SETREUID => sys_setreuid(arg1 as u32, arg2 as u32) as u64,
        LINUX_SYS_SETREGID => sys_setregid(arg1 as u32, arg2 as u32) as u64,
        LINUX_SYS_GETGROUPS => sys_getgroups(arg1 as usize, arg2 as *mut u32) as u64,
        LINUX_SYS_SETRESUID => sys_setresuid(arg1 as u32, arg2 as u32, arg3 as u32) as u64,
        LINUX_SYS_GETRESUID => sys_getresuid(arg1 as *mut u32, arg2 as *mut u32, arg3 as *mut u32) as u64,
        LINUX_SYS_SETRESGID => sys_setresgid(arg1 as u32, arg2 as u32, arg3 as u32) as u64,
        LINUX_SYS_GETRESGID => sys_getresgid(arg1 as *mut u32, arg2 as *mut u32, arg3 as *mut u32) as u64,
        LINUX_SYS_CAPGET | LINUX_SYS_CAPSET => 0,
        LINUX_SYS_MKNOD => 0,
        LINUX_SYS_STATFS | LINUX_SYS_FSTATFS => sys_statfs(arg2 as *mut LinuxStatFs) as u64,
        LINUX_SYS_ARCH_PRCTL => sys_arch_prctl(arg1, arg2) as u64,
        LINUX_SYS_SETRLIMIT => 0,
        LINUX_SYS_MOUNT => sys_mount(arg1 as *const u8, arg2 as *const u8, arg3 as *const u8, arg4, arg5 as *const u8) as u64,
        LINUX_SYS_UMOUNT2 => sys_umount2(arg1 as *const u8, arg2 as i32) as u64,
        LINUX_SYS_REBOOT => 0,
        LINUX_SYS_GETTID => crate::task::scheduler::current_tid() as u64,
        LINUX_SYS_FUTEX => futex::sys_futex(arg1, arg2 as i32, arg3 as u32, arg4, arg5, arg6 as u32) as u64,
        LINUX_SYS_EPOLL_CREATE => epoll::sys_epoll_create1(0) as u64,
        LINUX_SYS_EPOLL_CREATE1 => epoll::sys_epoll_create1(arg1 as i32) as u64,
        LINUX_SYS_GETDENTS64 => sys_getdents64(arg1 as usize, arg2 as *mut u8, arg3 as usize) as u64,
        LINUX_SYS_SET_TID_ADDRESS => sys_set_tid_address(arg1 as *mut i32) as u64,
        LINUX_SYS_CLOCK_GETTIME => sys_clock_gettime(arg1 as i32, arg2 as *mut LinuxTimeSpec) as u64,
<<<<<<< HEAD
        229 => { // clock_getres: the timer ticks at 1000 Hz
            let tp = arg2 as *mut LinuxTimeSpec;
            if !tp.is_null() {
                unsafe { (*tp).tv_sec = 0; (*tp).tv_nsec = 1_000_000; }
            }
            0
        }
        // utimensat: timestamps are not stored (the root filesystem is read-only); accepted so
        // programs that restore mtimes (fontconfig, xkbcomp) do not warn.
        280 => 0,
        37 => 0,  // alarm: no SIGALRM delivery yet; no previous alarm pending
        141 => 0, // setpriority: a single priority class, accepted
=======
>>>>>>> 67740566240f8bc3cf3fa1dbde7456fa0a3e6ea6
        LINUX_SYS_CLOCK_NANOSLEEP => sys_clock_nanosleep(arg1 as i32, arg2 as i32, arg3 as *const LinuxTimeSpec, arg4 as *mut LinuxTimeSpec) as u64,
        LINUX_SYS_EXIT_GROUP => sys_exit(arg1 as i32),
        LINUX_SYS_EPOLL_WAIT | LINUX_SYS_EPOLL_PWAIT => epoll::sys_epoll_wait(arg1 as usize, arg2 as *mut epoll::EpollEvent, arg3 as i32, arg4 as i32) as u64,
        LINUX_SYS_EPOLL_CTL => epoll::sys_epoll_ctl(arg1 as usize, arg2 as i32, arg3 as i32, arg4 as *const epoll::EpollEvent) as u64,
        LINUX_SYS_TGKILL => sys_tgkill(arg1 as usize, arg2 as usize, arg3 as i32) as u64,
        200 => sys_tgkill(0, arg1 as usize, arg2 as i32) as u64, // tkill
        LINUX_SYS_OPENAT => sys_openat(arg1 as i32, arg2 as *const u8, arg3 as u32) as u64,
        LINUX_SYS_MKDIRAT => sys_mkdirat(arg1 as i32, arg2 as *const u8, arg3 as u32) as u64,
        LINUX_SYS_MKNODAT => 0,
        LINUX_SYS_FSTATAT => sys_fstatat(arg1 as i32, arg2 as *const u8, arg3 as *mut LinuxStat, arg4 as u32) as u64,
        LINUX_SYS_UNLINKAT => sys_unlinkat(arg1 as i32, arg2 as *const u8, arg3 as u32) as u64,
        LINUX_SYS_LINKAT => sys_linkat(arg1 as i32, arg2 as *const u8, arg3 as i32, arg4 as *const u8, arg5 as i32) as u64,
        LINUX_SYS_SYMLINKAT => sys_symlinkat(arg1 as *const u8, arg2 as i32, arg3 as *const u8) as u64,
        LINUX_SYS_READLINKAT => sys_readlinkat(arg1 as i32, arg2 as *const u8, arg3 as *mut u8, arg4 as usize) as u64,
        LINUX_SYS_FACCESSAT => sys_faccessat(arg1 as i32, arg2 as *const u8, arg3 as u32, arg4 as u32) as u64,
        LINUX_SYS_SET_ROBUST_LIST => sys_set_robust_list(arg1, arg2 as usize) as u64,
        LINUX_SYS_GET_ROBUST_LIST => sys_get_robust_list(arg1 as i32, arg2 as *mut u64, arg3 as *mut usize) as u64,
        // timerfd/signalfd are not implemented: say so, so programs use their fallbacks.
        LINUX_SYS_TIMERFD_CREATE | LINUX_SYS_SIGNALFD4 | LINUX_SYS_TIMERFD_SETTIME | LINUX_SYS_TIMERFD_GETTIME => (-38isize) as u64,
        LINUX_SYS_FALLOCATE => 0,
        LINUX_SYS_EVENTFD2 => epoll::sys_eventfd2(arg1 as u32, arg2 as i32) as u64,
        LINUX_SYS_EVENTFD => epoll::sys_eventfd2(arg1 as u32, 0) as u64,
        LINUX_SYS_DUP3 => sys_dup3(arg1 as usize, arg2 as usize, arg3 as i32) as u64,
        LINUX_SYS_PIPE2 => sys_pipe2(arg1 as *mut [i32; 2], arg2 as i32) as u64,
        LINUX_SYS_PRLIMIT64 => sys_prlimit64(arg1 as i32, arg2 as u32, arg3 as *const LinuxRlimit, arg4 as *mut LinuxRlimit) as u64,
        LINUX_SYS_GETRANDOM => sys_getrandom(arg1 as *mut u8, arg2 as usize, arg3 as u32) as u64,
        LINUX_SYS_RSEQ => 0,
        LINUX_SYS_SENDMSG => sys_sendmsg(arg1 as usize, arg2 as *const LinuxMsgHdr, arg3 as i32) as u64,
        LINUX_SYS_RECVMSG => sys_recvmsg(arg1 as usize, arg2 as *mut LinuxMsgHdr, arg3 as i32) as u64,
        // shutdown(2)/setitimer(2): accepted; there is no half-close or interval timer yet.
        LINUX_SYS_SHUTDOWN | LINUX_SYS_SETITIMER => 0,
        LINUX_SYS_TIME => {
            // Same uptime-based clock as gettimeofday (no RTC driver yet).
            let secs = crate::drivers::timer::get_ticks() / 1000;
            if arg1 != 0 {
                unsafe { *(arg1 as *mut i64) = secs as i64; }
            }
            secs
        }
        // No per-task fs credentials: report uid/gid 0 (root) as the previous value.
        LINUX_SYS_SETFSUID | LINUX_SYS_SETFSGID | LINUX_SYS_FADVISE64 => 0,
        LINUX_SYS_PRCTL => sys_prctl(arg1, arg2, arg3) as u64,
        LINUX_SYS_STATX => sys_statx(arg1 as i32, arg2 as *const u8, arg3 as u32, arg4 as u32, arg5 as *mut LinuxStatx) as u64,

        _ => {
            crate::lunix_serial_println!("[SYSCALL] Unimplemented syscall number: {}", num);
            (-38isize) as u64 // -ENOSYS: lets glibc fall back to older syscalls
        }
    };
    crate::lunix_strace!("  [SYSCALL_DISPATCH_RET] num=0x{:X} -> 0x{:X}", num, ret);
    ret
}

/// Address space (CR3) of the program that mapped /dev/fb0, or 0.
static FB_OWNER_CR3: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

/// Signals whose default action is to terminate the process (no handlers are installed by
/// the kernel: signal delivery to user handlers is not implemented). Signals that are
/// ignored or only stop/continue by default do nothing.
fn signal_terminates(sig: i32) -> bool {
    !matches!(sig, 0 | 17 | 18 | 19 | 20 | 21 | 22 | 23 | 28)
}

/// Terminate another process because of a fatal signal.
fn terminate_process(pid: usize, sig: i32) {
    let Some(p) = crate::task::scheduler::get_process(pid) else { return };
    crate::task::scheduler::kill_other_threads(pid, usize::MAX);
    {
        let mut proc = p.lock();
        proc.close_all_fds();
        if let Some(parent_tid) = proc.vfork_waiting_parent.take() {
            drop(proc);
            crate::task::scheduler::unblock_thread(parent_tid);
        }
    }
    crate::task::scheduler::set_process_exit_code(pid, 128 + sig);
    release_address_space(pid);
}

/// `kill(2)`.
pub fn sys_kill(pid: isize, sig: i32) -> isize {
    if !(0..=64).contains(&sig) {
        return -22; // -EINVAL
    }
    let me = crate::task::scheduler::current_pid();
    let target = if pid > 0 { pid as usize } else if pid == 0 || pid == -1 { return 0 } else { (-pid) as usize };
    let Some(proc) = crate::task::scheduler::get_process(target) else { return -3 }; // -ESRCH
    if !proc.lock().is_alive {
        return -3;
    }
    if sig == 0 || !signal_terminates(sig) {
        return 0;
    }
    if target == me {
        sys_exit(128 + sig);
    }
    if target > 1 {
        terminate_process(target, sig);
    }
    0
}

/// `tgkill(2)` / `tkill(2)`: signals aimed at the calling thread (raise/abort) take effect
/// on the process; signals to other threads are ignored.
pub fn sys_tgkill(_tgid: usize, tid: usize, sig: i32) -> isize {
    if sig == 0 || !signal_terminates(sig) {
        return 0;
    }
    if tid == crate::task::scheduler::current_tid() {
        sys_exit(128 + sig);
    }
    0
}

/// `exit(2)`: ends the calling thread. The process lives on while other threads run.
pub fn sys_exit_thread(code: i32) -> ! {
    let pid = crate::task::scheduler::current_pid();
    if crate::task::scheduler::live_thread_count(pid) > 1 {
        let ctid = crate::task::scheduler::take_current_clear_child_tid();
        if ctid != 0 {
            // pthread_join waits on this word; 0 = the thread is gone.
            unsafe { *(ctid as *mut i32) = 0 };
            futex::wake(ctid, 1);
        }
        crate::task::scheduler::exit_current_thread();
    }
    sys_exit(code)
}

/// Give a dying process's memory back (unless a `vfork` parent still shares it).
fn release_address_space(pid: usize) {
    let Some(p) = crate::task::scheduler::get_process(pid) else { return };
    let cr3 = p.lock().cr3;
    let (kernel_cr3, _) = x86_64::registers::control::Cr3::read();
    if cr3 == 0 || crate::task::scheduler::address_space_shared(cr3, pid) {
        return;
    }
    let _ = kernel_cr3;
    crate::mm::vmm::free_user_frames(x86_64::PhysAddr::new(cr3));
}

/// `exit_group(2)` / last-thread exit: ends the whole process.
pub fn sys_exit(code: i32) -> ! {
    let pid = crate::task::scheduler::current_pid();
    crate::task::scheduler::kill_other_threads(pid, crate::task::scheduler::current_tid());
    // The framebuffer owner is gone: give the screen back to the text console.
    if let Some(p) = crate::task::scheduler::get_current_process() {
        let cr3 = p.lock().cr3;
        if cr3 != 0 && FB_OWNER_CR3.load(core::sync::atomic::Ordering::SeqCst) == cr3 {
            FB_OWNER_CR3.store(0, core::sync::atomic::Ordering::SeqCst);
            crate::display::console::set_graphics_mode(false);
        }
    }
    if let Some(proc_arc) = crate::task::scheduler::get_current_process() {
        let mut proc = proc_arc.lock();
        if let Some(parent_tid) = proc.vfork_waiting_parent.take() {
            crate::task::scheduler::unblock_thread(parent_tid);
        }
    }
    if let Some(p) = crate::task::scheduler::get_current_process() {
        // A dying process closes all its descriptors, so pipe readers get EOF, sockets hang up.
        p.lock().close_all_fds();
    }
    crate::task::scheduler::set_process_exit_code(pid, code);
    release_address_space(pid);
    crate::lunix_strace!("[exit] pid {} status {}", pid, code);
    if pid == 1 {
        // Linux panics when init dies; we drop to the kernel debug console instead.
        crate::drivers::keyboard::set_userspace_tty(false);
        lunix_println!("\n[-] init (PID 1) exited with status {}. Kernel debug console active.", code);
    }
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
    crate::lunix_strace!("  [SYS_WRITE] fd={}, len={}", fd, len);
    if crate::arch::x86_64::serial::TRACE && !buf.is_null() && len > 0 && fd == 2 {
        // stderr text is what explains most failures: show the start of it in traces.
        let shown = unsafe { core::slice::from_raw_parts(buf, len.min(120)) };
        crate::lunix_strace!("    stderr: {}", alloc::string::String::from_utf8_lossy(shown));
    }
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
            FdTarget::EventFd(counter) => {
                let slice = unsafe { slice::from_raw_parts(buf, len) };
                return epoll::eventfd_write(&counter, slice);
            }
            FdTarget::PipeWrite(pipe) => {
                let non_blocking = (flags & 0x800) != 0;
                let slice = unsafe { slice::from_raw_parts(buf, len) };
                // A blocking write completes fully: it waits for the reader to make room.
                let mut written = 0usize;
                loop {
                    let write_res = {
                        pipe.lock().write(&slice[written..], non_blocking)
                    };
                    match write_res {
                        Ok(n) => {
                            written += n;
                            if written >= slice.len() || non_blocking {
                                return written as isize;
                            }
                        }
                        Err(crate::task::pipe::PipeError::WouldBlock)
                        | Err(crate::task::pipe::PipeError::BufferFull) => {
                            if non_blocking {
                                return if written > 0 { written as isize } else { -11 }; // -EAGAIN
                            }
                            crate::task::scheduler::sleep_ms(1);
                        }
                        Err(crate::task::pipe::PipeError::BrokenPipe) => {
                            return if written > 0 { written as isize } else { -32 }; // -EPIPE
                        }
                    }
                }
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
    crate::lunix_strace!("  [SYS_READ] fd={}, len={}", fd, len);
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
                return crate::drivers::tty::read(slice, (flags & 0x800) != 0);
            }
            FdTarget::EventFd(counter) => {
                let slice = unsafe { slice::from_raw_parts_mut(buf, len) };
                return epoll::eventfd_read(&counter, slice, (flags & 0x800) != 0);
            }
            FdTarget::PipeRead(pipe) => {
                let non_blocking = (flags & 0x800) != 0;
                let slice = unsafe { slice::from_raw_parts_mut(buf, len) };
                // A blocking read waits until data arrives or every writer has closed (EOF).
                loop {
                    let read_res = {
                        pipe.lock().read(slice, non_blocking)
                    };
                    match read_res {
                        Ok(n) => return n as isize,
                        Err(crate::task::pipe::PipeError::WouldBlock) => {
                            if non_blocking {
                                return -11; // -EAGAIN
                            }
                            crate::task::scheduler::sleep_ms(1);
                        }
                        Err(crate::task::pipe::PipeError::BrokenPipe) => return 0, // EOF
                        Err(crate::task::pipe::PipeError::BufferFull) => return 0,
                    }
                }
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
                let nonblock = (flags & 0x800) != 0;
                // Blocking waits happen here, outside the handle lock, so that poll/epoll
                // on the same file can still check readiness meanwhile.
                loop {
                    let r = handle.lock().read_nonblock(slice);
                    match r {
                        Ok(n) => return n as isize,
                        Err("EAGAIN") => {
                            if nonblock {
                                return -11;
                            }
                            crate::task::scheduler::sleep_ms(1);
                        }
                        Err("EINVAL") => return -22,
                        Err(_) => return -1,
                    }
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
    crate::lunix_strace!("  [SYS_LSEEK] fd={}, offset={}, whence={}", fd, offset, whence);
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
    crate::lunix_strace!("  [SYS_PREAD64] fd={}, count={}, pos={}", fd, count, pos);
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
    crate::lunix_strace!("  [SYS_WRITEV] fd={}, iovcnt={}", fd, iovcnt);
    if iov_ptr.is_null() || iovcnt == 0 {
        return 0;
    }
    let mut total_written = 0isize;
    let iovs = unsafe { core::slice::from_raw_parts(iov_ptr, iovcnt) };
    for iov in iovs {
        if !iov.iov_base.is_null() && iov.iov_len > 0 {
            let written = sys_write(fd, iov.iov_base, iov.iov_len);
            if written < 0 {
                // Report the error only if nothing was written yet (POSIX).
                return if total_written > 0 { total_written } else { written };
            }
            total_written += written;
            if (written as usize) < iov.iov_len {
                break; // short write
            }
        }
    }
    total_written
}

pub fn sys_readv(fd: usize, iov_ptr: *const LinuxIoVec, iovcnt: usize) -> isize {
    crate::lunix_strace!("  [SYS_READV] fd={}, iovcnt={}", fd, iovcnt);
    if iov_ptr.is_null() || iovcnt == 0 {
        return 0;
    }
    let mut total_read = 0isize;
    let iovs = unsafe { core::slice::from_raw_parts(iov_ptr, iovcnt) };
    for iov in iovs {
        if !iov.iov_base.is_null() && iov.iov_len > 0 {
            let read_bytes = sys_read(fd, iov.iov_base as *mut u8, iov.iov_len);
            if read_bytes < 0 {
                // Report the error only if nothing was read yet (e.g. -EAGAIN).
                return if total_read > 0 { total_read } else { read_bytes };
            }
            total_read += read_bytes;
            if (read_bytes as usize) < iov.iov_len {
                break; // short read: do not block for the next buffer
            }
        }
    }
    total_read
}

/// `struct msghdr`.
#[repr(C)]
pub struct LinuxMsgHdr {
    pub msg_name: *mut u8,
    pub msg_namelen: u32,
    pub msg_iov: *const LinuxIoVec,
    pub msg_iovlen: usize,
    pub msg_control: *mut u8,
    pub msg_controllen: usize,
    pub msg_flags: i32,
}

/// sendmsg(2): the iovec is written like `writev`. Ancillary data (fd passing) is not
/// supported and is ignored.
pub fn sys_sendmsg(fd: usize, msg: *const LinuxMsgHdr, _flags: i32) -> isize {
    if msg.is_null() {
        return -14;
    }
    let m = unsafe { &*msg };
    sys_writev(fd, m.msg_iov, m.msg_iovlen)
}

/// recvmsg(2): the iovec is filled like `readv`; no ancillary data is returned.
pub fn sys_recvmsg(fd: usize, msg: *mut LinuxMsgHdr, flags: i32) -> isize {
    if msg.is_null() {
        return -14;
    }
    let m = unsafe { &mut *msg };
    // MSG_DONTWAIT (0x40) makes just this call non-blocking, whatever the fd flags say.
    let dontwait = (flags & 0x40) != 0;
    if dontwait && !fd_has_data(fd) {
        return -11; // -EAGAIN
    }
    let n = sys_readv(fd, m.msg_iov, m.msg_iovlen);
    m.msg_controllen = 0;
    m.msg_flags = 0;
    m.msg_namelen = 0;
    n
}

/// Readiness for the DONTWAIT check above.
fn fd_has_data(fd: usize) -> bool {
    fd_poll_revents(fd, POLLIN) & (POLLIN | POLLHUP | POLLERR) != 0
}

pub fn sys_open(path_ptr: *const u8, flags: u32, _mode: u32) -> isize {
    sys_openat(-100, path_ptr, flags)
}

pub fn sys_close(fd: usize) -> isize {
    crate::lunix_strace!("  [SYS_CLOSE] fd={}", fd);
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
                            // A program now draws straight onto the framebuffer (X): the text
                            // console must stop painting over it until that program exits.
                            FB_OWNER_CR3.store(
                                x86_64::registers::control::Cr3::read().0.start_address().as_u64(),
                                core::sync::atomic::Ordering::SeqCst,
                            );
                            crate::display::console::set_graphics_mode(true);
                            return target_addr;
                        }
                    }
                }
            }
        }
    }

    // Demand-paged mapping: anonymous memory and file-backed mappings of VFS files are only
    // recorded here; pages are allocated (and filled from the file) on first access.
    let backing = if fd >= 0 {
        crate::task::scheduler::get_current_process().and_then(|proc_arc| {
            let proc = proc_arc.lock();
            let desc_arc = proc.get_fd(fd as usize)?;
            let desc = desc_arc.lock();
            match &desc.target {
                FdTarget::VfsHandle { handle, .. } => Some(Some((handle.clone(), offset))),
                _ => None, // in-memory files etc.: eager path below
            }
        })
    } else {
        Some(None)
    };
    if let Some(file) = backing {
        crate::mm::vma::map(target_addr, length, file);
        return target_addr;
    }

    for p in 0..pages {
        let page_vaddr = x86_64::VirtAddr::new(target_addr + (p * 4096));
        if !crate::mm::vmm::is_page_mapped(page_vaddr) {
            if let Some(frame) = crate::mm::pmm::alloc_frame() {
                if let Err(e) = crate::mm::vmm::map_page(page_vaddr, frame, map_flags) {
                    crate::lunix_strace!("  [SYS_MMAP_ERR] failed to map page p={} (virt 0x{:X}): {}", p, page_vaddr.as_u64(), e);
                } else {
                    unsafe {
                        core::ptr::write_bytes(page_vaddr.as_mut_ptr::<u8>(), 0, 4096);
                    }
                }
            } else {
                crate::lunix_strace!("  [SYS_MMAP_ERR] PMM out of frames for page p={}", p);
            }
        }
        if p == 0 || p == 257 || p == 258 || p == 259 || p == pages - 1 {
            crate::lunix_strace!("  [SYS_MMAP_PAGE_CHECK] p={}/{}, virt 0x{:X}, mapped={}", p, pages, page_vaddr.as_u64(), crate::mm::vmm::is_page_mapped(page_vaddr));
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

    crate::lunix_strace!("  [SYS_MMAP_DONE] target=0x{:X}, len=0x{:X}", target_addr, length);

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

pub fn sys_set_tid_address(tidptr: *mut i32) -> isize {
    let tid = crate::task::scheduler::current_tid();
    crate::task::scheduler::set_thread_clear_child_tid(tid, tidptr as u64);
    tid as isize
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

pub fn sys_munmap(addr: u64, length: u64) -> isize {
    if addr & 0xFFF != 0 {
        return -22; // -EINVAL
    }
    crate::mm::vma::unmap(addr, length);
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

/// True if `fd` refers to the console tty (fd 0/1/2 or an opened /dev/tty*, /dev/console).
fn fd_is_tty(fd: usize) -> bool {
    let Some(proc_arc) = crate::task::scheduler::get_current_process() else { return false };
    let proc = proc_arc.lock();
    let Some(desc_arc) = proc.get_fd(fd) else { return false };
    let desc = desc_arc.lock();
    match &desc.target {
        FdTarget::Stdin | FdTarget::Stdout | FdTarget::Stderr => true,
        FdTarget::VfsHandle { path, .. } => path.starts_with("/dev/tty") || path == "/dev/console",
        _ => false,
    }
}

/// Give the device behind `fd` (if it is a VFS handle) first refusal on an ioctl.
fn device_ioctl(fd: usize, request: u64, arg: u64) -> Option<isize> {
    let proc_arc = crate::task::scheduler::get_current_process()?;
    let handle = {
        let proc = proc_arc.lock();
        let desc_arc = proc.get_fd(fd)?;
        let desc = desc_arc.lock();
        match &desc.target {
            FdTarget::VfsHandle { handle, .. } => handle.clone(),
            _ => return None,
        }
    };
    let r = handle.lock().ioctl(request, arg);
    r
}

pub fn sys_ioctl(fd: usize, request: u64, arg: u64) -> isize {
    if let Some(r) = device_ioctl(fd, request, arg) {
        return r;
    }
    const TIOCGWINSZ: u64 = 0x5413;
    const TCGETS: u64 = 0x5401;
    const TCSETS: u64 = 0x5402;
    const TCSETSW: u64 = 0x5403;
    const TCSETSF: u64 = 0x5404;
    const FIONBIO: u64 = 0x5421;
    const FIONREAD: u64 = 0x541B;
    // struct termios2 variants used by glibc >= 2.42 for tcgetattr/tcsetattr.
    const TCGETS2: u64 = 0x802C542A;
    const TCSETS2: u64 = 0x402C542B;
    const TCSETSW2: u64 = 0x402C542C;
    const TCSETSF2: u64 = 0x402C542D;
    const TIOCSCTTY: u64 = 0x540E;
    const TIOCGPGRP: u64 = 0x540F;
    const TIOCSPGRP: u64 = 0x5410;
    const TIOCNOTTY: u64 = 0x5422;
    const TIOCGSID: u64 = 0x5429;

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
        VT_GETMODE | VT_SETMODE | VT_ACTIVATE | VT_WAITACTIVE => 0,
        // KDGKBTYPE / KDGKBMODE / KDSKBMODE: X probes and then switches the keyboard to raw
        // mode; input reaches it through /dev/input/event* instead of the tty.
        0x4B33 => {
            if arg != 0 {
                unsafe { *(arg as *mut u8) = 2; } // KB_101
            }
            0
        }
        0x4B44 => {
            if arg != 0 {
                unsafe { *(arg as *mut i32) = 1; } // K_XLATE
            }
            0
        }
        0x4B45 => 0,
        // VT_GETSTATE: struct vt_stat { v_active, v_signal, v_state }
        0x5603 => {
            if arg != 0 {
                unsafe {
                    *(arg as *mut u16) = 1;
                    *((arg + 2) as *mut u16) = 0;
                    *((arg + 4) as *mut u16) = 2;
                }
            }
            0
        }
        KDSETMODE => {
            // KD_TEXT = 0, KD_GRAPHICS = 1: X takes the screen away from the text console.
            crate::display::console::set_graphics_mode(arg == 1);
            0
        }
        KDGETMODE => {
            if arg != 0 {
                unsafe { *(arg as *mut i32) = crate::display::console::graphics_mode() as i32; }
            }
            0
        }
        TIOCGPGRP | TIOCGSID => {
            if arg == 0 {
                return -14;
            }
            if !fd_is_tty(fd) {
                return -25;
            }
            // Process group and session id are the pid of the (single) session leader for now.
            unsafe { *(arg as *mut i32) = sys_getpid() as i32; }
            0
        }
        TIOCSPGRP | TIOCSCTTY | TIOCNOTTY => {
            if !fd_is_tty(fd) {
                return -25;
            }
            0
        }
        TCGETS => {
            if arg == 0 {
                return -14;
            }
            if !fd_is_tty(fd) {
                return -25; // -ENOTTY
            }
            unsafe { *(arg as *mut crate::drivers::tty::Termios) = crate::drivers::tty::get_termios(); }
            0
        }
        TCGETS2 => {
            if arg == 0 {
                return -14;
            }
            if !fd_is_tty(fd) {
                return -25;
            }
            // termios2 = termios (36 bytes) + c_ispeed + c_ospeed.
            const B38400: u32 = 15;
            unsafe {
                *(arg as *mut crate::drivers::tty::Termios) = crate::drivers::tty::get_termios();
                let speeds = (arg + 36) as *mut u32;
                *speeds = B38400;
                *speeds.add(1) = B38400;
            }
            0
        }
        TCSETS2 | TCSETSW2 | TCSETSF2 => {
            if arg == 0 {
                return -14;
            }
            if !fd_is_tty(fd) {
                return -25;
            }
            crate::drivers::tty::set_termios(unsafe { *(arg as *const crate::drivers::tty::Termios) });
            0
        }
        TCSETS | TCSETSW | TCSETSF => {
            if arg == 0 {
                return -14;
            }
            if !fd_is_tty(fd) {
                return -25;
            }
            crate::drivers::tty::set_termios(unsafe { *(arg as *const crate::drivers::tty::Termios) });
            0
        }
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
        FIONREAD => {
            if arg == 0 {
                return -14;
            }
            let n = if fd_is_tty(fd) { crate::drivers::tty::available() } else { 0 };
            unsafe { *(arg as *mut i32) = n as i32; }
            0
        }
        _ => -25, // -ENOTTY
    }
}

pub fn sys_brk(brk: u64) -> u64 {
    crate::mm::vma::brk(brk)
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
        copy_cstr(&mut uts.nodename, b"arch\0");
        copy_cstr(&mut uts.release, b"6.8.0-arch1-1-lunix\0");
        copy_cstr(&mut uts.version, b"#1 SMP PREEMPT 2026-09-21 (Lunix 0.1.0-arch-baremetal)\0");
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

/// `O_CLOEXEC` as stored in a descriptor's flags: the descriptor is closed on `execve`.
pub const O_CLOEXEC: u32 = 0x80000;

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
                    if cmd == F_DUPFD_CLOEXEC {
                        if let Some(d) = proc.get_fd(newfd) {
                            d.lock().flags |= O_CLOEXEC;
                        }
                    } else if let Some(d) = proc.get_fd(newfd) {
                        d.lock().flags &= !O_CLOEXEC; // dup'd descriptors start without close-on-exec
                    }
                    newfd as isize
                } else {
                    -9 // -EBADF
                }
            }
            F_GETFD => {
                if let Some(desc_arc) = proc.get_fd(fd) {
                    let desc = desc_arc.lock();
                    ((desc.flags & O_CLOEXEC) != 0) as isize // FD_CLOEXEC
                } else {
                    -9
                }
            }
            F_SETFD => {
                if let Some(desc_arc) = proc.get_fd(fd) {
                    let mut desc = desc_arc.lock();
                    desc.flags = (desc.flags & !O_CLOEXEC) | if arg & 1 != 0 { O_CLOEXEC } else { 0 };
                    0
                } else {
                    -9
                }
            }
            F_GETFL => {
                if let Some(desc_arc) = proc.get_fd(fd) {
                    let desc = desc_arc.lock();
                    // The access mode comes from what the descriptor is: glibc's fdopen()
                    // refuses to wrap a pipe's write end unless this reports O_WRONLY.
                    let access: u32 = match desc.target {
                        FdTarget::Stdin | FdTarget::PipeRead(_) => 0,
                        FdTarget::Stdout | FdTarget::Stderr | FdTarget::PipeWrite(_) => 1,
                        FdTarget::Socket(_) | FdTarget::UnixSocket(_) => 2,
                        _ => desc.flags & 3,
                    };
                    ((desc.flags & !3 & !O_CLOEXEC) | access) as isize
                } else {
                    -9
                }
            }
            F_SETFL => {
                if let Some(desc_arc) = proc.get_fd(fd) {
                    let mut desc = desc_arc.lock();
                    // Only O_APPEND/O_NONBLOCK-style status flags change; the access mode
                    // and the close-on-exec bit are kept.
                    let keep = (desc.flags & 3) | (desc.flags & O_CLOEXEC);
                    desc.flags = keep | (arg as u32 & !3 & !O_CLOEXEC);
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

    let cwd = crate::task::scheduler::get_current_process()
        .map(|p| p.lock().cwd.clone())
        .unwrap_or_else(|| alloc::string::String::from("/"));
    let bytes = cwd.as_bytes();
    if bytes.len() + 1 > size {
        return -34; // -ERANGE
    }
    unsafe {
        core::ptr::copy_nonoverlapping(bytes.as_ptr(), buf, bytes.len());
        *buf.add(bytes.len()) = 0;
    }
    (bytes.len() + 1) as isize
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
<<<<<<< HEAD
    crate::lunix_strace!("  [SYS_SOCKET] domain={}, type={}, proto={}", domain, sock_type, protocol);
=======
    crate::lunix_serial_println!("  [SYS_SOCKET] domain={}, type={}, proto={}", domain, sock_type, protocol);
>>>>>>> 67740566240f8bc3cf3fa1dbde7456fa0a3e6ea6
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

<<<<<<< HEAD
    // Only AF_UNIX (above) and AF_INET exist. Others (AF_NETLINK, AF_PACKET, AF_INET6 ...)
    // must fail cleanly so callers such as getifaddrs() take their error path.
    if domain != 2 {
        return -97; // -EAFNOSUPPORT
    }
=======
>>>>>>> 67740566240f8bc3cf3fa1dbde7456fa0a3e6ea6
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
<<<<<<< HEAD
    crate::lunix_strace!("  [SYS_BIND] fd={}, addr={:p}, len={}", sockfd, addr, addrlen);
=======
    crate::lunix_serial_println!("  [SYS_BIND] fd={}, addr={:p}, len={}", sockfd, addr, addrlen);
>>>>>>> 67740566240f8bc3cf3fa1dbde7456fa0a3e6ea6
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
<<<<<<< HEAD
    crate::lunix_strace!("  [SYS_LISTEN] fd={}, backlog={}", sockfd, backlog);
=======
    crate::lunix_serial_println!("  [SYS_LISTEN] fd={}, backlog={}", sockfd, backlog);
>>>>>>> 67740566240f8bc3cf3fa1dbde7456fa0a3e6ea6
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
<<<<<<< HEAD
    crate::lunix_strace!("  [SYS_CONNECT] fd={}, addr={:p}, len={}", sockfd, addr, addrlen);
=======
    crate::lunix_serial_println!("  [SYS_CONNECT] fd={}, addr={:p}, len={}", sockfd, addr, addrlen);
>>>>>>> 67740566240f8bc3cf3fa1dbde7456fa0a3e6ea6
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
<<<<<<< HEAD
    crate::lunix_strace!("  [SYS_ACCEPT] fd={}", sockfd);
=======
    crate::lunix_serial_println!("  [SYS_ACCEPT] fd={}", sockfd);
>>>>>>> 67740566240f8bc3cf3fa1dbde7456fa0a3e6ea6
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
<<<<<<< HEAD
    crate::lunix_strace!("  [SYS_SOCKETPAIR] type={}", sock_type);
=======
    crate::lunix_serial_println!("  [SYS_SOCKETPAIR] type={}", sock_type);
>>>>>>> 67740566240f8bc3cf3fa1dbde7456fa0a3e6ea6
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
<<<<<<< HEAD
        open_fds: 1,
=======
>>>>>>> 67740566240f8bc3cf3fa1dbde7456fa0a3e6ea6
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
<<<<<<< HEAD
        open_fds: 1,
=======
>>>>>>> 67740566240f8bc3cf3fa1dbde7456fa0a3e6ea6
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

<<<<<<< HEAD
/// Readiness of a single fd for the events in `events` (POLLIN/POLLOUT), or POLLNVAL.
pub(crate) fn fd_poll_revents(fd: usize, events: i16) -> i16 {
    let Some(proc_arc) = crate::task::scheduler::get_current_process() else {
        return events & (POLLIN | POLLOUT);
    };
    let proc = proc_arc.lock();
    let Some(desc_arc) = proc.get_fd(fd) else { return POLLNVAL };
    let desc = desc_arc.lock();
    let mut revents = 0i16;
    match desc.target {
        FdTarget::UnixSocket(ref sock) => {
            let sock_guard = sock.lock();
            if (events & POLLIN) != 0 && sock_guard.poll_read_ready() {
                revents |= POLLIN;
            }
            if (events & POLLOUT) != 0 && sock_guard.poll_write_ready() {
                revents |= POLLOUT;
            }
        }
        FdTarget::PipeRead(ref pipe) => {
            let p = pipe.lock();
            if (events & POLLIN) != 0 && p.available_to_read() > 0 {
                revents |= POLLIN;
            }
            // All writers gone and nothing left to read: end of file.
            if p.available_to_read() == 0 && (p.writers_count == 0 || p.closed_write) {
                revents |= POLLHUP;
            }
        }
        FdTarget::PipeWrite(ref pipe) => {
            let p = pipe.lock();
            if (events & POLLOUT) != 0 && p.available_to_write() > 0 {
                revents |= POLLOUT;
            }
            if p.readers_count == 0 || p.closed_read {
                revents |= POLLERR;
            }
        }
        FdTarget::EventFd(ref c) => {
            if (events & POLLIN) != 0 && epoll::eventfd_readable(c) {
                revents |= POLLIN;
            }
            revents |= events & POLLOUT;
        }
        FdTarget::Stdin => {
            if (events & POLLIN) != 0 && crate::drivers::tty::has_input() {
                revents |= POLLIN;
            }
        }
        FdTarget::VfsHandle { ref path, .. } if path.starts_with("/dev/tty") || path == "/dev/console" => {
            if (events & POLLIN) != 0 && crate::drivers::tty::has_input() {
                revents |= POLLIN;
            }
            revents |= events & POLLOUT;
        }
        FdTarget::VfsHandle { ref handle, .. } => {
            if (events & POLLIN) != 0 && handle.lock().poll_readable() {
                revents |= POLLIN;
            }
            revents |= events & POLLOUT;
        }
        _ => revents |= events & (POLLIN | POLLOUT),
    }
    revents
}

/// Sleep 1 ms at a time until `deadline_ms` (uptime ms; `None` = forever). True if expired.
fn wait_step(deadline_ms: Option<u64>) -> bool {
    if let Some(d) = deadline_ms {
        if crate::drivers::timer::get_ticks() >= d {
            return true;
        }
    }
    crate::task::scheduler::sleep_ms(1);
    false
}

fn deadline_from_ms(timeout_ms: i64) -> Option<u64> {
    if timeout_ms < 0 { None } else { Some(crate::drivers::timer::get_ticks() + timeout_ms as u64) }
}

pub fn sys_poll(fds: *mut LinuxPollFd, nfds: usize, timeout: i32) -> isize {
    poll_impl(fds, nfds, timeout as i64)
}

fn poll_impl(fds: *mut LinuxPollFd, nfds: usize, timeout_ms: i64) -> isize {
    let deadline = deadline_from_ms(timeout_ms);
    loop {
        let mut ready = 0;
        if !fds.is_null() {
            for pfd in unsafe { slice::from_raw_parts_mut(fds, nfds) } {
                pfd.revents = 0;
                if pfd.fd >= 0 {
                    pfd.revents = fd_poll_revents(pfd.fd as usize, pfd.events);
                }
                if pfd.revents != 0 {
                    ready += 1;
                }
            }
        }
        if ready > 0 || timeout_ms == 0 || wait_step(deadline) {
            return ready;
        }
    }
}

fn timespec_to_ms(ts: *const LinuxTimeSpec) -> i64 {
    if ts.is_null() {
        return -1;
    }
    let (sec, nsec) = unsafe { ((*ts).tv_sec, (*ts).tv_nsec) };
    sec.saturating_mul(1000).saturating_add((nsec + 999_999) / 1_000_000)
}

/// ppoll(2): the signal mask is ignored (no signal delivery yet).
pub fn sys_ppoll(fds: *mut LinuxPollFd, nfds: usize, ts: *const LinuxTimeSpec) -> isize {
    poll_impl(fds, nfds, timespec_to_ms(ts))
}

/// select(2) / pselect6(2) core over fd_set bitmaps.
fn select_impl(nfds: usize, readfds: *mut u64, writefds: *mut u64, exceptfds: *mut u64, timeout_ms: i64) -> isize {
    let nfds = nfds.min(1024);
    let words = (nfds + 63) / 64;
    let snapshot = |p: *mut u64| -> alloc::vec::Vec<u64> {
        if p.is_null() { alloc::vec![0u64; words] } else { unsafe { slice::from_raw_parts(p, words).to_vec() } }
    };
    let (rin, win) = (snapshot(readfds), snapshot(writefds));
    let deadline = deadline_from_ms(timeout_ms);
    loop {
        let mut rout = alloc::vec![0u64; words];
        let mut wout = alloc::vec![0u64; words];
        let mut ready = 0isize;
        for fd in 0..nfds {
            let (w, bit) = (fd / 64, 1u64 << (fd % 64));
            let mut ev = 0i16;
            if rin[w] & bit != 0 { ev |= POLLIN; }
            if win[w] & bit != 0 { ev |= POLLOUT; }
            if ev == 0 { continue; }
            let rev = fd_poll_revents(fd, ev);
            if rev == POLLNVAL { return -9; } // -EBADF
            if rev & (POLLIN | POLLHUP | POLLERR) != 0 && rin[w] & bit != 0 { rout[w] |= bit; ready += 1; }
            if rev & POLLOUT != 0 && win[w] & bit != 0 { wout[w] |= bit; ready += 1; }
        }
        if ready > 0 || timeout_ms == 0 || wait_step(deadline) {
            unsafe {
                if !readfds.is_null() { slice::from_raw_parts_mut(readfds, words).copy_from_slice(&rout); }
                if !writefds.is_null() { slice::from_raw_parts_mut(writefds, words).copy_from_slice(&wout); }
                if !exceptfds.is_null() { slice::from_raw_parts_mut(exceptfds, words).fill(0); }
            }
            return ready;
        }
    }
}

pub fn sys_select(nfds: usize, r: *mut u64, w: *mut u64, e: *mut u64, tv: *const LinuxTimeVal) -> isize {
    let ms = if tv.is_null() { -1 } else { unsafe { (*tv).tv_sec.saturating_mul(1000).saturating_add(((*tv).tv_usec + 999) / 1000) } };
    select_impl(nfds, r, w, e, ms)
}

/// pselect6(2): arg 6 is a {sigmask*, size} pair; the mask is ignored.
pub fn sys_pselect6(nfds: usize, r: *mut u64, w: *mut u64, e: *mut u64, ts: *const LinuxTimeSpec) -> isize {
    select_impl(nfds, r, w, e, timespec_to_ms(ts))
}

/// True if `fd` is a stream socket handled by the descriptor layer (AF_UNIX).
fn is_stream_socket(fd: usize) -> bool {
    crate::task::scheduler::get_current_process()
        .and_then(|p| p.lock().get_fd(fd))
        .map_or(false, |d| matches!(d.lock().target, FdTarget::UnixSocket(_)))
}

/// recv(2)/recvfrom(2): stream sockets read like `read`; MSG_DONTWAIT (0x40) applies to
/// this call only.
pub fn sys_recvfrom_flags(fd: usize, buf: *mut u8, len: usize, flags: i32) -> isize {
    if is_stream_socket(fd) {
        if (flags & 0x40) != 0 && !fd_has_data(fd) {
            return -11; // -EAGAIN
        }
        return sys_read(fd, buf, len);
    }
    sys_recvfrom(fd, buf, len)
}

pub fn sys_sendto(sockfd: usize, buf: *const u8, len: usize, _flags: u32, _dest_addr: *const u8) -> isize {
    if is_stream_socket(sockfd) {
        return sys_write(sockfd, buf, len);
    }
    let _ = sockfd;
=======
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
>>>>>>> 67740566240f8bc3cf3fa1dbde7456fa0a3e6ea6
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

/// Read a NUL-terminated user string (at most `max` bytes).
fn user_cstr(p: *const u8, max: usize) -> Option<alloc::string::String> {
    if p.is_null() {
        return None;
    }
    let mut len = 0;
    unsafe {
        while len < max && *p.add(len) != 0 {
            len += 1;
        }
        let bytes = core::slice::from_raw_parts(p, len);
        Some(alloc::string::String::from_utf8_lossy(bytes).into_owned())
    }
}

/// Read a NULL-terminated user array of C strings.
fn user_string_array(arr: *const *const u8, max_entries: usize) -> alloc::vec::Vec<alloc::string::String> {
    let mut out = alloc::vec::Vec::new();
    if arr.is_null() {
        return out;
    }
    for i in 0..max_entries {
        let p = unsafe { *arr.add(i) };
        match user_cstr(p, 32768) {
            Some(s) => out.push(s),
            None => break,
        }
    }
    out
}

pub fn sys_execve(filename_ptr: *const u8, argv_ptr: *const *const u8, envp_ptr: *const *const u8) -> isize {
    let Some(path) = user_cstr(filename_ptr, 4096) else { return -14 }; // -EFAULT

    // Errors glibc's execvp relies on: keep searching PATH on -ENOENT, give up on others.
    match crate::fs::vfs::stat(&path) {
        Err(_) => return -2, // -ENOENT
        Ok(inode) if inode.is_dir() => return -13, // -EACCES
        Ok(_) => {}
    }

    let mut args_vec = user_string_array(argv_ptr, 1024);
    if args_vec.is_empty() {
        args_vec.push(path.clone());
    }
    let env_vec = user_string_array(envp_ptr, 4096);

    let args_slices: alloc::vec::Vec<&str> = args_vec.iter().map(|s| s.as_str()).collect();
    let env_slices: alloc::vec::Vec<&str> = env_vec.iter().map(|s| s.as_str()).collect();
    // A NULL envp means an empty environment, as on Linux.
    let env_arg: Option<&[&str]> = Some(&env_slices);

    match crate::task::elf::exec_elf_replace(&path, &args_slices, env_arg) {
        Ok(_) => 0,
        Err(e) => {
            crate::lunix_strace!("  [SYS_EXECVE] Failed: {}", e);
            if e.contains("Invalid ELF") || e.contains("Not a 64-bit") || e.contains("too small") {
                -8 // -ENOEXEC
            } else if e.contains("too long") {
                -7 // -E2BIG
            } else {
                -12 // -ENOMEM
            }
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

    crate::lunix_strace!(
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
    sys_clone(0x4111, 0, 0, 0, 0)
}

/// `clone(CLONE_THREAD | CLONE_VM ...)`: a new thread in the calling process. It shares the
/// address space and descriptor table, runs on the caller-provided stack and gets its own
/// TLS register when `CLONE_SETTLS` is given.
fn clone_thread(flags: u64, stack: u64, ptid: u64, ctid: u64, tls: u64) -> isize {
    const CLONE_SETTLS: u64 = 0x0008_0000;
    const CLONE_PARENT_SETTID: u64 = 0x0010_0000;
    const CLONE_CHILD_CLEARTID: u64 = 0x0020_0000;
    const CLONE_CHILD_SETTID: u64 = 0x0100_0000;

    let pid = crate::task::scheduler::current_pid();
    let mut ctx = *crate::arch::x86_64::syscall::CURRENT_USER_CONTEXT.lock();
    if stack != 0 {
        ctx.rsp = stack;
    }
    ctx.rax = 0; // the new thread sees clone() return 0
    if flags & CLONE_SETTLS != 0 {
        ctx.fs_base = tls;
    }

    let tid = crate::task::scheduler::spawn_with_pid("user_thread", pid, clone_runner_trampoline, 8);
    crate::task::scheduler::inherit_fpu_state(tid);
    crate::task::scheduler::set_thread_fs_base(tid, ctx.fs_base);
    THREAD_CLONE_MAP.lock().insert(tid, ctx);

    // The address space is shared, so these writes are visible to both threads.
    if flags & CLONE_PARENT_SETTID != 0 && ptid != 0 {
        unsafe { *(ptid as *mut i32) = tid as i32 };
    }
    if flags & CLONE_CHILD_SETTID != 0 && ctid != 0 {
        unsafe { *(ctid as *mut i32) = tid as i32 };
    }
    if flags & CLONE_CHILD_CLEARTID != 0 {
        crate::task::scheduler::set_thread_clear_child_tid(tid, ctid);
    }
    crate::lunix_strace!("  [SYS_CLONE] thread TID {} in PID {} (flags=0x{:X})", tid, pid, flags);
    tid as isize
}

pub fn sys_clone(flags: u64, stack: u64, ptid: u64, ctid: u64, tls: u64) -> isize {
    const CLONE_THREAD: u64 = 0x0001_0000;
    if flags & CLONE_THREAD != 0 {
        return clone_thread(flags, stack, ptid, ctid, tls);
    }
    let parent_pid = crate::task::scheduler::current_pid();
    let parent_tid = crate::task::scheduler::current_tid();
    let child_pid = crate::task::scheduler::allocate_pid();
    let is_vfork = (flags & 0x4000) != 0;

    crate::lunix_strace!("  [SYS_CLONE] flags=0x{:X}, Parent PID {} (TID {}) spawning child PID {} (is_vfork={})...", flags, parent_pid, parent_tid, child_pid, is_vfork);

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
    // The child continues with the parent's floating point / SSE registers.
    crate::task::scheduler::inherit_fpu_state(child_tid);
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
            crate::lunix_strace!("  [SYS_WAIT4] Reaped child PID {} with exit_code {}", child_pid, exit_code);
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
    if path.starts_with('/') {
        alloc::string::String::from(path)
    } else {
        let base_cwd = if let Some(proc_arc) = crate::task::scheduler::get_current_process() {
            let proc = proc_arc.lock();
            if dfd >= 0 {
                if let Some(desc_arc) = proc.get_fd(dfd as usize) {
                    let desc = desc_arc.lock();
                    match desc.target {
                        FdTarget::Directory { ref path, .. } => path.clone(),
                        _ => proc.cwd.clone(),
                    }
                } else {
                    proc.cwd.clone()
                }
            } else {
                proc.cwd.clone()
            }
        } else {
            alloc::string::String::from("/")
        };

        if base_cwd == "/" {
            let mut full = alloc::string::String::from("/");
            full.push_str(path);
            full
        } else {
            let mut full = base_cwd;
            full.push('/');
            full.push_str(path);
            full
        }
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
<<<<<<< HEAD
    crate::lunix_strace!("  [SYS_OPENAT] dfd={}, path='{}', flags=0x{:X}", dfd, resolved_path, flags);
=======
    crate::lunix_serial_println!("  [SYS_OPENAT] dfd={}, path='{}', flags=0x{:X}", dfd, resolved_path, flags);
>>>>>>> 67740566240f8bc3cf3fa1dbde7456fa0a3e6ea6

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
<<<<<<< HEAD
                crate::lunix_strace!("  [SYS_OPENAT_CREAT] Created dynamic in-memory file '{}' -> fd {}", resolved_path, fd);
=======
                crate::lunix_serial_println!("  [SYS_OPENAT_CREAT] Created dynamic in-memory file '{}' -> fd {}", resolved_path, fd);
>>>>>>> 67740566240f8bc3cf3fa1dbde7456fa0a3e6ea6
                return fd as isize;
            }
        }
    }

    -2 // -ENOENT
}

pub fn sys_fstat(fd: usize, statbuf: *mut LinuxStat) -> isize {
    crate::lunix_strace!("  [SYS_FSTAT] fd={}", fd);
    if statbuf.is_null() {
        return -14; // -EFAULT
    }
    if let Some(proc_arc) = crate::task::scheduler::get_current_process() {
        let proc = proc_arc.lock();
        if let Some(desc_arc) = proc.get_fd(fd) {
            let desc = desc_arc.lock();
            // File type bits of st_mode: pipes are FIFOs, sockets are sockets, tty/device
            // handles are character devices. Tools like head/cat/ls choose behaviour from this.
            let (type_bits, rdev): (u32, u64) = match desc.target {
                FdTarget::PipeRead(_) | FdTarget::PipeWrite(_) => (0o010000, 0),
                FdTarget::Socket(_) | FdTarget::UnixSocket(_) => (0o140000, 0),
                FdTarget::Stdin | FdTarget::Stdout | FdTarget::Stderr => (0o020000, 4 << 8),
                FdTarget::VfsHandle { ref path, .. } if path.starts_with("/dev/tty") || path == "/dev/console" => (0o020000, 4 << 8),
                FdTarget::VfsHandle { ref path, .. } if path.starts_with("/dev/") && !path.starts_with("/dev/sd") && !path.starts_with("/dev/vd") && !path.starts_with("/dev/nvme") => (0o020000, char_dev_rdev(path)),
                FdTarget::VfsHandle { ref path, .. } if path.starts_with("/dev/") => (0o060000, 8 << 8),
                _ => (0o100000, 0),
            };
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
                st.st_mode = if is_dir { 0o040755 } else if type_bits == 0o100000 { 0o100755 } else { type_bits | 0o620 };
                st.st_uid = 0;
                st.st_gid = 0;
                st.st_rdev = rdev;
                st.st_size = if type_bits == 0o100000 || is_dir { size } else { 0 };
                st.st_blksize = 512;
                st.st_blocks = (st.st_size + 511) / 512;
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

<<<<<<< HEAD
    // AT_SYMLINK_NOFOLLOW (0x100) gives lstat semantics.
    let lookup = if (flags & 0x100) != 0 { crate::fs::vfs::lstat(&resolved) } else { crate::fs::vfs::stat(&resolved) };
    if let Ok(inode) = lookup {
=======
    if let Ok(inode) = crate::fs::vfs::stat(&resolved) {
>>>>>>> 67740566240f8bc3cf3fa1dbde7456fa0a3e6ea6
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
            use crate::fs::inode::INodeType;
            let type_bits: u32 = match inode.node_type {
                INodeType::Directory => 0o040000,
                INodeType::SymLink => 0o120000,
                INodeType::CharDevice => 0o020000,
                INodeType::BlockDevice => 0o060000,
                INodeType::File => 0o100000,
            };
            let perm = if inode.permissions != 0 { inode.permissions as u32 & 0o7777 } else { 0o755 };
            st.st_mode = type_bits | perm;
            st.st_uid = 0;
            st.st_gid = 0;
            st.st_rdev = if inode.node_type == INodeType::CharDevice { char_dev_rdev(&resolved) } else { 0 };
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
    sys_readlinkat(-100, path_ptr, buf, bufsiz)
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
        crate::lunix_strace!("  [SYS_READLINK] path='{}'", resolved);
        if resolved == "/proc/self/exe" || resolved.ends_with("/exe") {
            // Path the current process was exec'd from.
            let exe = crate::task::scheduler::get_current_process()
                .map(|p| p.lock().name.clone())
                .unwrap_or_default();
            let exe_path = exe.as_bytes();
            let copy_len = exe_path.len().min(bufsiz);
            unsafe {
                core::ptr::copy_nonoverlapping(exe_path.as_ptr(), buf, copy_len);
            }
            return copy_len as isize;
        }
        if let Ok(target) = crate::fs::vfs::readlink(&resolved) {
            let bytes = target.as_bytes();
            let copy_len = bytes.len().min(bufsiz);
            unsafe {
                core::ptr::copy_nonoverlapping(bytes.as_ptr(), buf, copy_len);
            }
            return copy_len as isize;
        }
        if let Some(target) = SYMLINKS.lock().get(&resolved) {
            let bytes = target.as_bytes();
            let copy_len = bytes.len().min(bufsiz);
            unsafe {
                core::ptr::copy_nonoverlapping(bytes.as_ptr(), buf, copy_len);
            }
            return copy_len as isize;
        }
    }
    -22 // -EINVAL
}

pub fn sys_truncate(path_ptr: *const u8, length: usize) -> isize {
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
        let resolved = resolve_at_path(-100, path);
        if let Some(buf_arc) = IN_MEMORY_FILES.lock().get(&resolved).cloned() {
            buf_arc.lock().resize(length, 0);
            return 0;
        }
    }
    0
}

pub fn sys_ftruncate(fd: usize, length: usize) -> isize {
    if let Some(proc_arc) = crate::task::scheduler::get_current_process() {
        let proc = proc_arc.lock();
        if let Some(desc_arc) = proc.get_fd(fd) {
            let desc = desc_arc.lock();
            if let FdTarget::File { ref buffer, .. } = desc.target {
                buffer.lock().resize(length, 0);
                return 0;
            }
        }
    }
    0
}

pub fn sys_link(oldpath_ptr: *const u8, newpath_ptr: *const u8) -> isize {
    sys_linkat(-100, oldpath_ptr, -100, newpath_ptr, 0)
}

pub fn sys_linkat(olddfd: i32, oldname_ptr: *const u8, newdfd: i32, newname_ptr: *const u8, _flags: i32) -> isize {
    if oldname_ptr.is_null() || newname_ptr.is_null() {
        return -14;
    }
    let mut old_len = 0;
    unsafe {
        while *oldname_ptr.add(old_len) != 0 && old_len < 256 {
            old_len += 1;
        }
    }
    let mut new_len = 0;
    unsafe {
        while *newname_ptr.add(new_len) != 0 && new_len < 256 {
            new_len += 1;
        }
    }
    let old_slice = unsafe { core::slice::from_raw_parts(oldname_ptr, old_len) };
    let new_slice = unsafe { core::slice::from_raw_parts(newname_ptr, new_len) };
    let old_rel = match core::str::from_utf8(old_slice) {
        Ok(s) => s,
        Err(_) => return -22,
    };
    let new_rel = match core::str::from_utf8(new_slice) {
        Ok(s) => s,
        Err(_) => return -22,
    };

    let old_resolved = resolve_at_path(olddfd, old_rel);
    let new_resolved = resolve_at_path(newdfd, new_rel);
    crate::lunix_strace!("  [SYS_LINKAT] linking '{}' -> '{}'", old_resolved, new_resolved);

    let arc_opt = {
        let mem = IN_MEMORY_FILES.lock();
        mem.get(&old_resolved).cloned()
    };

    if let Some(buf_arc) = arc_opt {
        IN_MEMORY_FILES.lock().insert(new_resolved, buf_arc);
        return 0;
    }

    if let Ok(mut handle) = crate::fs::vfs::open(&old_resolved) {
        let mut data = alloc::vec::Vec::new();
        let mut chunk = [0u8; 512];
        while let Ok(n) = handle.read(&mut chunk) {
            if n == 0 { break; }
            data.extend_from_slice(&chunk[..n]);
        }
        let arc_buf = alloc::sync::Arc::new(spin::Mutex::new(data));
        IN_MEMORY_FILES.lock().insert(new_resolved, arc_buf.clone());
        IN_MEMORY_FILES.lock().insert(old_resolved, arc_buf);
        return 0;
    }

    // Fallback: create shared empty buffer for lock files
    let arc_buf = alloc::sync::Arc::new(spin::Mutex::new(alloc::vec::Vec::new()));
    IN_MEMORY_FILES.lock().insert(new_resolved, arc_buf.clone());
    IN_MEMORY_FILES.lock().insert(old_resolved, arc_buf);
    0
}

pub fn sys_symlink(target_ptr: *const u8, linkpath_ptr: *const u8) -> isize {
    sys_symlinkat(target_ptr, -100, linkpath_ptr)
}

pub fn sys_symlinkat(target_ptr: *const u8, newdfd: i32, linkpath_ptr: *const u8) -> isize {
    if target_ptr.is_null() || linkpath_ptr.is_null() {
        return -14;
    }
    let mut t_len = 0;
    unsafe {
        while *target_ptr.add(t_len) != 0 && t_len < 256 {
            t_len += 1;
        }
    }
    let mut l_len = 0;
    unsafe {
        while *linkpath_ptr.add(l_len) != 0 && l_len < 256 {
            l_len += 1;
        }
    }
    let t_slice = unsafe { core::slice::from_raw_parts(target_ptr, t_len) };
    let l_slice = unsafe { core::slice::from_raw_parts(linkpath_ptr, l_len) };
    let target_str = match core::str::from_utf8(t_slice) {
        Ok(s) => s,
        Err(_) => return -22,
    };
    let link_rel = match core::str::from_utf8(l_slice) {
        Ok(s) => s,
        Err(_) => return -22,
    };
    let link_resolved = resolve_at_path(newdfd, link_rel);
    crate::lunix_strace!("  [SYS_SYMLINKAT] created symlink '{}' -> '{}'", link_resolved, target_str);
    SYMLINKS.lock().insert(link_resolved, alloc::string::String::from(target_str));
    0
}

pub fn sys_getuid() -> u32 {
    crate::task::scheduler::get_current_process()
        .map(|p| p.lock().uid)
        .unwrap_or(0)
}

pub fn sys_geteuid() -> u32 {
    crate::task::scheduler::get_current_process()
        .map(|p| p.lock().euid)
        .unwrap_or(0)
}

pub fn sys_getgid() -> u32 {
    crate::task::scheduler::get_current_process()
        .map(|p| p.lock().gid)
        .unwrap_or(0)
}

pub fn sys_getegid() -> u32 {
    crate::task::scheduler::get_current_process()
        .map(|p| p.lock().egid)
        .unwrap_or(0)
}

pub fn sys_setuid(uid: u32) -> isize {
    if let Some(proc_arc) = crate::task::scheduler::get_current_process() {
        let mut p = proc_arc.lock();
        p.uid = uid;
        p.euid = uid;
        p.suid = uid;
        p.fsuid = uid;
    }
    0
}

pub fn sys_setgid(gid: u32) -> isize {
    if let Some(proc_arc) = crate::task::scheduler::get_current_process() {
        let mut p = proc_arc.lock();
        p.gid = gid;
        p.egid = gid;
        p.sgid = gid;
        p.fsgid = gid;
    }
    0
}

pub fn sys_setreuid(ruid: u32, euid: u32) -> isize {
    if let Some(proc_arc) = crate::task::scheduler::get_current_process() {
        let mut p = proc_arc.lock();
        if ruid != u32::MAX { p.uid = ruid; }
        if euid != u32::MAX { p.euid = euid; }
    }
    0
}

pub fn sys_setregid(rgid: u32, egid: u32) -> isize {
    if let Some(proc_arc) = crate::task::scheduler::get_current_process() {
        let mut p = proc_arc.lock();
        if rgid != u32::MAX { p.gid = rgid; }
        if egid != u32::MAX { p.egid = egid; }
    }
    0
}

pub fn sys_setresuid(ruid: u32, euid: u32, suid: u32) -> isize {
    if let Some(proc_arc) = crate::task::scheduler::get_current_process() {
        let mut p = proc_arc.lock();
        if ruid != u32::MAX { p.uid = ruid; }
        if euid != u32::MAX { p.euid = euid; }
        if suid != u32::MAX { p.suid = suid; }
    }
    0
}

pub fn sys_getresuid(ruid_ptr: *mut u32, euid_ptr: *mut u32, suid_ptr: *mut u32) -> isize {
    if let Some(proc_arc) = crate::task::scheduler::get_current_process() {
        let p = proc_arc.lock();
        unsafe {
            if !ruid_ptr.is_null() { *ruid_ptr = p.uid; }
            if !euid_ptr.is_null() { *euid_ptr = p.euid; }
            if !suid_ptr.is_null() { *suid_ptr = p.suid; }
        }
    }
    0
}

pub fn sys_setresgid(rgid: u32, egid: u32, sgid: u32) -> isize {
    if let Some(proc_arc) = crate::task::scheduler::get_current_process() {
        let mut p = proc_arc.lock();
        if rgid != u32::MAX { p.gid = rgid; }
        if egid != u32::MAX { p.egid = egid; }
        if sgid != u32::MAX { p.sgid = sgid; }
    }
    0
}

pub fn sys_getresgid(rgid_ptr: *mut u32, egid_ptr: *mut u32, sgid_ptr: *mut u32) -> isize {
    if let Some(proc_arc) = crate::task::scheduler::get_current_process() {
        let p = proc_arc.lock();
        unsafe {
            if !rgid_ptr.is_null() { *rgid_ptr = p.gid; }
            if !egid_ptr.is_null() { *egid_ptr = p.egid; }
            if !sgid_ptr.is_null() { *sgid_ptr = p.sgid; }
        }
    }
    0
}

pub fn sys_umask(mask: u32) -> isize {
    if let Some(proc_arc) = crate::task::scheduler::get_current_process() {
        let mut p = proc_arc.lock();
        let old = p.umask;
        p.umask = mask & 0o777;
        old as isize
    } else {
        0o022
    }
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
    crate::lunix_strace!("  [SYS_ARCH_PRCTL] code=0x{:X}, addr=0x{:X}", code, addr);
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
<<<<<<< HEAD
        crate::lunix_strace!("  [SYS_UNLINK] unlinking '{}'", resolved);
=======
        crate::lunix_serial_println!("  [SYS_UNLINK] unlinking '{}'", resolved);
>>>>>>> 67740566240f8bc3cf3fa1dbde7456fa0a3e6ea6
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
        crate::lunix_strace!("  [SYS_PIPE2] read_fd={}, write_fd={}", read_fd, write_fd);
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

/// Linux device number (major<<8 | minor) of a `/dev` character device. X's evdev driver
/// (among others) tells devices apart by `st_rdev`, so each node needs its own.
fn char_dev_rdev(path: &str) -> u64 {
    let name = path.trim_start_matches("/dev/");
    let (major, minor): (u64, u64) = if let Some(n) = name.strip_prefix("input/event") {
        (13, 64 + n.parse::<u64>().unwrap_or(0))
    } else if let Some(n) = name.strip_prefix("pts/") {
        (136, n.parse::<u64>().unwrap_or(0))
    } else {
        match name {
            "null" => (1, 3),
            "zero" => (1, 5),
            "random" => (1, 8),
            "urandom" => (1, 9),
            "console" => (5, 1),
            "tty" => (5, 0),
            "ptmx" | "pts/ptmx" => (5, 2),
            "fb0" | "fb/0" | "graphics/fb0" => (29, 0),
            "input/mice" | "mice" => (13, 63),
            "input/mouse" | "mouse" => (13, 32),
            _ => (1, 0),
        }
    };
    (major << 8) | minor
}

/// `struct statx_timestamp`.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct LinuxStatxTimestamp {
    pub tv_sec: i64,
    pub tv_nsec: u32,
    pub __reserved: i32,
}

/// `struct statx` (256 bytes).
#[repr(C)]
pub struct LinuxStatx {
    pub stx_mask: u32,
    pub stx_blksize: u32,
    pub stx_attributes: u64,
    pub stx_nlink: u32,
    pub stx_uid: u32,
    pub stx_gid: u32,
    pub stx_mode: u16,
    pub __spare0: u16,
    pub stx_ino: u64,
    pub stx_size: u64,
    pub stx_blocks: u64,
    pub stx_attributes_mask: u64,
    pub stx_atime: LinuxStatxTimestamp,
    pub stx_btime: LinuxStatxTimestamp,
    pub stx_ctime: LinuxStatxTimestamp,
    pub stx_mtime: LinuxStatxTimestamp,
    pub stx_rdev_major: u32,
    pub stx_rdev_minor: u32,
    pub stx_dev_major: u32,
    pub stx_dev_minor: u32,
    pub stx_mnt_id: u64,
    pub stx_dio_mem_align: u32,
    pub stx_dio_offset_align: u32,
    pub __spare3: [u64; 12],
}

/// statx(2): implemented on top of fstatat, which already handles AT_EMPTY_PATH and dirfds.
pub fn sys_statx(dfd: i32, path: *const u8, flags: u32, _mask: u32, out: *mut LinuxStatx) -> isize {
    if out.is_null() {
        return -14;
    }
    let mut st: LinuxStat = unsafe { core::mem::zeroed() };
    let ret = sys_fstatat(dfd, path, &mut st, flags);
    if ret < 0 {
        return ret;
    }
    const STATX_BASIC_STATS: u32 = 0x7ff;
    let ts = |sec: i64, nsec: i64| LinuxStatxTimestamp { tv_sec: sec, tv_nsec: nsec as u32, __reserved: 0 };
    unsafe {
        core::ptr::write_bytes(out as *mut u8, 0, core::mem::size_of::<LinuxStatx>());
        let x = &mut *out;
        x.stx_mask = STATX_BASIC_STATS;
        x.stx_blksize = st.st_blksize as u32;
        x.stx_nlink = st.st_nlink as u32;
        x.stx_uid = st.st_uid;
        x.stx_gid = st.st_gid;
        x.stx_mode = st.st_mode as u16;
        x.stx_ino = st.st_ino;
        x.stx_size = st.st_size as u64;
        x.stx_blocks = st.st_blocks as u64;
        x.stx_atime = ts(st.st_atime, st.st_atime_nsec);
        x.stx_ctime = ts(st.st_ctime, st.st_ctime_nsec);
        x.stx_mtime = ts(st.st_mtime, st.st_mtime_nsec);
        x.stx_rdev_major = ((st.st_rdev >> 8) & 0xfff) as u32;
        x.stx_rdev_minor = (st.st_rdev & 0xff) as u32;
        x.stx_dev_major = ((st.st_dev >> 8) & 0xfff) as u32;
        x.stx_dev_minor = (st.st_dev & 0xff) as u32;
    }
    0
}

/// prctl(2). Options the kernel has no per-task state for are accepted and ignored;
/// anything unknown is rejected with -EINVAL.
pub fn sys_prctl(option: u64, arg2: u64, _arg3: u64) -> isize {
    const PR_SET_PDEATHSIG: u64 = 1;
    const PR_GET_PDEATHSIG: u64 = 2;
    const PR_GET_DUMPABLE: u64 = 3;
    const PR_SET_DUMPABLE: u64 = 4;
    const PR_SET_KEEPCAPS: u64 = 8;
    const PR_SET_NAME: u64 = 15;
    const PR_GET_NAME: u64 = 16;
    const PR_CAPBSET_READ: u64 = 23;
    const PR_CAPBSET_DROP: u64 = 24;
    const PR_SET_CHILD_SUBREAPER: u64 = 36;
    const PR_SET_NO_NEW_PRIVS: u64 = 38;
    const PR_GET_NO_NEW_PRIVS: u64 = 39;
    match option {
        PR_SET_PDEATHSIG | PR_SET_DUMPABLE | PR_SET_KEEPCAPS | PR_SET_NAME | PR_CAPBSET_DROP
        | PR_SET_CHILD_SUBREAPER | PR_SET_NO_NEW_PRIVS => 0,
        PR_GET_PDEATHSIG => {
            if arg2 == 0 { return -14; }
            unsafe { *(arg2 as *mut i32) = 0; }
            0
        }
        PR_GET_DUMPABLE | PR_CAPBSET_READ => 1,
        PR_GET_NO_NEW_PRIVS => 0,
        PR_GET_NAME => {
            if arg2 == 0 { return -14; }
            let name = b"lunix ";
            unsafe { core::ptr::copy_nonoverlapping(name.as_ptr(), arg2 as *mut u8, name.len()); }
            0
        }
        _ => -22,
    }
}
