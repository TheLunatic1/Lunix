//! Symmetric Multiprocessing (SMP) Subsystem

pub mod percpu;

use core::sync::atomic::{AtomicU32, Ordering};
use crate::arch::x86_64::acpi;
use crate::arch::x86_64::apic::lapic;

pub static CPU_COUNT: AtomicU32 = AtomicU32::new(1);
pub static AP_ONLINE_COUNT: AtomicU32 = AtomicU32::new(0);

const AP_TRAMPOLINE_PHYS: u64 = 0x8000;
const AP_STACK_SIZE: usize = 4096 * 8; // 32KB stack per AP

#[repr(align(4096))]
struct ApStack([u8; AP_STACK_SIZE]);
static mut AP_STACKS: [ApStack; 16] = [
    ApStack([0; AP_STACK_SIZE]), ApStack([0; AP_STACK_SIZE]),
    ApStack([0; AP_STACK_SIZE]), ApStack([0; AP_STACK_SIZE]),
    ApStack([0; AP_STACK_SIZE]), ApStack([0; AP_STACK_SIZE]),
    ApStack([0; AP_STACK_SIZE]), ApStack([0; AP_STACK_SIZE]),
    ApStack([0; AP_STACK_SIZE]), ApStack([0; AP_STACK_SIZE]),
    ApStack([0; AP_STACK_SIZE]), ApStack([0; AP_STACK_SIZE]),
    ApStack([0; AP_STACK_SIZE]), ApStack([0; AP_STACK_SIZE]),
    ApStack([0; AP_STACK_SIZE]), ApStack([0; AP_STACK_SIZE]),
];

#[repr(C, packed)]
struct TrampolineParams {
    cr3: u64,
    entry64: u64,
    stack_ptr: u64,
    ready_flag: u32,
}

pub fn init() {
    lunix_serial_println!("[smp] Initializing Symmetric Multiprocessing (SMP)...");

    let madt = match acpi::get_madt() {
        Some(m) => m,
        None => {
            lunix_serial_println!("  [SMP] No MADT found, single-core mode active.");
            return;
        }
    };

    let bsp_apic_id = lapic::id();
    let total_procs = madt.processors.len();
    lunix_serial_println!("  [SMP] Detected {} total logical processor(s). BSP APIC ID: {}", total_procs, bsp_apic_id);

    if total_procs <= 1 {
        lunix_serial_println!("  [SMP] Running in uniprocessor mode (1 CPU).");
        return;
    }

    // Map trampoline low memory (0x8000)
    let _ = crate::mm::vmm::map_mmio_range(x86_64::PhysAddr::new(AP_TRAMPOLINE_PHYS), 8192);

    // Set up trampoline payload at 0x8000
    prepare_trampoline();

    // Get current PML4 CR3
    let (pml4_frame, _) = x86_64::registers::control::Cr3::read();
    let pml4_addr = pml4_frame.start_address().as_u64();

    let mut ap_index = 0;
    for proc in madt.processors.iter() {
        if proc.apic_id == bsp_apic_id || !proc.is_enabled() {
            continue;
        }

        if ap_index >= 16 {
            break;
        }

        lunix_serial_println!("  [SMP] Booting AP (CPU Core #{} / APIC ID {})...", ap_index + 1, proc.apic_id);
        boot_ap(proc.apic_id, ap_index, pml4_addr);
        ap_index += 1;
    }

    let online = AP_ONLINE_COUNT.load(Ordering::SeqCst);
    CPU_COUNT.store(online + 1, Ordering::SeqCst);
    lunix_serial_println!("[smp] SMP initialization complete. Active CPU cores: {}", online + 1);
}

fn prepare_trampoline() {
    let trampoline_code: &[u8] = &[
        // [0x8000] 16-bit Real Mode Entry
        0xFA,                               // cli
        0x31, 0xC0,                         // xor ax, ax
        0x8E, 0xD8,                         // mov ds, ax
        0x8E, 0xC0,                         // mov es, ax
        0x8E, 0xD0,                         // mov ss, ax
        0xBC, 0x00, 0x70,                   // mov sp, 0x7000
        // [0x800B] 32-bit lgdt [0x8080]
        0x66, 0x0F, 0x01, 0x16, 0x80, 0x80, // lgdt [0x8080]
        // [0x8011] Enable PE in CR0
        0x0F, 0x20, 0xC0,                   // mov eax, cr0
        0x0C, 0x01,                         // or al, 1
        0x0F, 0x22, 0xC0,                   // mov cr0, eax
        // [0x8018] Far jump to 32-bit Protected Mode (offset 0x8020, selector 0x08)
        0xEA, 0x20, 0x80, 0x08, 0x00,       // jmp 08:8020 (16-bit far jmp: 5 bytes)
        // [0x801D] 3 NOPs to pad exactly to 0x8020
        0x90, 0x90, 0x90,
        // [0x8020] 32-bit Protected Mode
        0xB8, 0x10, 0x00, 0x00, 0x00,       // mov eax, 0x10 (32-bit Data selector)
        0x8E, 0xD8,                         // mov ds, eax
        0x8E, 0xC0,                         // mov es, eax
        0x8E, 0xD0,                         // mov ss, eax
        // Enable PAE in CR4
        0x0F, 0x20, 0xE0,                   // mov eax, cr4
        0x0D, 0x20, 0x00, 0x00, 0x00,       // or eax, 0x20 (PAE bit 5)
        0x0F, 0x22, 0xE0,                   // mov cr4, eax
        // Load CR3 with PML4 address from params at 0x8F00
        0xA1, 0x00, 0x8F, 0x00, 0x00,       // mov eax, [0x8F00]
        0x0F, 0x22, 0xD8,                   // mov cr3, eax
        // Enable Long Mode (LME) in EFER MSR (0xC0000080)
        0xB9, 0x80, 0x00, 0x00, 0xC0,       // mov ecx, 0xC0000080
        0x0F, 0x32,                         // rdmsr
        0x0F, 0xBA, 0xE8, 0x08,             // bts eax, 8 (LME bit 8)
        0x0F, 0x30,                         // wrmsr
        // Enable Paging (PG) in CR0
        0x0F, 0x20, 0xC0,                   // mov eax, cr0
        0x0D, 0x00, 0x00, 0x00, 0x80,       // or eax, 0x80000000 (PG bit 31)
        0x0F, 0x22, 0xC0,                   // mov cr0, eax
        // [0x8051] Far jump to 64-bit Long Mode (offset 0x8060, selector 0x18)
        0xEA, 0x60, 0x80, 0x00, 0x00, 0x18, 0x00, // jmp 0018:00008060 (32-bit far jmp: 7 bytes)
        // [0x8058] 8 NOPs to pad to 0x8060
        0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90,
        // [0x8060] 64-bit Long Mode
        // Load RSP from params at 0x8F10
        0x48, 0x8B, 0x24, 0x25, 0x10, 0x8F, 0x00, 0x00, // mov rsp, [0x8F10]
        // Load 64-bit entry address from params at 0x8F08 into RAX and jump
        0x48, 0x8B, 0x04, 0x25, 0x08, 0x8F, 0x00, 0x00, // mov rax, [0x8F08]
        0xFF, 0xE0,                                     // jmp rax
    ];

    unsafe {
        let dest = AP_TRAMPOLINE_PHYS as *mut u8;
        core::ptr::copy_nonoverlapping(trampoline_code.as_ptr(), dest, trampoline_code.len());

        // Place GDT Pointer at 0x8080: limit (2 bytes) = 31, base (4 bytes) = 0x8088
        let gdt_ptr = (0x8080) as *mut u16;
        *gdt_ptr = 31;
        *((0x8082) as *mut u32) = 0x8088;

        // Place GDT Descriptors at 0x8088:
        let gdt = (0x8088) as *mut u64;
        *gdt.add(0) = 0x0000_0000_0000_0000; // Null
        *gdt.add(1) = 0x00CF_9A00_0000_FFFF; // 32-bit Code (0x08)
        *gdt.add(2) = 0x00CF_9200_0000_FFFF; // 32-bit Data (0x10)
        *gdt.add(3) = 0x0020_9A00_0000_0000; // 64-bit Code (0x18)
    }
}

fn boot_ap(apic_id: u8, ap_index: usize, pml4_addr: u64) {
    let stack_top = unsafe {
        let stack_ptr = AP_STACKS[ap_index].0.as_mut_ptr();
        stack_ptr.add(AP_STACK_SIZE) as u64
    };

    let params_ptr = (0x8F00) as *mut TrampolineParams;
    unsafe {
        (*params_ptr).cr3 = pml4_addr;
        (*params_ptr).entry64 = ap_entry as *const () as u64;
        (*params_ptr).stack_ptr = stack_top;
        (*params_ptr).ready_flag = 0;
    }

    // 1. Send INIT IPI (Assert)
    unsafe {
        lapic::send_ipi(apic_id, 0x500, 0, 1 << 14, 0); // INIT, assert
    }

    // Short pause
    for _ in 0..5_000 {
        core::hint::spin_loop();
    }

    // 2. Send Start-up IPI (SIPI) pointing to page 0x08 (0x8000)
    unsafe {
        lapic::send_ipi(apic_id, 0x600, 0x08, 1 << 14, 0);
    }

    // Short pause
    for _ in 0..2_000 {
        core::hint::spin_loop();
    }

    // Check if AP came online
    let mut online = false;
    for _ in 0..10_000 {
        if unsafe { (*params_ptr).ready_flag == 1 } {
            online = true;
            break;
        }
        for _ in 0..100 {
            core::hint::spin_loop();
        }
    }

    if !online {
        // Retry SIPI once
        unsafe {
            lapic::send_ipi(apic_id, 0x600, 0x08, 1 << 14, 0);
        }
        for _ in 0..10_000 {
            if unsafe { (*params_ptr).ready_flag == 1 } {
                online = true;
                break;
            }
            for _ in 0..100 {
                core::hint::spin_loop();
            }
        }
    }

    if online {
        lunix_serial_println!("  [SMP-AP] Application Processor (APIC ID: {}) successfully booted and idling.", apic_id);
    }
}

pub extern "sysv64" fn ap_entry() -> ! {
    // 1. Initialize CPU tables and syscall MSRs on AP
    unsafe {
        crate::arch::x86_64::gdt::init();
        crate::arch::x86_64::idt::init();
        crate::arch::x86_64::fpu::init();
        crate::arch::x86_64::syscall::init();

        // 2. Initialize Local APIC on AP
        if let Some(madt) = acpi::get_madt() {
            lapic::init(madt.local_apic_address);
        }

        // 3. Signal BSP that AP is ready
        let params_ptr = (0x8F00) as *mut TrampolineParams;
        (*params_ptr).ready_flag = 1;

        AP_ONLINE_COUNT.fetch_add(1, Ordering::SeqCst);
    }

    // Enable interrupts on this AP
    x86_64::instructions::interrupts::enable();

    crate::arch::x86_64::hlt_loop()
}
