use core::fmt;
use core::sync::atomic::{AtomicUsize, Ordering};
use spin::Mutex;

const RX_BUFFER_SIZE: usize = 8192;
static mut RX_BUFFER: [u8; RX_BUFFER_SIZE] = [0; RX_BUFFER_SIZE];
static RX_HEAD: AtomicUsize = AtomicUsize::new(0);
static RX_TAIL: AtomicUsize = AtomicUsize::new(0);

static SERIAL_PORT_LOCK: Mutex<()> = Mutex::new(());

pub fn init() {
    let iir = x86_64::instructions::interrupts::without_interrupts(|| {
        let _guard = SERIAL_PORT_LOCK.lock();
        unsafe {
            // Disable all interrupts during config
            crate::arch::x86_64::io::outb(0x3F9, 0x00);
            // Enable DLAB (set baud rate divisor)
            crate::arch::x86_64::io::outb(0x3FB, 0x80);
            // Set divisor to 1 (115200 baud)
            crate::arch::x86_64::io::outb(0x3F8, 0x01);
            crate::arch::x86_64::io::outb(0x3F9, 0x00);
            // 8 bits, no parity, one stop bit
            crate::arch::x86_64::io::outb(0x3FB, 0x03);
            // Enable FIFO, clear TX/RX FIFO, 1-byte trigger threshold (0x07)
            crate::arch::x86_64::io::outb(0x3FA, 0x07);
            // Enable auxiliary output 2 (OUT2), RTS, DTR (0x0B)
            crate::arch::x86_64::io::outb(0x3FC, 0x0B);
            // Enable Received Data Available and Line Status interrupts in IER (0x05)
            crate::arch::x86_64::io::outb(0x3F9, 0x05);

            // Flush / Clear any residual status registers
            let _ = crate::arch::x86_64::io::inb(0x3F8);
            let _ = crate::arch::x86_64::io::inb(0x3FD);
            let iir = crate::arch::x86_64::io::inb(0x3FA);
            let _ = crate::arch::x86_64::io::inb(0x3FE);

            iir
        }
    });

    write_str("  [SERIAL] 16550 UART Configured (IIR: ");
    write_hex(iir as u64);
    write_str(")\n");
}

pub fn poll_hardware() {
    x86_64::instructions::interrupts::without_interrupts(|| {
        unsafe {
            while (crate::arch::x86_64::io::inb(0x3FD) & 1) != 0 {
                let byte = crate::arch::x86_64::io::inb(0x3F8);
                let head = RX_HEAD.load(Ordering::Acquire);
                let next_head = (head + 1) % RX_BUFFER_SIZE;
                let tail = RX_TAIL.load(Ordering::Acquire);
                if next_head != tail {
                    core::ptr::write_volatile(&mut RX_BUFFER[head], byte);
                    RX_HEAD.store(next_head, Ordering::Release);
                }
            }
        }
    });
}

#[inline(always)]
pub fn pop_byte() -> Option<u8> {
    let tail = RX_TAIL.load(Ordering::Acquire);
    let head = RX_HEAD.load(Ordering::Acquire);
    if tail != head {
        let b = unsafe { core::ptr::read_volatile(&RX_BUFFER[tail]) };
        RX_TAIL.store((tail + 1) % RX_BUFFER_SIZE, Ordering::Release);
        Some(b)
    } else {
        None
    }
}

pub fn drain_input<F: FnMut(u8)>(mut handler: F) {
    poll_hardware();
    while let Some(b) = pop_byte() {
        handler(b);
    }
}

#[inline]
unsafe fn write_raw_byte_unlocked(byte: u8) {
    poll_hardware();

    // Wait for transmitter holding register empty (THRE)
    while (crate::arch::x86_64::io::inb(0x3FD) & 0x20) == 0 {
        poll_hardware();
        core::hint::spin_loop();
    }

    // Output the byte
    crate::arch::x86_64::io::outb(0x3F8, byte);
}

#[inline]
pub fn write_raw_byte(byte: u8) {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let _guard = SERIAL_PORT_LOCK.lock();
        unsafe {
            write_raw_byte_unlocked(byte);
        }
    });
}

pub fn write_byte(byte: u8) {
    if byte == b'\x08' {
        // Erase character on ANSI/VT100 terminal: backspace, space, backspace
        write_raw_byte(b'\x08');
        write_raw_byte(b' ');
        write_raw_byte(b'\x08');
    } else {
        write_raw_byte(byte);
    }
}

pub struct DirectSerialWriter;

impl fmt::Write for DirectSerialWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        write_str(s);
        Ok(())
    }
}

pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    let mut writer = DirectSerialWriter;
    let _ = writer.write_fmt(args);
}

pub fn write_str(s: &str) {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let _guard = SERIAL_PORT_LOCK.lock();
        unsafe {
            for byte in s.bytes() {
                if byte == b'\x08' {
                    write_raw_byte_unlocked(b'\x08');
                    write_raw_byte_unlocked(b' ');
                    write_raw_byte_unlocked(b'\x08');
                } else {
                    write_raw_byte_unlocked(byte);
                }
            }
        }
    });
}

pub fn write_hex(mut val: u64) {
    let mut buf = [b'0'; 18];
    buf[0] = b'0';
    buf[1] = b'x';
    for i in (0..16).rev() {
        let nibble = (val & 0xF) as u8;
        buf[2 + i] = if nibble < 10 { b'0' + nibble } else { b'A' + (nibble - 10) };
        val >>= 4;
    }
    if let Ok(s) = core::str::from_utf8(&buf) {
        write_str(s);
    }
}

pub fn write_dec(mut val: usize) {
    if val == 0 {
        write_str("0");
        return;
    }
    let mut buf = [0u8; 20];
    let mut i = 0;
    while val > 0 {
        buf[i] = b'0' + (val % 10) as u8;
        val /= 10;
        i += 1;
    }
    buf[..i].reverse();
    if let Ok(s) = core::str::from_utf8(&buf[..i]) {
        write_str(s);
    }
}

#[macro_export]
macro_rules! lunix_serial_print {
    ($($arg:tt)*) => {
        $crate::arch::x86_64::serial::_print(format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! lunix_serial_println {
    () => ($crate::lunix_serial_print!("\n"));
    ($($arg:tt)*) => {{
        $crate::arch::x86_64::serial::_print(format_args!($($arg)*));
        $crate::arch::x86_64::serial::_print(format_args!("\n"));
    }};
}
