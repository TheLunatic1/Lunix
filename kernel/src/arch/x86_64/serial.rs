use core::fmt;
use spin::Mutex;

const RX_BUFFER_SIZE: usize = 4096;

struct SerialQueue {
    buffer: [u8; RX_BUFFER_SIZE],
    head: usize,
    tail: usize,
}

static RX_QUEUE: Mutex<SerialQueue> = Mutex::new(SerialQueue {
    buffer: [0; RX_BUFFER_SIZE],
    head: 0,
    tail: 0,
});

pub fn init() {
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
        // Enable auxiliary output 2 (OUT2), RTS, DTR
        crate::arch::x86_64::io::outb(0x3FC, 0x0B);
        // Enable Received Data Available interrupt in IER (port 0x3F9, bit 0 = 1)
        crate::arch::x86_64::io::outb(0x3F9, 0x01);
    }
}

pub fn poll_hardware() {
    x86_64::instructions::interrupts::without_interrupts(|| {
        unsafe {
            while (crate::arch::x86_64::io::inb(0x3FD) & 1) != 0 {
                let b = crate::arch::x86_64::io::inb(0x3F8);
                let mut queue = RX_QUEUE.lock();
                let next_head = (queue.head + 1) % RX_BUFFER_SIZE;
                if next_head != queue.tail {
                    let h = queue.head;
                    queue.buffer[h] = b;
                    queue.head = next_head;
                }
            }
        }
    });
}

pub fn pop_byte() -> Option<u8> {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let mut queue = RX_QUEUE.lock();
        if queue.head != queue.tail {
            let b = queue.buffer[queue.tail];
            queue.tail = (queue.tail + 1) % RX_BUFFER_SIZE;
            Some(b)
        } else {
            None
        }
    })
}

pub fn drain_input<F: FnMut(u8)>(mut handler: F) {
    poll_hardware();
    while let Some(b) = pop_byte() {
        handler(b);
    }
}

pub fn write_byte(byte: u8) {
    x86_64::instructions::interrupts::without_interrupts(|| {
        unsafe {
            for _ in 0..100_000 {
                while (crate::arch::x86_64::io::inb(0x3FD) & 1) != 0 {
                    let rx_b = crate::arch::x86_64::io::inb(0x3F8);
                    let mut queue = RX_QUEUE.lock();
                    let next_head = (queue.head + 1) % RX_BUFFER_SIZE;
                    if next_head != queue.tail {
                        let h = queue.head;
                        queue.buffer[h] = rx_b;
                        queue.head = next_head;
                    }
                }
                let lsr = crate::arch::x86_64::io::inb(0x3FD);
                if (lsr & 0x20) != 0 {
                    break;
                }
                core::hint::spin_loop();
            }
            crate::arch::x86_64::io::outb(0x3F8, byte);
        }
    });
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
        for byte in s.bytes() {
            unsafe {
                for _ in 0..100_000 {
                    while (crate::arch::x86_64::io::inb(0x3FD) & 1) != 0 {
                        let rx_b = crate::arch::x86_64::io::inb(0x3F8);
                        let mut queue = RX_QUEUE.lock();
                        let next_head = (queue.head + 1) % RX_BUFFER_SIZE;
                        if next_head != queue.tail {
                            let h = queue.head;
                            queue.buffer[h] = rx_b;
                            queue.head = next_head;
                        }
                    }
                    let lsr = crate::arch::x86_64::io::inb(0x3FD);
                    if (lsr & 0x20) != 0 {
                        break;
                    }
                    core::hint::spin_loop();
                }
                crate::arch::x86_64::io::outb(0x3F8, byte);
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
