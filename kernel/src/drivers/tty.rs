//! Linux TTY line discipline.
//!
//! [`Ldisc`] is the terminal state machine shared by the console (`/dev/tty*`, fd 0) and
//! by pseudo-terminals (`/dev/pts/N`, see `pty.rs`). Keyboard and COM1 bytes are fed to the
//! console instance through [`input_byte`]; userspace reads them through [`read`]. In
//! canonical mode (`ICANON`) input is line-buffered with erase/EOF editing, otherwise bytes
//! are delivered as they arrive. Terminal attributes are a real `struct termios`, readable
//! and writable via `TCGETS`/`TCSETS`.

use alloc::collections::VecDeque;
use spin::Mutex;

pub const NCCS: usize = 19;

// c_lflag
const ISIG: u32 = 0o000001;
const ICANON: u32 = 0o000002;
const ECHO: u32 = 0o000010;

// c_cc indices
const VERASE: usize = 2;
const VEOF: usize = 4;

/// Kernel `struct termios` (asm-generic/termbits.h), 36 bytes.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Termios {
    pub c_iflag: u32,
    pub c_oflag: u32,
    pub c_cflag: u32,
    pub c_lflag: u32,
    pub c_line: u8,
    pub c_cc: [u8; NCCS],
}

impl Termios {
    pub const fn default() -> Self {
        let mut c_cc = [0u8; NCCS];
        c_cc[0] = 3; // VINTR  ^C
        c_cc[1] = 28; // VQUIT  ^\
        c_cc[2] = 127; // VERASE DEL
        c_cc[3] = 21; // VKILL  ^U
        c_cc[4] = 4; // VEOF   ^D
        c_cc[6] = 1; // VMIN
        c_cc[8] = 17; // VSTART ^Q
        c_cc[9] = 19; // VSTOP  ^S
        c_cc[10] = 26; // VSUSP  ^Z
        c_cc[12] = 18; // VREPRINT ^R
        c_cc[13] = 15; // VDISCARD ^O
        c_cc[14] = 23; // VWERASE ^W
        c_cc[15] = 22; // VLNEXT ^V
        Termios {
            c_iflag: 0o002400, // ICRNL | IXON
            c_oflag: 0o000005, // OPOST | ONLCR
            c_cflag: 0o002277, // B38400 | CS8 | CREAD | HUPCL
            c_lflag: 0o105073, // ISIG|ICANON|ECHO|ECHOE|ECHOK|ECHOCTL|ECHOKE|IEXTEN
            c_line: 0,
            c_cc,
        }
    }
}

/// Line discipline state of one terminal.
pub struct Ldisc {
    pub termios: Termios,
    /// Line currently being edited (canonical mode only).
    line: VecDeque<u8>,
    /// Bytes ready for `read`.
    ready: VecDeque<u8>,
    /// A VEOF on an empty line: the next read returns 0.
    eof: bool,
}

impl Ldisc {
    pub const fn new() -> Self {
        Ldisc { termios: Termios::default(), line: VecDeque::new(), ready: VecDeque::new(), eof: false }
    }

    /// Feed one input byte. Bytes to echo back to the terminal are written to `echo`;
    /// returns how many.
    pub fn input_byte(&mut self, b: u8, echo: &mut [u8; 3]) -> usize {
        let lflag = self.termios.c_lflag;
        let do_echo = lflag & ECHO != 0;
        let mut n = 0;
        if lflag & ICANON != 0 {
            let cc = self.termios.c_cc;
            if b == b'\n' {
                self.line.push_back(b'\n');
                let line: alloc::vec::Vec<u8> = self.line.drain(..).collect();
                self.ready.extend(line);
                if do_echo {
                    echo[0] = b'\n';
                    n = 1;
                }
            } else if b == 0x08 || b == cc[VERASE] {
                if self.line.pop_back().is_some() && do_echo {
                    *echo = [0x08, b' ', 0x08];
                    n = 3;
                }
            } else if b == cc[VEOF] {
                if self.line.is_empty() {
                    self.eof = true;
                } else {
                    let line: alloc::vec::Vec<u8> = self.line.drain(..).collect();
                    self.ready.extend(line);
                }
            } else if b == cc[0] && lflag & ISIG != 0 {
                // ^C: discard the pending line. Signal delivery to the foreground
                // process group is not implemented yet.
                self.line.clear();
                if do_echo {
                    echo[0] = b'\n';
                    n = 1;
                }
            } else if b >= b' ' || b == b'\t' {
                self.line.push_back(b);
                if do_echo && b != b'\t' {
                    echo[0] = b;
                    n = 1;
                }
            }
        } else {
            self.ready.push_back(b);
            if do_echo && b >= b' ' {
                echo[0] = b;
                n = 1;
            }
        }
        n
    }

    /// True when a `read` would not block (`POLLIN`).
    pub fn has_input(&self) -> bool {
        !self.ready.is_empty() || self.eof
    }

    /// Bytes readable right now (FIONREAD).
    pub fn available(&self) -> usize {
        self.ready.len()
    }

    /// One read attempt: `Some(n)` (n = 0 means end-of-file) or `None` if it would block.
    pub fn try_read(&mut self, buf: &mut [u8]) -> Option<isize> {
        if self.ready.is_empty() {
            if self.eof {
                self.eof = false;
                return Some(0);
            }
            return None;
        }
        let canonical = self.termios.c_lflag & ICANON != 0;
        let mut n = 0;
        while n < buf.len() {
            match self.ready.pop_front() {
                Some(b) => {
                    buf[n] = b;
                    n += 1;
                    if canonical && b == b'\n' {
                        break;
                    }
                }
                None => break,
            }
        }
        Some(n as isize)
    }

    pub fn set_termios(&mut self, new: Termios) {
        let was_canonical = self.termios.c_lflag & ICANON != 0;
        self.termios = new;
        // Leaving canonical mode makes any half-typed line readable.
        if was_canonical && new.c_lflag & ICANON == 0 {
            let line: alloc::vec::Vec<u8> = self.line.drain(..).collect();
            self.ready.extend(line);
        }
    }
}

static TTY: Mutex<Ldisc> = Mutex::new(Ldisc::new());

fn with_tty<R>(f: impl FnOnce(&mut Ldisc) -> R) -> R {
    x86_64::instructions::interrupts::without_interrupts(|| f(&mut TTY.lock()))
}

/// Feed one input byte (from keyboard or serial) through the console's line discipline.
pub fn input_byte(b: u8) {
    // Echo is produced outside the lock: it writes to the console.
    let mut echo = [0u8; 3];
    let n = with_tty(|t| t.input_byte(b, &mut echo));
    if n > 0 {
        if let Ok(s) = core::str::from_utf8(&echo[..n]) {
            crate::lunix_print!("{}", s);
        }
    }
}

/// True when a `read` would not block (`POLLIN`).
pub fn has_input() -> bool {
    with_tty(|t| t.has_input())
}

/// Bytes readable right now (FIONREAD).
pub fn available() -> usize {
    with_tty(|t| t.available())
}

/// Read from the console tty. Blocks until data is available unless `nonblock`, in which
/// case it returns `-EAGAIN` (-11). Returns 0 for end-of-file (VEOF).
pub fn read(buf: &mut [u8], nonblock: bool) -> isize {
    if buf.is_empty() {
        return 0;
    }
    loop {
        if let Some(n) = with_tty(|t| t.try_read(buf)) {
            return n;
        }
        if nonblock {
            return -11;
        }
        crate::task::scheduler::sleep_ms(1);
    }
}

pub fn get_termios() -> Termios {
    with_tty(|t| t.termios)
}

pub fn set_termios(new: Termios) {
    with_tty(|t| t.set_termios(new));
}
