//! Virtual File System (VFS) Core

use super::file::{DirectoryEntry, FileHandle};
use super::inode::INode;
use super::path::Path;
use alloc::boxed::Box;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VfsError {
    NotFound,
    NotADirectory,
    NotAFile,
    PermissionDenied,
    AlreadyExists,
    DeviceError,
    UnsupportedOperation,
}

pub trait FileSystem: Send + Sync {
    fn open(&self, path: &str) -> Result<Box<dyn FileHandle>, VfsError>;
    fn read_dir(&self, path: &str) -> Result<Vec<DirectoryEntry>, VfsError>;
    fn stat(&self, path: &str) -> Result<INode, VfsError>;
    /// Like `stat` but does not follow a final symlink. Filesystems without symlinks
    /// need not override this.
    fn lstat(&self, path: &str) -> Result<INode, VfsError> {
        self.stat(path)
    }
    fn readlink(&self, _path: &str) -> Result<String, VfsError> {
        Err(VfsError::UnsupportedOperation)
    }
}

struct VfsMount {
    prefix: String,
    fs: Arc<dyn FileSystem>,
}

static VFS_MOUNTS: Mutex<Vec<VfsMount>> = Mutex::new(Vec::new());

pub fn mount(mount_point: &str, fs: Arc<dyn FileSystem>) {
    let mut mounts = VFS_MOUNTS.lock();
    lunix_serial_println!("  [VFS] Mounted filesystem at '{}'", mount_point);
    mounts.push(VfsMount {
        prefix: String::from(mount_point),
        fs,
    });
}

fn resolve_mount<'a>(mounts: &'a [VfsMount], path: &str) -> Option<(&'a Arc<dyn FileSystem>, String)> {
    let norm = Path::new(path);
    let full = norm.as_str();

    // Find longest matching prefix
    let mut best_match: Option<(&Arc<dyn FileSystem>, usize)> = None;

    for mount in mounts.iter() {
        if full == mount.prefix || (full.starts_with(&mount.prefix) && (mount.prefix == "/" || full.as_bytes().get(mount.prefix.len()) == Some(&b'/'))) {
            let p_len = mount.prefix.len();
            if let Some((_, best_len)) = best_match {
                if p_len > best_len {
                    best_match = Some((&mount.fs, p_len));
                }
            } else {
                best_match = Some((&mount.fs, p_len));
            }
        }
    }

    if let Some((fs, len)) = best_match {
        let relative = if len == 1 && full.starts_with('/') {
            String::from(full)
        } else if full.len() == len {
            String::from("/")
        } else {
            String::from(&full[len..])
        };
        Some((fs, relative))
    } else {
        None
    }
}

fn with_mount<T>(path: &str, f: impl FnOnce(&Arc<dyn FileSystem>, &str) -> Result<T, VfsError>) -> Result<T, VfsError> {
    let mounts = VFS_MOUNTS.lock();
    match resolve_mount(&mounts, path) {
        Some((fs, rel_path)) => f(fs, &rel_path),
        None => Err(VfsError::NotFound),
    }
}

pub fn open(path: &str) -> Result<Box<dyn FileHandle>, VfsError> {
    with_mount(path, |fs, p| fs.open(p))
}

pub fn read_dir(path: &str) -> Result<Vec<DirectoryEntry>, VfsError> {
    with_mount(path, |fs, p| fs.read_dir(p))
}

/// `stat(2)`: follows symlinks.
pub fn stat(path: &str) -> Result<INode, VfsError> {
    with_mount(path, |fs, p| fs.stat(p))
}

/// `lstat(2)`: does not follow a symlink in the final component.
pub fn lstat(path: &str) -> Result<INode, VfsError> {
    with_mount(path, |fs, p| fs.lstat(p))
}

/// `readlink(2)`: the target of the symlink at `path`.
pub fn readlink(path: &str) -> Result<String, VfsError> {
    with_mount(path, |fs, p| fs.readlink(p))
}

pub fn read_to_vec(path: &str) -> Result<Vec<u8>, VfsError> {
    let mut file = open(path)?;
    let size = file.size() as usize;
    let mut buf = alloc::vec![0u8; size];
    let mut read_bytes = 0;

    while read_bytes < size {
        match file.read(&mut buf[read_bytes..]) {
            Ok(0) => break,
            Ok(n) => read_bytes += n,
            Err(_) => return Err(VfsError::DeviceError),
        }
    }
    buf.truncate(read_bytes);
    Ok(buf)
}
