use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::Mutex;

pub const MAX_FD: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FdKind {
    Stdin,
    Stdout,
    Stderr,
    File,
    PipeRead,
    PipeWrite,
    Socket(usize),
}

#[derive(Clone)]
pub struct FileDescriptor {
    pub kind: FdKind,
    pub path: String,
    pub offset: usize,
    pub flags: u32,
}

pub struct Process {
    pub id: usize,
    pub ppid: usize,
    pub name: String,
    pub threads: Vec<usize>,
    pub children: Vec<usize>,
    pub cr3: u64,
    pub cwd: String,
    pub is_alive: bool,
    pub exit_code: Option<i32>,
    pub fds: [Option<Arc<Mutex<FileDescriptor>>>; MAX_FD],
}

impl Process {
    pub fn new_kernel() -> Self {
        const INIT_FD: Option<Arc<Mutex<FileDescriptor>>> = None;
        let (cr3_frame, _) = x86_64::registers::control::Cr3::read();
        
        let mut proc = Self {
            id: 0,
            ppid: 0,
            name: String::from("kernel"),
            threads: Vec::new(),
            children: Vec::new(),
            cr3: cr3_frame.start_address().as_u64(),
            cwd: String::from("/"),
            is_alive: true,
            exit_code: None,
            fds: [INIT_FD; MAX_FD],
        };
        proc.init_std_fds();
        proc
    }

    pub fn new_user(id: usize, ppid: usize, name: &str, cr3: u64) -> Self {
        const INIT_FD: Option<Arc<Mutex<FileDescriptor>>> = None;
        let mut proc = Self {
            id,
            ppid,
            name: String::from(name),
            threads: Vec::new(),
            children: Vec::new(),
            cr3,
            cwd: String::from("/"),
            is_alive: true,
            exit_code: None,
            fds: [INIT_FD; MAX_FD],
        };
        proc.init_std_fds();
        proc
    }

    pub fn init_std_fds(&mut self) {
        self.fds[0] = Some(Arc::new(Mutex::new(FileDescriptor {
            kind: FdKind::Stdin,
            path: String::from("/dev/stdin"),
            offset: 0,
            flags: 0,
        })));
        self.fds[1] = Some(Arc::new(Mutex::new(FileDescriptor {
            kind: FdKind::Stdout,
            path: String::from("/dev/stdout"),
            offset: 0,
            flags: 0,
        })));
        self.fds[2] = Some(Arc::new(Mutex::new(FileDescriptor {
            kind: FdKind::Stderr,
            path: String::from("/dev/stderr"),
            offset: 0,
            flags: 0,
        })));
    }

    pub fn clone_process(&self, new_pid: usize) -> Self {
        let mut new_proc = Self::new_user(new_pid, self.id, &self.name, self.cr3);
        new_proc.cwd = self.cwd.clone();
        for i in 0..MAX_FD {
            if let Some(ref fd) = self.fds[i] {
                let lock = fd.lock();
                new_proc.fds[i] = Some(Arc::new(Mutex::new(lock.clone())));
            }
        }
        new_proc
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

    pub fn dup_fd(&mut self, oldfd: usize, newfd: usize) -> Option<usize> {
        if oldfd >= MAX_FD || newfd >= MAX_FD {
            return None;
        }
        if let Some(ref src) = self.fds[oldfd] {
            let clone = Arc::new(Mutex::new(src.lock().clone()));
            self.fds[newfd] = Some(clone);
            Some(newfd)
        } else {
            None
        }
    }
}

