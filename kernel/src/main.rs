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
pub mod fs;
pub mod mm;
pub mod net;
pub mod sync;
pub mod syscall;
pub mod task;

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

    lunix_println!("Lunix kernel {} (x86_64) - Rust bare-metal Linux-compatible kernel", env!("CARGO_PKG_VERSION"));

    // 3. Initialize CPU Descriptors (GDT, TSS, IDT, PIC)
    arch::x86_64::gdt::init();
    arch::x86_64::idt::init();
    arch::x86_64::pic::init();
    lunix_println!("[+] Initialized CPU Tables (GDT, TSS, IDT, PIC).");

    // 4. Initialize Memory Subsystem (PMM, VMM, Heap)
    mm::init(boot_info);
    lunix_println!("[+] Memory Management Subsystem ready (PMM, VMM, 16MB Heap).");

    // 5. Initialize ACPI & Modern APIC / SMP Multi-Core
    arch::x86_64::acpi::init(boot_info);
    arch::x86_64::apic::init();
    arch::x86_64::smp::init();

    let total_cores = arch::x86_64::smp::CPU_COUNT.load(core::sync::atomic::Ordering::SeqCst);
    lunix_println!("[+] ACPI 2.0 & APIC configured. Multi-Core Active: {} Core(s).", total_cores);

    // 6. Initialize Hardware Drivers (Keyboard, PCI, Storage)
    drivers::init();
    lunix_println!("[+] Hardware Drivers initialized (PCI, PIT/APIC Timer, PS/2 Keyboard, Storage).");

    // 7. Initialize VFS and Mount Root Filesystem
    fs::init();

    // 8. Initialize Network Protocol Stack (TCP/IP)
    net::init();

    // 9. Initialize Preemptive Multitasking & Task Scheduler
    task::init();
    lunix_println!("[+] Preemptive Multitasking & Priority Scheduler active.");

    // 10. Initialize Fast MSR Syscall & Ring 3 ABI
    arch::x86_64::syscall::init();
    lunix_println!("[+] Fast MSR Syscall & Ring 3 User Space ABI ready.");

    // 11. Enable interrupts
    x86_64::instructions::interrupts::enable();
    lunix_serial_println!("[kmain] Hardware Interrupts enabled.");

    // 12. Hand control to userspace: exec PID 1 from the root filesystem.
    // The tty flag is set first: init may already be running (and reading the tty)
    // before this thread gets scheduled again.
    drivers::keyboard::set_userspace_tty(true);
    if task::start_init().is_some() {
        // Nothing to print: init already owns the console.
    } else {
        drivers::keyboard::set_userspace_tty(false);
        lunix_println!("[-] No init found on root filesystem; falling back to kernel debug console.");
        drivers::keyboard::print_prompt();
    }

    // The boot thread doubles as the console input pump: it feeds keyboard and
    // serial bytes to the tty line discipline (or the debug console if there is no init).
    drivers::keyboard::run_shell_loop()
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
