use x86_64::structures::paging::PageTableFlags;
use x86_64::VirtAddr;
use alloc::string::String;
use crate::lunix_println;
use spin::Mutex;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Elf64Header {
    pub ident: [u8; 16],
    pub elf_type: u16,
    pub machine: u16,
    pub version: u32,
    pub entry: u64,
    pub phoff: u64,
    pub shoff: u64,
    pub flags: u32,
    pub ehsize: u16,
    pub phentsize: u16,
    pub phnum: u16,
    pub shentsize: u16,
    pub shnum: u16,
    pub shstrndx: u16,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Elf64Phdr {
    pub p_type: u32,
    pub p_flags: u32,
    pub p_offset: u64,
    pub p_vaddr: u64,
    pub p_paddr: u64,
    pub p_filesz: u64,
    pub p_memsz: u64,
    pub p_align: u64,
}

pub const PT_LOAD: u32 = 1;
pub const USER_STACK_BASE: u64 = 0x0000_7FFF_0000_0000;
pub const USER_STACK_SIZE: usize = 64 * 1024; // 64 KiB

#[derive(Debug, Clone, Copy)]
pub struct LoadedProgram {
    pub entry_point: u64,
    pub user_stack_top: u64,
}

static CURRENT_ELF_EXEC: Mutex<Option<LoadedProgram>> = Mutex::new(None);

pub fn load_elf(elf_bytes: &[u8]) -> Result<LoadedProgram, &'static str> {
    if elf_bytes.len() < core::mem::size_of::<Elf64Header>() {
        return Err("Binary too small for ELF64 header");
    }

    let header = unsafe { &*(elf_bytes.as_ptr() as *const Elf64Header) };

    // Validate ELF magic: \x7fELF
    if &header.ident[0..4] != b"\x7fELF" {
        return Err("Invalid ELF magic");
    }

    // Must be 64-bit (2) and x86_64 (0x3E)
    if header.ident[4] != 2 || header.machine != 0x3E {
        return Err("Not a 64-bit x86_64 ELF binary");
    }

    lunix_serial_println!("  [ELF64] Valid ELF found. Entry: 0x{:X}, PhNum: {}", header.entry, header.phnum);

    // Iterate program headers and map PT_LOAD segments
    let phdr_size = header.phentsize as usize;
    for i in 0..header.phnum as usize {
        let phdr_offset = header.phoff as usize + (i * phdr_size);
        if phdr_offset + phdr_size > elf_bytes.len() {
            return Err("Invalid program header offset");
        }

        let phdr = unsafe { &*(elf_bytes.as_ptr().add(phdr_offset) as *const Elf64Phdr) };

        if phdr.p_type == PT_LOAD {
            let vaddr = phdr.p_vaddr;
            let memsz = phdr.p_memsz as usize;
            let filesz = phdr.p_filesz as usize;

            lunix_serial_println!("  [ELF64] PT_LOAD: VAddr=0x{:X}, FileSz={}, MemSz={}", vaddr, filesz, memsz);

            let num_pages = (memsz + 4095) / 4096;
            for p in 0..num_pages {
                let page_vaddr = VirtAddr::new(vaddr + (p * 4096) as u64);
                let phys_frame = crate::mm::pmm::alloc_frame().ok_or("Out of memory for user page")?;

                // Map with USER_ACCESSIBLE | PRESENT | WRITABLE
                let flags = PageTableFlags::PRESENT
                    | PageTableFlags::WRITABLE
                    | PageTableFlags::USER_ACCESSIBLE;

                let _ = crate::mm::vmm::map_page(page_vaddr, phys_frame, flags);

                // Zero-fill the physical frame
                unsafe {
                    core::ptr::write_bytes(phys_frame.as_u64() as *mut u8, 0, 4096);
                }
            }

            // Copy file segment content
            let src_start = phdr.p_offset as usize;
            let src_end = src_start + filesz;
            if src_end <= elf_bytes.len() {
                unsafe {
                    core::ptr::copy_nonoverlapping(
                        elf_bytes.as_ptr().add(src_start),
                        vaddr as *mut u8,
                        filesz,
                    );
                }
            }
        }
    }

    // Allocate and map User Stack (64 KiB)
    let num_stack_pages = USER_STACK_SIZE / 4096;
    let mut stack_top_frame = 0u64;
    for p in 0..num_stack_pages {
        let page_vaddr = VirtAddr::new(USER_STACK_BASE + (p * 4096) as u64);
        let phys_frame = crate::mm::pmm::alloc_frame().ok_or("Out of memory for user stack")?;

        let flags = PageTableFlags::PRESENT
            | PageTableFlags::WRITABLE
            | PageTableFlags::USER_ACCESSIBLE;

        let _ = crate::mm::vmm::map_page(page_vaddr, phys_frame, flags);
        unsafe {
            core::ptr::write_bytes(phys_frame.as_u64() as *mut u8, 0, 4096);
        }
        if p == num_stack_pages - 1 {
            stack_top_frame = phys_frame.as_u64();
        }
    }

    let user_stack_top = USER_STACK_BASE + (USER_STACK_SIZE as u64) - 64;

    // Set up standard Linux initial user stack frame:
    // [RSP + 0]  = argc (0)
    // [RSP + 8]  = argv[0] (NULL)
    // [RSP + 16] = NULL (end of envp)
    // [RSP + 24] = AT_PAGESZ (6)
    // [RSP + 32] = 4096
    // [RSP + 40] = AT_ENTRY (9)
    // [RSP + 48] = header.entry
    // [RSP + 56] = AT_NULL (0)
    unsafe {
        let stack_ptr = (stack_top_frame + 4096 - 64) as *mut u64;
        *stack_ptr.add(0) = 0; // argc = 0
        *stack_ptr.add(1) = 0; // argv[0] = NULL
        *stack_ptr.add(2) = 0; // envp = NULL
        *stack_ptr.add(3) = 6; // AT_PAGESZ
        *stack_ptr.add(4) = 4096;
        *stack_ptr.add(5) = 9; // AT_ENTRY
        *stack_ptr.add(6) = header.entry;
        *stack_ptr.add(7) = 0; // AT_NULL
    }

    Ok(LoadedProgram {
        entry_point: header.entry,
        user_stack_top,
    })
}

fn elf_runner_trampoline() {
    let prog = {
        let lock = CURRENT_ELF_EXEC.lock();
        lock.expect("No ELF program loaded for execution")
    };

    lunix_println!("  [USER] Transitioning CPU to Ring 3 at entry point 0x{:X}...", prog.entry_point);
    unsafe {
        crate::task::user::enter_user_mode(prog.entry_point, prog.user_stack_top);
    }
}

pub fn exec_elf(path: &str) -> Result<usize, String> {
    lunix_println!("[ELF] Loading binary '{}' from VFS...", path);
    let bytes = crate::fs::vfs::read_to_vec(path)
        .map_err(|e| alloc::format!("Failed to read '{}': {:?}", path, e))?;

    let prog = load_elf(&bytes)
        .map_err(|e| alloc::format!("ELF loader error: {}", e))?;

    *CURRENT_ELF_EXEC.lock() = Some(prog);

    let tid = crate::task::scheduler::spawn("elf_process", elf_runner_trampoline, 8);
    lunix_println!("[+] Spawned Ring 3 ELF process thread (TID: {})", tid);
    Ok(tid)
}

