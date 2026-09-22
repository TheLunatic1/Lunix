//! `epoll` and `eventfd`.
//!
//! An epoll instance is a set of (fd, event mask, user data) registrations. `epoll_wait`
//! blocks until at least one registered descriptor is ready (using the same readiness test
//! as `poll`/`select`) or the timeout expires. Edge-triggered mode is treated as level
//! triggered, which is always a correct (if chattier) superset.

use super::{fd_poll_revents, O_CLOEXEC};
use crate::task::process::{FdTarget, FileDescriptor};
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::Mutex;

const EPOLLIN: u32 = 0x001;
const EPOLLOUT: u32 = 0x004;
const EPOLLERR: u32 = 0x008;
const EPOLLHUP: u32 = 0x010;
const EPOLLONESHOT: u32 = 1 << 30;

const EPOLL_CTL_ADD: i32 = 1;
const EPOLL_CTL_DEL: i32 = 2;
const EPOLL_CTL_MOD: i32 = 3;

const EPOLL_CLOEXEC: i32 = 0x80000;
const EFD_NONBLOCK: i32 = 0x800;
const EFD_CLOEXEC: i32 = 0x80000;

/// `struct epoll_event` (packed on x86_64).
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct EpollEvent {
    pub events: u32,
    pub data: u64,
}

#[derive(Clone)]
pub struct EpollEntry {
    fd: i32,
    events: u32,
    data: u64,
    /// EPOLLONESHOT entries are disabled after they fire, until re-armed with EPOLL_CTL_MOD.
    disabled: bool,
}

pub type EpollSet = Arc<Mutex<Vec<EpollEntry>>>;

fn current_fd(fd: usize) -> Option<Arc<Mutex<FileDescriptor>>> {
    crate::task::scheduler::get_current_process().and_then(|p| p.lock().get_fd(fd))
}

fn epoll_set(epfd: usize) -> Option<EpollSet> {
    let desc = current_fd(epfd)?;
    let g = desc.lock();
    match &g.target {
        FdTarget::Epoll(set) => Some(set.clone()),
        _ => None,
    }
}

pub fn sys_epoll_create1(flags: i32) -> isize {
    let Some(proc_arc) = crate::task::scheduler::get_current_process() else { return -1 };
    let mut proc = proc_arc.lock();
    let desc = FileDescriptor {
        target: FdTarget::Epoll(Arc::new(Mutex::new(Vec::new()))),
        flags: 2 | if flags & EPOLL_CLOEXEC != 0 { O_CLOEXEC } else { 0 },
    };
    match proc.allocate_fd(desc) {
        Some(fd) => fd as isize,
        None => -24, // -EMFILE
    }
}

pub fn sys_epoll_ctl(epfd: usize, op: i32, fd: i32, event: *const EpollEvent) -> isize {
    let Some(set) = epoll_set(epfd) else { return -22 }; // -EINVAL: not an epoll fd
    if fd < 0 || current_fd(fd as usize).is_none() {
        return -9; // -EBADF
    }
    let mut entries = set.lock();
    let pos = entries.iter().position(|e| e.fd == fd);
    match op {
        EPOLL_CTL_ADD | EPOLL_CTL_MOD => {
            if event.is_null() {
                return -14;
            }
            let ev = unsafe { core::ptr::read_unaligned(event) };
            let entry = EpollEntry { fd, events: ev.events, data: ev.data, disabled: false };
            match (op, pos) {
                (EPOLL_CTL_ADD, Some(_)) => -17, // -EEXIST
                (EPOLL_CTL_ADD, None) => {
                    entries.push(entry);
                    0
                }
                (_, Some(i)) => {
                    entries[i] = entry;
                    0
                }
                (_, None) => -2, // -ENOENT
            }
        }
        EPOLL_CTL_DEL => match pos {
            Some(i) => {
                entries.remove(i);
                0
            }
            None => -2,
        },
        _ => -22,
    }
}

/// `epoll_wait(2)` / `epoll_pwait(2)` (the signal mask is ignored).
pub fn sys_epoll_wait(epfd: usize, events: *mut EpollEvent, maxevents: i32, timeout_ms: i32) -> isize {
    if events.is_null() || maxevents <= 0 {
        return -22;
    }
    let Some(set) = epoll_set(epfd) else { return -22 };
    let deadline = if timeout_ms < 0 {
        None
    } else {
        Some(crate::drivers::timer::get_ticks() + timeout_ms as u64)
    };
    loop {
        let mut ready = 0usize;
        {
            let mut entries = set.lock();
            for e in entries.iter_mut() {
                if ready >= maxevents as usize {
                    break;
                }
                if e.disabled {
                    continue;
                }
                let want = (e.events & (EPOLLIN | EPOLLOUT)) as i16;
                let rev = fd_poll_revents(e.fd as usize, want) as u32;
                // Errors and hang-ups are always reported, even if not asked for.
                let fired = rev & (e.events | EPOLLERR | EPOLLHUP);
                if fired != 0 {
                    unsafe {
                        core::ptr::write_unaligned(events.add(ready), EpollEvent { events: fired, data: e.data });
                    }
                    ready += 1;
                    if e.events & EPOLLONESHOT != 0 {
                        e.disabled = true;
                    }
                }
            }
        }
        if ready > 0 || timeout_ms == 0 {
            return ready as isize;
        }
        if let Some(d) = deadline {
            if crate::drivers::timer::get_ticks() >= d {
                return 0;
            }
        }
        crate::task::scheduler::sleep_ms(1);
    }
}

pub fn sys_eventfd2(initval: u32, flags: i32) -> isize {
    let Some(proc_arc) = crate::task::scheduler::get_current_process() else { return -1 };
    let mut proc = proc_arc.lock();
    let mut fl = 2u32;
    if flags & EFD_NONBLOCK != 0 {
        fl |= 0x800;
    }
    if flags & EFD_CLOEXEC != 0 {
        fl |= O_CLOEXEC;
    }
    let desc = FileDescriptor { target: FdTarget::EventFd(Arc::new(Mutex::new(initval as u64))), flags: fl };
    match proc.allocate_fd(desc) {
        Some(fd) => fd as isize,
        None => -24,
    }
}

/// Read an eventfd: returns the counter (8 bytes) and resets it; blocks while it is zero.
pub fn eventfd_read(counter: &Arc<Mutex<u64>>, buf: &mut [u8], nonblock: bool) -> isize {
    if buf.len() < 8 {
        return -22;
    }
    loop {
        {
            let mut c = counter.lock();
            if *c > 0 {
                buf[..8].copy_from_slice(&c.to_le_bytes());
                *c = 0;
                return 8;
            }
        }
        if nonblock {
            return -11; // -EAGAIN
        }
        crate::task::scheduler::sleep_ms(1);
    }
}

/// Write an eventfd: adds the 8-byte value to the counter.
pub fn eventfd_write(counter: &Arc<Mutex<u64>>, buf: &[u8]) -> isize {
    if buf.len() < 8 {
        return -22;
    }
    let v = u64::from_le_bytes([buf[0], buf[1], buf[2], buf[3], buf[4], buf[5], buf[6], buf[7]]);
    let mut c = counter.lock();
    *c = c.saturating_add(v);
    8
}

pub fn eventfd_readable(counter: &Arc<Mutex<u64>>) -> bool {
    *counter.lock() > 0
}
