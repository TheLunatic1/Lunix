//! VFS File Handles and Directory Entries

use super::inode::INodeType;
use alloc::string::String;
use alloc::vec::Vec;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeekFrom {
    Start(u64),
    Current(i64),
    End(i64),
}

#[derive(Debug, Clone)]
pub struct DirectoryEntry {
    pub name: String,
    pub node_type: INodeType,
    pub size: u64,
}

pub trait FileHandle: Send + Sync {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, &'static str>;
    fn write(&mut self, buf: &[u8]) -> Result<usize, &'static str>;
    fn seek(&mut self, pos: SeekFrom) -> Result<u64, &'static str>;
    fn size(&self) -> u64;
}

pub struct MemoryFile {
    data: Vec<u8>,
    position: usize,
}

impl MemoryFile {
    pub fn new(data: Vec<u8>) -> Self {
        Self { data, position: 0 }
    }
}

impl FileHandle for MemoryFile {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, &'static str> {
        if self.position >= self.data.len() {
            return Ok(0); // EOF
        }

        let remaining = self.data.len() - self.position;
        let to_read = buf.len().min(remaining);

        buf[..to_read].copy_from_slice(&self.data[self.position..self.position + to_read]);
        self.position += to_read;
        Ok(to_read)
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize, &'static str> {
        let needed = self.position + buf.len();
        if needed > self.data.len() {
            self.data.resize(needed, 0);
        }
        self.data[self.position..self.position + buf.len()].copy_from_slice(buf);
        self.position += buf.len();
        Ok(buf.len())
    }

    fn seek(&mut self, pos: SeekFrom) -> Result<u64, &'static str> {
        let new_pos = match pos {
            SeekFrom::Start(offset) => offset as i64,
            SeekFrom::Current(offset) => self.position as i64 + offset,
            SeekFrom::End(offset) => self.data.len() as i64 + offset,
        };

        if new_pos < 0 {
            return Err("Invalid seek position below 0");
        }

        self.position = new_pos as usize;
        Ok(self.position as u64)
    }

    fn size(&self) -> u64 {
        self.data.len() as u64
    }
}
