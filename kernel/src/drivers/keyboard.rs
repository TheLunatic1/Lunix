use crate::arch::x86_64::io::{inb, outb};
use crate::mm::pmm;
use crate::{lunix_print, lunix_println};
use alloc::string::String;
use pc_keyboard::{layouts, DecodedKey, HandleControl, Keyboard, ScancodeSet1};
use spin::Mutex;

const KEYBOARD_DATA_PORT: u16 = 0x60;

pub static KEYBOARD: Mutex<Option<Keyboard<layouts::Us104Key, ScancodeSet1>>> = Mutex::new(None);
pub static COMMAND_BUFFER: Mutex<String> = Mutex::new(String::new());

pub fn init() {
    let keyboard = Keyboard::new(
        ScancodeSet1::new(),
        layouts::Us104Key,
        HandleControl::Ignore,
    );
    *KEYBOARD.lock() = Some(keyboard);
}

fn execute_command(cmd: &str) {
    let trimmed = cmd.trim();
    if trimmed.is_empty() {
        return;
    }

    match trimmed {
        "help" => {
            lunix_println!("Available Lunix OS Commands:");
            lunix_println!("  help     - Show this help menu");
            lunix_println!("  info     - Display OS and CPU architecture information");
            lunix_println!("  mem      - Display physical memory & heap statistics");
            lunix_println!("  pci      - Scan and list hardware PCI bus devices");
            lunix_println!("  nt       - Display Windows NT Driver Subsystem status");
            lunix_println!("  clear    - Clear console screen buffer");
            lunix_println!("  reboot   - Soft reboot machine via 8042 controller");
            lunix_println!("  panic    - Trigger a kernel panic diagnostic test");
        }
        "info" => {
            lunix_println!("=======================================================");
            lunix_println!("  Lunix OS Kernel v0.1.0");
            lunix_println!("  Architecture : x86_64 (64-bit Long Mode)");
            lunix_println!("  Boot Protocol: Pure Rust Custom UEFI Bootloader (GOP)");
            lunix_println!("  Subsystems   : PMM, VMM, 16MB Heap, PCI, Win64 WDM Subsystem");
            lunix_println!("=======================================================");
        }
        "mem" => {
            let (total_bytes, usable_bytes, used_bytes) = pmm::get_memory_stats();
            let total_mb = total_bytes / (1024 * 1024);
            let usable_mb = usable_bytes / (1024 * 1024);
            let used_mb = used_bytes / (1024 * 1024);
            let free_mb = usable_mb.saturating_sub(used_mb);

            lunix_println!("Physical Memory Breakdown:");
            lunix_println!("  Total RAM     : {} MB", total_mb);
            lunix_println!("  Usable RAM    : {} MB", usable_mb);
            lunix_println!("  Allocated RAM : {} MB", used_mb);
            lunix_println!("  Free RAM      : {} MB", free_mb);
            lunix_println!("  Kernel Heap   : 16 MB (Dynamic Allocator)");
        }
        "pci" => {
            crate::drivers::pci::scan_bus();
        }
        "nt" => {
            lunix_println!("Windows NT Subsystem Status:");
            lunix_println!("  Core API Exports: ntoskrnl.exe & hal.dll (extern \"win64\" ABI)");
            lunix_println!("  Loaded Drivers  : \\Driver\\SampleLunixDriver (WDM)");
            lunix_println!("  Created Devices : \\Device\\LunixSampleDevice0");
            lunix_println!("  IRP Support     : MJ_CREATE, MJ_CLOSE, MJ_DEVICE_CONTROL");
        }
        "clear" => {
            if let Some(ref mut console) = *crate::display::console::CONSOLE.lock() {
                console.framebuffer.clear(console.bg_color);
                console.cursor_x = 0;
                console.cursor_y = 0;
                console.draw_header();
            }
        }
        "reboot" => {
            lunix_println!("Rebooting system...");
            unsafe {
                let mut good: u8 = 0x02;
                while (good & 0x02) != 0 {
                    good = inb(0x64);
                }
                outb(0x64, 0xFE);
            }
        }
        "panic" => {
            panic!("User-requested test kernel panic from Lunix shell prompt!");
        }
        unknown => {
            lunix_println!("Unknown command: '{}'. Type 'help' for available commands.", unknown);
        }
    }
}

pub fn on_interrupt() {
    let scancode = unsafe { inb(KEYBOARD_DATA_PORT) };

    let mut keyboard_lock = KEYBOARD.lock();
    if let Some(ref mut keyboard) = *keyboard_lock {
        if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
            if let Some(key) = keyboard.process_keyevent(key_event) {
                match key {
                    DecodedKey::Unicode(character) => {
                        let mut buffer = COMMAND_BUFFER.lock();
                        match character {
                            '\n' => {
                                lunix_println!("");
                                let cmd = buffer.clone();
                                buffer.clear();
                                drop(buffer);

                                execute_command(&cmd);
                                lunix_print!("lunix> ");
                            }
                            '\x08' => {
                                // Backspace
                                if !buffer.is_empty() {
                                    buffer.pop();
                                    lunix_print!("\x08");
                                }
                            }
                            ch => {
                                buffer.push(ch);
                                lunix_print!("{}", ch);
                            }
                        }
                    }
                    DecodedKey::RawKey(key) => {
                        let _ = key;
                    }
                }
            }
        }
    }
}
