//! Pseudo-terminals (`/dev/ptmx` + `/dev/pts/N`).
//!
//! Opening `/dev/ptmx` allocates a pair and returns the master side. The program driving a
//! terminal emulator (xterm) writes keystrokes to the master and reads the screen output
//! from it; the shell runs on the slave (`/dev/pts/N`, found with `TIOCGPTN`), which behaves
//! like a real terminal because it runs through the same line discipline as the console.

use crate::drivers::tty::{Ldisc, Termios};
use crate::fs::file::{FileHandle, SeekFrom};
use alloc::collections::VecDeque;
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::Mutex;

const ICRNL: u32 = 0o000400;
const OPOST: u32 = 0o000001;
const ONLCR: u32 = 0o000004;

pub struct Pty {
    ldisc: Ldisc,
    /// Bytes the slave wrote (and line-discipline echo), waiting for the master to read.
    to_master: VecDeque<u8>,
    unlocked: bool,
    master_open: bool,
    rows: u16,
    cols: u16,
}

type SharedPty = Arc<Mutex<Pty>>;

static PTYS: Mutex<Vec<SharedPty>> = Mutex::new(Vec::new());

fn with<R>(pty: &SharedPty, f: impl FnOnce(&mut Pty) -> R) -> R {
    x86_64::instructions::interrupts::without_interrupts(|| f(&mut pty.lock()))
}

impl Pty {
    /// Terminal output from the slave side: `\n` becomes `\r\n` under ONLCR.
    fn slave_output(&mut self, buf: &[u8]) {
        let post = self.ldisc.termios.c_oflag & (OPOST | ONLCR) == (OPOST | ONLCR);
        for &b in buf {
            if b == b'\n' && post {
                self.to_master.push_back(b'\r');
            }
            self.to_master.push_back(b);
        }
    }

    /// Keystrokes from the master side, through the line discipline.
    fn master_input(&mut self, buf: &[u8]) {
        for &b in buf {
            let b = if b == b'\r' && self.ldisc.termios.c_iflag & ICRNL != 0 { b'\n' } else { b };
            let mut echo = [0u8; 3];
            let n = self.ldisc.input_byte(b, &mut echo);
            if n > 0 {
                let echoed: Vec<u8> = echo[..n].to_vec();
                self.slave_output(&echoed);
            }
        }
    }
}

/// `TIOCGWINSZ` layout.
#[repr(C)]
struct WinSize {
    ws_row: u16,
    ws_col: u16,
    ws_xpixel: u16,
    ws_ypixel: u16,
}

/// Terminal ioctls common to both ends.
fn tty_ioctl(pty: &SharedPty, req: u64, arg: u64) -> Option<isize> {
    const TCGETS: u64 = 0x5401;
    const TCSETS: u64 = 0x5402;
    const TCSETSW: u64 = 0x5403;
    const TCSETSF: u64 = 0x5404;
    const TCGETS2: u64 = 0x802C542A;
    const TCSETS2: u64 = 0x402C542B;
    const TCSETSW2: u64 = 0x402C542C;
    const TCSETSF2: u64 = 0x402C542D;
    const TIOCSCTTY: u64 = 0x540E;
    const TIOCGPGRP: u64 = 0x540F;
    const TIOCSPGRP: u64 = 0x5410;
    const TIOCGWINSZ: u64 = 0x5413;
    const TIOCSWINSZ: u64 = 0x5414;
    const TIOCNOTTY: u64 = 0x5422;
    const TIOCGSID: u64 = 0x5429;
    const FIONREAD: u64 = 0x541B;

    let fault = if arg == 0 { Some(-14) } else { None };
    Some(match req {
        TCGETS | TCGETS2 => {
            if let Some(e) = fault {
                return Some(e);
            }
            let t = with(pty, |p| p.ldisc.termios);
            unsafe { *(arg as *mut Termios) = t };
            if req == TCGETS2 {
                const B38400: u32 = 15;
                unsafe {
                    let speeds = (arg + 36) as *mut u32;
                    *speeds = B38400;
                    *speeds.add(1) = B38400;
                }
            }
            0
        }
        TCSETS | TCSETSW | TCSETSF | TCSETS2 | TCSETSW2 | TCSETSF2 => {
            if let Some(e) = fault {
                return Some(e);
            }
            let t = unsafe { *(arg as *const Termios) };
            with(pty, |p| p.ldisc.set_termios(t));
            0
        }
        TIOCGWINSZ => {
            if let Some(e) = fault {
                return Some(e);
            }
            let (rows, cols) = with(pty, |p| (p.rows, p.cols));
            unsafe { *(arg as *mut WinSize) = WinSize { ws_row: rows, ws_col: cols, ws_xpixel: 0, ws_ypixel: 0 } };
            0
        }
        TIOCSWINSZ => {
            if let Some(e) = fault {
                return Some(e);
            }
            let ws = unsafe { &*(arg as *const WinSize) };
            with(pty, |p| {
                p.rows = ws.ws_row;
                p.cols = ws.ws_col;
            });
            0
        }
        TIOCGPGRP | TIOCGSID => {
            if let Some(e) = fault {
                return Some(e);
            }
            unsafe { *(arg as *mut i32) = crate::syscall::sys_getpid() as i32 };
            0
        }
        TIOCSPGRP | TIOCSCTTY | TIOCNOTTY => 0,
        FIONREAD => {
            if let Some(e) = fault {
                return Some(e);
            }
            let n = with(pty, |p| p.ldisc.available());
            unsafe { *(arg as *mut i32) = n as i32 };
            0
        }
        _ => return None,
    })
}

/// Master end: what the terminal emulator holds (`/dev/ptmx`).
pub struct PtyMaster {
    idx: usize,
    pty: SharedPty,
}

impl PtyMaster {
    pub fn open() -> Self {
        let mut list = PTYS.lock();
        let pty = Arc::new(Mutex::new(Pty {
            ldisc: Ldisc::new(),
            to_master: VecDeque::new(),
            unlocked: false,
            master_open: true,
            rows: 24,
            cols: 80,
        }));
        list.push(pty.clone());
        PtyMaster { idx: list.len() - 1, pty }
    }
}

impl Drop for PtyMaster {
    fn drop(&mut self) {
        with(&self.pty, |p| p.master_open = false);
    }
}

impl FileHandle for PtyMaster {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, &'static str> {
        loop {
            match self.read_nonblock(buf) {
                Err("EAGAIN") => crate::task::scheduler::sleep_ms(1),
                other => return other,
            }
        }
    }

    fn read_nonblock(&mut self, buf: &mut [u8]) -> Result<usize, &'static str> {
        with(&self.pty, |p| {
            if p.to_master.is_empty() {
                return Err("EAGAIN");
            }
            let n = buf.len().min(p.to_master.len());
            for slot in buf.iter_mut().take(n) {
                *slot = p.to_master.pop_front().unwrap();
            }
            Ok(n)
        })
    }

    fn poll_readable(&self) -> bool {
        with(&self.pty, |p| !p.to_master.is_empty())
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize, &'static str> {
        with(&self.pty, |p| p.master_input(buf));
        Ok(buf.len())
    }

    fn seek(&mut self, _pos: SeekFrom) -> Result<u64, &'static str> {
        Ok(0)
    }

    fn size(&self) -> u64 {
        0
    }

    fn ioctl(&mut self, req: u64, arg: u64) -> Option<isize> {
        const TIOCGPTN: u64 = 0x8004_5430;
        const TIOCSPTLCK: u64 = 0x4004_5431;
        const TIOCGPTLCK: u64 = 0x8004_5439;
        match req {
            TIOCGPTN => {
                if arg == 0 {
                    return Some(-14);
                }
                unsafe { *(arg as *mut u32) = self.idx as u32 };
                Some(0)
            }
            TIOCSPTLCK => {
                if arg == 0 {
                    return Some(-14);
                }
                let lock = unsafe { *(arg as *const i32) } != 0;
                with(&self.pty, |p| p.unlocked = !lock);
                Some(0)
            }
            TIOCGPTLCK => {
                if arg == 0 {
                    return Some(-14);
                }
                let locked = with(&self.pty, |p| !p.unlocked);
                unsafe { *(arg as *mut i32) = locked as i32 };
                Some(0)
            }
            _ => tty_ioctl(&self.pty, req, arg),
        }
    }
}

/// Slave end: the controlling terminal of the program running inside the emulator.
pub struct PtySlave {
    pty: SharedPty,
}

impl PtySlave {
    /// Open `/dev/pts/<idx>`; None if that pty does not exist.
    pub fn open(idx: usize) -> Option<Self> {
        PTYS.lock().get(idx).cloned().map(|pty| PtySlave { pty })
    }
}

impl FileHandle for PtySlave {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, &'static str> {
        loop {
            match self.read_nonblock(buf) {
                Err("EAGAIN") => crate::task::scheduler::sleep_ms(1),
                other => return other,
            }
        }
    }

    fn read_nonblock(&mut self, buf: &mut [u8]) -> Result<usize, &'static str> {
        if buf.is_empty() {
            return Ok(0);
        }
        with(&self.pty, |p| match p.ldisc.try_read(buf) {
            Some(n) => Ok(n as usize),
            // The emulator closed its end: the terminal hung up (EOF).
            None if !p.master_open => Ok(0),
            None => Err("EAGAIN"),
        })
    }

    fn poll_readable(&self) -> bool {
        with(&self.pty, |p| p.ldisc.has_input() || !p.master_open)
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize, &'static str> {
        with(&self.pty, |p| p.slave_output(buf));
        Ok(buf.len())
    }

    fn seek(&mut self, _pos: SeekFrom) -> Result<u64, &'static str> {
        Ok(0)
    }

    fn size(&self) -> u64 {
        0
    }

    fn ioctl(&mut self, req: u64, arg: u64) -> Option<isize> {
        tty_ioctl(&self.pty, req, arg)
    }
}

/// Does `/dev/pts/<idx>` exist?
pub fn exists(idx: usize) -> bool {
    PTYS.lock().get(idx).is_some()
}
