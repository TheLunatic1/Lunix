use core::fmt;
use spin::Mutex;
use uart_16550::SerialPort;

pub static SERIAL1: Mutex<Option<SerialPort>> = Mutex::new(None);

pub fn init() {
    let mut serial_port = unsafe { SerialPort::new(0x3F8) };
    serial_port.init();
    *SERIAL1.lock() = Some(serial_port);
}

pub struct DirectSerialWriter;

impl fmt::Write for DirectSerialWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            unsafe {
                crate::arch::x86_64::io::outb(0x3F8, byte);
            }
        }
        Ok(())
    }
}

pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    let mut writer = DirectSerialWriter;
    let _ = writer.write_fmt(args);
}

pub fn write_str(s: &str) {
    for byte in s.bytes() {
        unsafe {
            crate::arch::x86_64::io::outb(0x3F8, byte);
        }
    }
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
