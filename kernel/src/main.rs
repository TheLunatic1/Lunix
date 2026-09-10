#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![allow(dead_code)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

extern crate alloc;

#[macro_use]
pub mod arch;
#[macro_use]
pub mod display;
pub mod drivers;
pub mod mm;
pub mod subsystems;

use core::panic::PanicInfo;
use lunix_common::BootInfo;

const KERNEL_STACK_SIZE: usize = 4096 * 16; // 64 KiB dedicated kernel stack

#[repr(align(4096))]
struct KernelStack([u8; KERNEL_STACK_SIZE]);

static mut KERNEL_STACK: KernelStack = KernelStack([0; KERNEL_STACK_SIZE]);

#[no_mangle]
#[unsafe(naked)]
pub unsafe extern "sysv64" fn _start(_boot_info: &'static BootInfo) -> ! {
    core::arch::naked_asm!(
        "lea rsp, [rip + {stack} + {size}]",
        "and rsp, -16",
        "call {kmain}",
        "2:",
        "hlt",
        "jmp 2b",
        stack = sym KERNEL_STACK,
        size = const KERNEL_STACK_SIZE,
        kmain = sym kmain,
    );
}

pub extern "sysv64" fn kmain(boot_info: &'static BootInfo) -> ! {
    // 0. Ensure interrupts are disabled and FPU/SSE are enabled
    x86_64::instructions::interrupts::disable();
    arch::x86_64::fpu::init();

    // 1. Initialize Serial COM1 port for immediate debugging output
    arch::x86_64::serial::init();
    lunix_serial_println!("=======================================================");
    lunix_serial_println!("        LUNIX OS KERNEL (x86_64 Long Mode)            ");
    lunix_serial_println!("=======================================================");

    // 2. Initialize Graphical Display & Console
    display::init(boot_info.framebuffer);

    lunix_println!("  _                 _        ____   _____ ");
    lunix_println!(" | |   _   _ _ __  (_)_  __ / __ \\ / ____|");
    lunix_println!(" | |  | | | | '_ \\ | \\ \\/ /| |  | | (___  ");
    lunix_println!(" | |__| |_| | | | || |>  < | |__| |\\___ \\ ");
    lunix_println!(" |_____\\__,_|_| |_||_/_/\\_\\ \\____/ |_____/");
    lunix_println!("");
    lunix_println!(" [+] Lunix Kernel v0.1.0 Initializing in 64-bit Long Mode...");

    // 3. Initialize CPU Descriptors (GDT, TSS, IDT, PIC)
    lunix_serial_println!("[kmain] Initializing GDT...");
    arch::x86_64::gdt::init();

    lunix_serial_println!("[kmain] Initializing IDT...");
    arch::x86_64::idt::init();

    lunix_serial_println!("[kmain] Initializing PIC...");
    arch::x86_64::pic::init();

    lunix_println!("[+] Initialized CPU Tables (GDT, TSS, IDT, PIC).");

    // 4. Initialize Memory Subsystem (PMM, VMM, Heap)
    arch::x86_64::serial::write_str("[kmain] Step 4 reached\n");
    lunix_serial_println!("[kmain] Initializing Memory Subsystem...");
    arch::x86_64::serial::write_str("[kmain] Calling mm::init\n");
    mm::init(boot_info);
    arch::x86_64::serial::write_str("[kmain] mm::init returned\n");

    arch::x86_64::serial::write_str("[kmain] Step 5: Drivers init\n");
    drivers::init();
    arch::x86_64::serial::write_str("[kmain] Drivers init done\n");

    arch::x86_64::serial::write_str("[kmain] Step 6: Subsystems init\n");
    subsystems::init();
    arch::x86_64::serial::write_str("[kmain] Subsystems init done\n");

    arch::x86_64::serial::write_str("[kmain] Step 7: Enabling Interrupts\n");
    x86_64::instructions::interrupts::enable();
    arch::x86_64::serial::write_str("[kmain] Interrupts enabled!\n");

    lunix_println!("");
    lunix_println!("===============================================================");
    lunix_println!("  LUNIX READY. Type on your keyboard to test input interactivity!");
    lunix_println!("===============================================================");
    lunix_print!("lunix> ");

    arch::x86_64::serial::write_str("[kmain] Entering HLT loop\n");
    arch::x86_64::hlt_loop()
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    lunix_serial_println!("\n[KERNEL PANIC] {}", info);
    if let Some(mut guard) = display::console::CONSOLE.try_lock() {
        if let Some(ref mut console) = *guard {
            use core::fmt::Write;
            let _ = write!(console, "\n[KERNEL PANIC] {}\n", info);
        }
    }
    arch::x86_64::hlt_loop()
}
