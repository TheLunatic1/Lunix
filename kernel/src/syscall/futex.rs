//! futex(2): the kernel side of userspace locks (pthread mutexes, condition variables,
//! `pthread_join`, ...).
//!
//! A waiter is registered under (address space, address). `FUTEX_WAKE` marks waiters as
//! woken; each waiter polls its own flag every millisecond, so a wake that arrives between
//! registration and sleeping is never lost.

use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, Ordering};
use spin::Mutex;
use x86_64::registers::control::Cr3;

const FUTEX_WAIT: i32 = 0;
const FUTEX_WAKE: i32 = 1;
const FUTEX_REQUEUE: i32 = 3;
const FUTEX_CMP_REQUEUE: i32 = 4;
const FUTEX_WAKE_OP: i32 = 5;
const FUTEX_WAIT_BITSET: i32 = 9;
const FUTEX_WAKE_BITSET: i32 = 10;

struct Waiter {
    cr3: u64,
    addr: u64,
    woken: Arc<AtomicBool>,
}

static WAITERS: Mutex<Vec<Waiter>> = Mutex::new(Vec::new());

fn current_cr3() -> u64 {
    Cr3::read().0.start_address().as_u64()
}

/// Wake up to `n` threads waiting on `addr` in the current address space.
pub fn wake(addr: u64, n: usize) -> usize {
    let cr3 = current_cr3();
    let mut w = WAITERS.lock();
    let mut woken = 0;
    let mut i = 0;
    while i < w.len() && woken < n {
        if w[i].cr3 == cr3 && w[i].addr == addr {
            w[i].woken.store(true, Ordering::SeqCst);
            w.remove(i);
            woken += 1;
        } else {
            i += 1;
        }
    }
    woken
}

fn requeue(from: u64, to: u64, n: usize) -> usize {
    let cr3 = current_cr3();
    let mut w = WAITERS.lock();
    let mut moved = 0;
    for waiter in w.iter_mut() {
        if moved >= n {
            break;
        }
        if waiter.cr3 == cr3 && waiter.addr == from {
            waiter.addr = to;
            moved += 1;
        }
    }
    moved
}

/// Block until woken, or until `deadline` (uptime in ms; None = forever).
fn wait(addr: u64, expected: u32, deadline: Option<u64>) -> isize {
    let woken = Arc::new(AtomicBool::new(false));
    {
        let mut w = WAITERS.lock();
        // The value check happens under the lock so a concurrent wake cannot slip between
        // the check and the registration.
        let current = unsafe { core::ptr::read_volatile(addr as *const u32) };
        if current != expected {
            return -11; // -EAGAIN
        }
        w.push(Waiter { cr3: current_cr3(), addr, woken: woken.clone() });
    }
    loop {
        if woken.load(Ordering::SeqCst) {
            return 0;
        }
        if let Some(d) = deadline {
            if crate::drivers::timer::get_ticks() >= d {
                let mut w = WAITERS.lock();
                // A wake may have just raced with the timeout: if we were removed, we were woken.
                if woken.load(Ordering::SeqCst) {
                    return 0;
                }
                w.retain(|x| !Arc::ptr_eq(&x.woken, &woken));
                return -110; // -ETIMEDOUT
            }
        }
        crate::task::scheduler::sleep_ms(1);
    }
}

fn timeout_to_deadline(ts: u64, absolute: bool) -> Option<u64> {
    if ts == 0 {
        return None;
    }
    let (sec, nsec) = unsafe { (*(ts as *const i64), *((ts + 8) as *const i64)) };
    let ms = (sec.max(0) as u64).saturating_mul(1000).saturating_add((nsec.max(0) as u64 + 999_999) / 1_000_000);
    if absolute {
        Some(ms) // the kernel clock is uptime-based for every clock id
    } else {
        Some(crate::drivers::timer::get_ticks() + ms)
    }
}

pub fn sys_futex(uaddr: u64, op: i32, val: u32, timeout: u64, uaddr2: u64, val3: u32) -> isize {
    if uaddr == 0 || uaddr & 3 != 0 {
        return -14; // -EFAULT
    }
    match op & 0x7F {
        FUTEX_WAIT => wait(uaddr, val, timeout_to_deadline(timeout, false)),
        FUTEX_WAIT_BITSET => wait(uaddr, val, timeout_to_deadline(timeout, true)),
        FUTEX_WAKE | FUTEX_WAKE_BITSET => wake(uaddr, val as usize) as isize,
        FUTEX_REQUEUE => {
            let woken = wake(uaddr, val as usize);
            (woken + requeue(uaddr, uaddr2, timeout as usize)) as isize
        }
        FUTEX_CMP_REQUEUE => {
            if unsafe { core::ptr::read_volatile(uaddr as *const u32) } != val3 {
                return -11; // -EAGAIN
            }
            let woken = wake(uaddr, val as usize);
            (woken + requeue(uaddr, uaddr2, timeout as usize)) as isize
        }
        FUTEX_WAKE_OP => {
            // val3 = op<<28 | cmp<<24 | oparg<<12 | cmparg; applied to *uaddr2.
            let (opc, cmp) = ((val3 >> 28) & 0xF, (val3 >> 24) & 0xF);
            let sign_ext = |v: u32| ((v << 20) as i32 >> 20) as i32;
            let (mut oparg, cmparg) = (sign_ext((val3 >> 12) & 0xFFF), sign_ext(val3 & 0xFFF));
            if opc & 8 != 0 {
                oparg = 1 << (oparg & 31); // FUTEX_OP_OPARG_SHIFT
            }
            let p = uaddr2 as *mut i32;
            let old = unsafe { core::ptr::read_volatile(p) };
            let new = match opc & 7 {
                0 => oparg,        // SET
                1 => old.wrapping_add(oparg), // ADD
                2 => old | oparg,  // OR
                3 => old & !oparg, // ANDN
                4 => old ^ oparg,  // XOR
                _ => return -38,
            };
            unsafe { core::ptr::write_volatile(p, new) };
            let mut woken = wake(uaddr, val as usize);
            let hit = match cmp {
                0 => old == cmparg,
                1 => old != cmparg,
                2 => old < cmparg,
                3 => old <= cmparg,
                4 => old > cmparg,
                5 => old >= cmparg,
                _ => return -38,
            };
            if hit {
                woken += wake(uaddr2, timeout as usize);
            }
            woken as isize
        }
        _ => -38, // -ENOSYS (priority-inheritance futexes, ...)
    }
}
