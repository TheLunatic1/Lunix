//! sysfs (`/sys`): the kernel's device model exported as a tree of directories, small
//! text attribute files and symlinks.
//!
//! Userspace relies on it for device discovery; e.g. the X server's `fbdev` driver checks
//! that `/sys/class/graphics/fb0` is a symlink before it accepts `/dev/fb0`. Only the
//! parts of the tree the kernel actually has devices for are populated.

use super::file::{DirectoryEntry, FileHandle, MemoryFile};
use super::inode::{INode, INodeType};
use super::path::Path;
use super::vfs::{FileSystem, VfsError};
use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

#[derive(Clone)]
enum Node {
    Dir,
    /// Attribute file whose text is produced on every open.
    Attr(fn() -> String),
    Link(&'static str),
}

pub struct SysFs {
    nodes: BTreeMap<String, Node>,
}

fn fb_info() -> (usize, usize, usize, usize) {
    match crate::display::console::get_framebuffer_info() {
        Some(f) => (f.width, f.height, f.stride, f.bytes_per_pixel),
        None => (0, 0, 0, 4),
    }
}

impl SysFs {
    pub fn new() -> Self {
        let mut s = SysFs { nodes: BTreeMap::new() };

        // Framebuffer device registered by the UEFI GOP driver (/dev/fb0).
        let fb = "/devices/platform/efi-framebuffer.0/graphics/fb0";
        s.dir(fb);
        s.attr(&alloc::format!("{}/name", fb), || String::from("EFI VGA\n"));
        s.attr(&alloc::format!("{}/dev", fb), || String::from("29:0\n"));
        s.attr(&alloc::format!("{}/bits_per_pixel", fb), || alloc::format!("{}\n", fb_info().3 * 8));
        s.attr(&alloc::format!("{}/virtual_size", fb), || {
            let (w, h, _, _) = fb_info();
            alloc::format!("{},{}\n", w, h)
        });
        s.attr(&alloc::format!("{}/stride", fb), || {
            let (_, _, stride, bpp) = fb_info();
            alloc::format!("{}\n", stride * bpp)
        });
        s.attr(&alloc::format!("{}/uevent", fb), || String::from("MAJOR=29\nMINOR=0\nDEVNAME=fb0\n"));
        // The framebuffer is a platform device (efifb), not a PCI one: X's fbdev driver
        // follows fb0/device/subsystem to find out which bus it is on.
        s.link(&alloc::format!("{}/device", fb), "../../../efi-framebuffer.0");
        s.link("/devices/platform/efi-framebuffer.0/subsystem", "../../../bus/platform");
        s.dir("/bus/platform");
        s.dir("/class");
        s.dir("/class/graphics");
        s.link("/class/graphics/fb0", "../../devices/platform/efi-framebuffer.0/graphics/fb0");
        s.dir("/dev");
        s.dir("/dev/char");
        s.link("/dev/char/29:0", "../../devices/platform/efi-framebuffer.0/graphics/fb0");

        // Generic topology.
        s.dir("/block");
        s.dir("/bus");
        s.dir("/fs");
        s.dir("/kernel");
        s.attr("/kernel/osrelease", || String::from("6.8.0-arch1-1-lunix\n"));
        s.attr("/kernel/ostype", || String::from("Linux\n"));
        s.attr("/devices/system/cpu/online", || {
            let n = crate::arch::x86_64::smp::CPU_COUNT.load(core::sync::atomic::Ordering::SeqCst);
            alloc::format!("0-{}\n", n.saturating_sub(1))
        });
        s.attr("/devices/system/cpu/possible", || String::from("0-63\n"));
        s.attr("/devices/system/cpu/present", || {
            let n = crate::arch::x86_64::smp::CPU_COUNT.load(core::sync::atomic::Ordering::SeqCst);
            alloc::format!("0-{}\n", n.saturating_sub(1))
        });
        s
    }

    /// Add a directory and all its parents.
    fn dir(&mut self, path: &str) {
        let mut cur = String::new();
        for comp in path.split('/').filter(|c| !c.is_empty()) {
            cur.push('/');
            cur.push_str(comp);
            self.nodes.entry(cur.clone()).or_insert(Node::Dir);
        }
    }

    fn attr(&mut self, path: &str, f: fn() -> String) {
        if let Some(idx) = path.rfind('/') {
            self.dir(&path[..idx]);
        }
        self.nodes.insert(String::from(path), Node::Attr(f));
    }

    fn link(&mut self, path: &str, target: &'static str) {
        if let Some(idx) = path.rfind('/') {
            self.dir(&path[..idx]);
        }
        self.nodes.insert(String::from(path), Node::Link(target));
    }

    fn get(&self, path: &str) -> Option<&Node> {
        let norm = Path::new(path);
        if norm.is_root() {
            return Some(&Node::Dir);
        }
        self.nodes.get(norm.as_str())
    }

    /// Resolve symlinks (relative and absolute-within-sysfs). `follow_last` selects
    /// stat vs lstat behaviour for the final component.
    fn resolve(&self, path: &str, follow_last: bool) -> Result<(String, Node), VfsError> {
        let mut cur = String::from(Path::new(path).as_str());
        for _ in 0..40 {
            // Resolve intermediate symlinks by walking the prefixes.
            let comps: Vec<String> = Path::new(&cur).components().to_vec();
            let mut prefix = String::new();
            let mut restart = false;
            for (i, comp) in comps.iter().enumerate() {
                let parent = prefix.clone();
                prefix.push('/');
                prefix.push_str(comp);
                let last = i + 1 == comps.len();
                if let Some(Node::Link(target)) = self.nodes.get(&prefix) {
                    if last && !follow_last {
                        break;
                    }
                    let joined = if target.starts_with('/') {
                        String::from(*target)
                    } else {
                        alloc::format!("{}/{}", parent, target)
                    };
                    let mut rest = String::from(joined);
                    for tail in &comps[i + 1..] {
                        rest.push('/');
                        rest.push_str(tail);
                    }
                    cur = String::from(Path::new(&rest).as_str());
                    restart = true;
                    break;
                }
            }
            if restart {
                continue;
            }
            return match self.get(&cur) {
                Some(n) => Ok((cur, n.clone())),
                None => Err(VfsError::NotFound),
            };
        }
        Err(VfsError::NotFound) // ELOOP
    }

    fn inode_of(&self, path: &str, node: &Node) -> INode {
        let (node_type, size, perm) = match node {
            Node::Dir => (INodeType::Directory, 0, 0o555),
            Node::Attr(f) => (INodeType::File, f().len() as u64, 0o444),
            Node::Link(t) => (INodeType::SymLink, t.len() as u64, 0o777),
        };
        let mut id = 0xcbf29ce484222325u64;
        for b in path.bytes() {
            id ^= b as u64;
            id = id.wrapping_mul(0x100000001b3);
        }
        INode {
            id,
            size,
            node_type,
            permissions: perm,
            name: String::from(path.rsplit('/').next().unwrap_or("")),
        }
    }
}

impl FileSystem for SysFs {
    fn open(&self, path: &str) -> Result<Box<dyn FileHandle>, VfsError> {
        match self.resolve(path, true)?.1 {
            Node::Attr(f) => Ok(Box::new(MemoryFile::new(f().into_bytes()))),
            Node::Dir => Err(VfsError::NotAFile),
            Node::Link(_) => Err(VfsError::NotFound),
        }
    }

    fn read_dir(&self, path: &str) -> Result<Vec<DirectoryEntry>, VfsError> {
        let (dir, node) = self.resolve(path, true)?;
        if !matches!(node, Node::Dir) {
            return Err(VfsError::NotADirectory);
        }
        let prefix = if dir == "/" { String::new() } else { dir };
        let mut out = Vec::new();
        for (p, n) in self.nodes.range(alloc::format!("{}/", prefix)..) {
            let Some(rest) = p.strip_prefix(&alloc::format!("{}/", prefix)) else { break };
            if rest.contains('/') {
                continue; // deeper descendant
            }
            let node_type = match n {
                Node::Dir => INodeType::Directory,
                Node::Attr(_) => INodeType::File,
                Node::Link(_) => INodeType::SymLink,
            };
            out.push(DirectoryEntry { name: String::from(rest), node_type, size: 0 });
        }
        Ok(out)
    }

    fn stat(&self, path: &str) -> Result<INode, VfsError> {
        let (p, n) = self.resolve(path, true)?;
        Ok(self.inode_of(&p, &n))
    }

    fn lstat(&self, path: &str) -> Result<INode, VfsError> {
        let (p, n) = self.resolve(path, false)?;
        Ok(self.inode_of(&p, &n))
    }

    fn readlink(&self, path: &str) -> Result<String, VfsError> {
        match self.resolve(path, false)?.1 {
            Node::Link(t) => Ok(String::from(t)),
            _ => Err(VfsError::UnsupportedOperation),
        }
    }
}
