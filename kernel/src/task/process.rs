use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::Mutex;

pub const MAX_FD: usize = 64;

#[derive(Clone)]
pub struct FileDescriptor {
    pub path: String,
    pub offset: usize,
    pub flags: u32,
}

pub struct Process {
    pub id: usize,
    pub name: String,
    pub threads: Vec<usize>,
    pub cr3: u64,
    pub cwd: String,
    pub fds: [Option<Arc<Mutex<FileDescriptor>>>; MAX_FD],
}

impl Process {
    pub fn new_kernel() -> Self {
        const INIT_FD: Option<Arc<Mutex<FileDescriptor>>> = None;
        let (cr3_frame, _) = x86_64::registers::control::Cr3::read();
        
        Self {
            id: 0,
            name: String::from("kernel"),
            threads: Vec::new(),
            cr3: cr3_frame.start_address().as_u64(),
            cwd: String::from("/"),
            fds: [INIT_FD; MAX_FD],
        }
    }

    pub fn new_user(id: usize, name: &str, cr3: u64) -> Self {
        const INIT_FD: Option<Arc<Mutex<FileDescriptor>>> = None;
        Self {
            id,
            name: String::from(name),
            threads: Vec::new(),
            cr3,
            cwd: String::from("/"),
            fds: [INIT_FD; MAX_FD],
        }
    }

    pub fn allocate_fd(&mut self, desc: FileDescriptor) -> Option<usize> {
        for (i, slot) in self.fds.iter_mut().enumerate() {
            if slot.is_none() {
                *slot = Some(Arc::new(Mutex::new(desc)));
                return Some(i);
            }
        }
        None
    }

    pub fn get_fd(&self, fd: usize) -> Option<Arc<Mutex<FileDescriptor>>> {
        if fd < MAX_FD {
            self.fds[fd].clone()
        } else {
            None
        }
    }

    pub fn close_fd(&mut self, fd: usize) -> bool {
        if fd < MAX_FD && self.fds[fd].is_some() {
            self.fds[fd] = None;
            true
        } else {
            false
        }
    }
}
