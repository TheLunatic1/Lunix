pub mod elf;
pub mod pipe;
pub mod process;
pub mod scheduler;
pub mod switch;
pub mod thread;
pub mod user;

pub use pipe::{create_pipe_pair, PipeBuffer, PipeError};
pub use process::{FdTarget, FileDescriptor, Process};
pub use scheduler::{init, sleep_ms, spawn, yield_now};
pub use thread::{Thread, ThreadState};

/// Paths tried for PID 1, in order. Real Arch `/sbin/init` is systemd, which needs far more
/// kernel surface (cgroups, netlink, signalfd, ...) than exists yet, so bring-up starts
/// with the real Arch bash, like booting Linux with `init=/bin/bash`.
const INIT_CANDIDATES: &[&str] = &[
    "/usr/bin/bash",
    "/bin/bash",
    "/sbin/init",
    "/usr/lib/systemd/systemd",
    "/bin/sh",
];

/// Exec the first available init binary as PID 1. Returns the path started.
pub fn start_init() -> Option<&'static str> {
    for &path in INIT_CANDIDATES {
        if crate::fs::vfs::stat(path).is_err() {
            continue;
        }
        // Printed before the exec: init may write to the console immediately.
        crate::lunix_println!("[+] Starting {} as PID 1.", path);
        match elf::exec_elf_with_args(path, &["init"]) {
            Ok(_) => return Some(path),
            Err(e) => crate::lunix_println!("[-] init: failed to start '{}': {}", path, e),
        }
    }
    None
}
