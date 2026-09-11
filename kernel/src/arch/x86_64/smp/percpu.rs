//! Per-CPU Local Storage and Core Data Structures

use crate::arch::x86_64::apic::lapic;

#[derive(Debug, Clone, Copy)]
pub struct PerCpuData {
    pub cpu_id: u32,
    pub apic_id: u8,
    pub is_bsp: bool,
    pub current_thread_id: u64,
}

impl PerCpuData {
    pub const fn empty() -> Self {
        Self {
            cpu_id: 0,
            apic_id: 0,
            is_bsp: false,
            current_thread_id: 0,
        }
    }
}

pub static mut PER_CPU_TABLE: [PerCpuData; 16] = [PerCpuData::empty(); 16];

pub fn current_cpu_id() -> u32 {
    let current_apic = lapic::id();
    unsafe {
        for i in 0..16 {
            if PER_CPU_TABLE[i].apic_id == current_apic {
                return PER_CPU_TABLE[i].cpu_id;
            }
        }
    }
    0
}
