#[macro_use]
pub mod serial;
pub mod acpi;
pub mod apic;
pub mod fpu;
pub mod gdt;
pub mod idt;
pub mod io;
pub mod pic;
pub mod smp;
pub mod syscall;

pub fn init() {
    fpu::init();
    serial::init();
    gdt::init();
    idt::init();
    pic::init();
}

pub fn hlt_loop() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}
