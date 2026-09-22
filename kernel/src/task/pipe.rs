//! Inter-Process Communication (IPC) Pipe Subsystem
//!
//! Provides in-memory circular FIFO byte buffers for Linux POSIX pipelines
//! (sys_pipe, sys_pipe2) and Windows Win32 anonymous pipes (CreatePipe).

use alloc::sync::Arc;
use spin::Mutex;

/// Linux default pipe size.
pub const PIPE_BUFFER_CAPACITY: usize = 65536;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipeError {
    WouldBlock,
    BrokenPipe,
    BufferFull,
}

pub struct PipeBuffer {
    buffer: alloc::vec::Vec<u8>,
    head: usize,
    tail: usize,
    count: usize,
    pub readers_count: usize,
    pub writers_count: usize,
    pub closed_read: bool,
    pub closed_write: bool,
}

impl PipeBuffer {
    pub fn new() -> Self {
        Self {
            buffer: alloc::vec![0u8; PIPE_BUFFER_CAPACITY],
            head: 0,
            tail: 0,
            count: 0,
            readers_count: 1,
            writers_count: 1,
            closed_read: false,
            closed_write: false,
        }
    }

    pub fn available_to_read(&self) -> usize {
        self.count
    }

    pub fn available_to_write(&self) -> usize {
        PIPE_BUFFER_CAPACITY - self.count
    }

    pub fn read(&mut self, buf: &mut [u8], non_blocking: bool) -> Result<usize, PipeError> {
        if self.count > 0 {
            let to_read = buf.len().min(self.count);
            for i in 0..to_read {
                buf[i] = self.buffer[self.tail];
                self.tail = (self.tail + 1) % PIPE_BUFFER_CAPACITY;
            }
            self.count -= to_read;
            Ok(to_read)
        } else if self.writers_count == 0 || self.closed_write {
            // EOF: No writers left and buffer is empty
            Ok(0)
        } else if non_blocking {
            Err(PipeError::WouldBlock)
        } else {
            Err(PipeError::WouldBlock)
        }
    }

    pub fn write(&mut self, buf: &[u8], non_blocking: bool) -> Result<usize, PipeError> {
        if self.readers_count == 0 || self.closed_read {
            return Err(PipeError::BrokenPipe);
        }

        let space = PIPE_BUFFER_CAPACITY - self.count;
        if space == 0 {
            if non_blocking {
                return Err(PipeError::WouldBlock);
            } else {
                return Err(PipeError::BufferFull);
            }
        }

        let to_write = buf.len().min(space);
        for i in 0..to_write {
            self.buffer[self.head] = buf[i];
            self.head = (self.head + 1) % PIPE_BUFFER_CAPACITY;
        }
        self.count += to_write;
        Ok(to_write)
    }

    pub fn add_reader(&mut self) {
        self.readers_count += 1;
        self.closed_read = false;
    }

    pub fn add_writer(&mut self) {
        self.writers_count += 1;
        self.closed_write = false;
    }

    pub fn remove_reader(&mut self) {
        if self.readers_count > 0 {
            self.readers_count -= 1;
        }
        if self.readers_count == 0 {
            self.closed_read = true;
        }
    }

    pub fn remove_writer(&mut self) {
        if self.writers_count > 0 {
            self.writers_count -= 1;
        }
        if self.writers_count == 0 {
            self.closed_write = true;
        }
    }
}

pub fn create_pipe_pair() -> (Arc<Mutex<PipeBuffer>>, Arc<Mutex<PipeBuffer>>) {
    let pipe = Arc::new(Mutex::new(PipeBuffer::new()));
    (pipe.clone(), pipe)
}
