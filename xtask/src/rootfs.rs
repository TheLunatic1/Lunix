//! In-memory Linux file tree, populated straight from Arch `.pkg.tar` archives.
//!
//! Reading the tarballs directly (instead of extracting them onto a Windows/FAT
//! filesystem) keeps everything a real Linux root needs: case-sensitive names
//! (`/usr/lib/Xorg` next to `/usr/lib/xorg/`), symlinks, hard links, modes and owners.

use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

/// Where a regular file's bytes come from.
#[derive(Clone)]
pub enum Source {
    Mem(Vec<u8>),
    /// `(archive index into Tree::archives, byte offset of the entry's data)`
    Tar(usize, u64),
}

pub enum Kind {
    Dir(BTreeMap<String, usize>),
    File { src: Source, size: u64 },
    Symlink(String),
}

pub struct Node {
    pub kind: Kind,
    /// Permission bits only (no file-type bits).
    pub mode: u16,
    pub uid: u32,
    pub gid: u32,
    pub mtime: u32,
}

pub struct Tree {
    pub nodes: Vec<Node>,
    pub archives: Vec<PathBuf>,
}

impl Tree {
    pub fn new() -> Self {
        Tree {
            nodes: vec![Node { kind: Kind::Dir(BTreeMap::new()), mode: 0o755, uid: 0, gid: 0, mtime: 0 }],
            archives: Vec::new(),
        }
    }

    fn split(path: &str) -> Vec<&str> {
        path.split('/').filter(|c| !c.is_empty() && *c != ".").collect()
    }

    /// Walk to the directory that should contain the last component, creating
    /// missing parents (0755 root:root). Returns (parent_index, leaf_name).
    fn parent_of(&mut self, path: &str) -> Option<(usize, String)> {
        let comps = Self::split(path);
        let (leaf, dirs) = comps.split_last()?;
        let mut cur = 0usize;
        for d in dirs {
            let next = match &self.nodes[cur].kind {
                Kind::Dir(children) => children.get(*d).copied(),
                _ => return None,
            };
            cur = match next {
                Some(i) if matches!(self.nodes[i].kind, Kind::Dir(_)) => i,
                Some(i) => {
                    // A non-directory is in the way (e.g. a symlink replaced by a real dir).
                    self.nodes[i] = Node { kind: Kind::Dir(BTreeMap::new()), mode: 0o755, uid: 0, gid: 0, mtime: 0 };
                    i
                }
                None => {
                    let i = self.new_node(Node { kind: Kind::Dir(BTreeMap::new()), mode: 0o755, uid: 0, gid: 0, mtime: 0 });
                    if let Kind::Dir(children) = &mut self.nodes[cur].kind {
                        children.insert((*d).to_string(), i);
                    }
                    i
                }
            };
        }
        Some((cur, (*leaf).to_string()))
    }

    fn new_node(&mut self, node: Node) -> usize {
        self.nodes.push(node);
        self.nodes.len() - 1
    }

    pub fn lookup(&self, path: &str) -> Option<usize> {
        let mut cur = 0usize;
        for c in Self::split(path) {
            match &self.nodes[cur].kind {
                Kind::Dir(children) => cur = *children.get(c)?,
                _ => return None,
            }
        }
        Some(cur)
    }

    /// Insert (or replace) a node at `path`. Directories merge: an existing directory
    /// keeps its children and only takes the new metadata.
    pub fn insert(&mut self, path: &str, node: Node) {
        let Some((parent, leaf)) = self.parent_of(path) else { return };
        let existing = match &self.nodes[parent].kind {
            Kind::Dir(children) => children.get(&leaf).copied(),
            _ => None,
        };
        match existing {
            Some(i) => {
                let both_dirs = matches!(self.nodes[i].kind, Kind::Dir(_)) && matches!(node.kind, Kind::Dir(_));
                if both_dirs {
                    let n = &mut self.nodes[i];
                    n.mode = node.mode;
                    n.uid = node.uid;
                    n.gid = node.gid;
                    n.mtime = node.mtime;
                } else {
                    self.nodes[i] = node;
                }
            }
            None => {
                let i = self.new_node(node);
                if let Kind::Dir(children) = &mut self.nodes[parent].kind {
                    children.insert(leaf, i);
                }
            }
        }
    }

    pub fn add_file(&mut self, path: &str, data: Vec<u8>, mode: u16) {
        let size = data.len() as u64;
        self.insert(path, Node { kind: Kind::File { src: Source::Mem(data), size }, mode, uid: 0, gid: 0, mtime: 0 });
    }

    /// Ingest one Arch package. Returns its parsed `.PKGINFO`.
    pub fn add_package(&mut self, tar_path: &Path) -> Result<PkgInfo> {
        let archive_idx = self.archives.len();
        self.archives.push(tar_path.to_path_buf());
        let file = File::open(tar_path).with_context(|| format!("open {}", tar_path.display()))?;
        let mut ar = tar::Archive::new(file);
        let mut pkg = PkgInfo::default();
        let mut paths: Vec<String> = Vec::new();

        for entry in ar.entries()? {
            let mut entry = entry?;
            let raw = entry.path()?.to_string_lossy().replace('\\', "/");
            let path = raw.trim_start_matches("./").trim_end_matches('/').to_string();
            if path.is_empty() {
                continue;
            }
            if path == ".PKGINFO" {
                let mut text = String::new();
                entry.read_to_string(&mut text)?;
                pkg = PkgInfo::parse(&text);
                continue;
            }
            if path.starts_with('.') {
                continue; // .MTREE, .BUILDINFO, .INSTALL, .CHANGELOG
            }
            if Self::is_dev_or_doc(&path) {
                continue;
            }

            let hdr = entry.header();
            let mode = (hdr.mode().unwrap_or(0o644) & 0o7777) as u16;
            let uid = hdr.uid().unwrap_or(0) as u32;
            let gid = hdr.gid().unwrap_or(0) as u32;
            let mtime = hdr.mtime().unwrap_or(0) as u32;
            let etype = hdr.entry_type();
            let size = hdr.size().unwrap_or(0);

            if etype.is_dir() {
                self.insert(&path, Node { kind: Kind::Dir(BTreeMap::new()), mode, uid, gid, mtime });
                paths.push(format!("{}/", path));
            } else if etype.is_symlink() {
                let target = entry.link_name()?.map(|p| p.to_string_lossy().replace('\\', "/")).unwrap_or_default();
                self.insert(&path, Node { kind: Kind::Symlink(target), mode: 0o777, uid, gid, mtime });
                paths.push(path);
            } else if etype.is_hard_link() {
                let target = entry.link_name()?.map(|p| p.to_string_lossy().replace('\\', "/")).unwrap_or_default();
                let target = target.trim_start_matches("./").to_string();
                if let Some(i) = self.lookup(&target) {
                    if let Kind::File { src, size } = &self.nodes[i].kind {
                        let (src, size) = (src.clone(), *size);
                        self.insert(&path, Node { kind: Kind::File { src, size }, mode, uid, gid, mtime });
                        paths.push(path);
                    }
                }
            } else if etype.is_file() {
                let offset = entry.raw_file_position();
                self.insert(&path, Node { kind: Kind::File { src: Source::Tar(archive_idx, offset), size }, mode, uid, gid, mtime });
                paths.push(path);
            }
        }
        pkg.files = paths;
        Ok(pkg)
    }

    /// Development files, docs and translations a runtime image doesn't need.
    fn is_dev_or_doc(p: &str) -> bool {
        p.starts_with("usr/include/")
            || p.starts_with("usr/share/man/")
            || p.starts_with("usr/share/doc/")
            || p.starts_with("usr/share/info/")
            || p.starts_with("usr/share/gtk-doc/")
            || p.starts_with("usr/share/licenses/")
            || p.starts_with("usr/share/locale/")
            || p.starts_with("usr/share/i18n/")
            || p.starts_with("usr/share/gir-1.0/")
            || p.starts_with("usr/share/vala/")
            || p.contains("/pkgconfig/")
            || p.contains("/cmake/")
            || p.ends_with(".a")
            || p.ends_with(".h")
            || p.ends_with(".o")
    }

    /// Write the pacman local database (`var/lib/pacman/local/<name>-<ver>/{desc,files}`)
    /// from the packages' own `.PKGINFO`, so `pacman -Q` reports what is really installed.
    pub fn write_pacman_db(&mut self, pkgs: &[PkgInfo], install_time: u64) {
        for p in pkgs {
            if p.name.is_empty() {
                continue;
            }
            let dir = format!("var/lib/pacman/local/{}-{}", p.name, p.version);
            let mut desc = String::new();
            let mut sec = |name: &str, vals: &[String]| {
                if !vals.is_empty() {
                    desc.push_str(&format!("%{}%\n", name));
                    for v in vals {
                        desc.push_str(v);
                        desc.push('\n');
                    }
                    desc.push('\n');
                }
            };
            sec("NAME", &[p.name.clone()]);
            sec("VERSION", &[p.version.clone()]);
            sec("BASE", &[p.base.clone()]);
            sec("DESC", &[p.desc.clone()]);
            sec("URL", &[p.url.clone()]);
            sec("ARCH", &[p.arch.clone()]);
            sec("BUILDDATE", &[p.builddate.clone()]);
            sec("INSTALLDATE", &[install_time.to_string()]);
            sec("PACKAGER", &[p.packager.clone()]);
            sec("SIZE", &[p.size.clone()]);
            sec("LICENSE", &p.license);
            sec("VALIDATION", &["pgp".to_string()]);
            sec("DEPENDS", &p.depend);
            sec("PROVIDES", &p.provides);
            self.add_file(&format!("{}/desc", dir), desc.into_bytes(), 0o644);
            let mut files = String::from("%FILES%\n");
            for f in &p.files {
                files.push_str(f);
                files.push('\n');
            }
            self.add_file(&format!("{}/files", dir), files.into_bytes(), 0o644);
        }
    }
}

#[derive(Default)]
pub struct PkgInfo {
    pub name: String,
    pub base: String,
    pub version: String,
    pub desc: String,
    pub url: String,
    pub arch: String,
    pub builddate: String,
    pub packager: String,
    pub size: String,
    pub license: Vec<String>,
    pub depend: Vec<String>,
    pub provides: Vec<String>,
    pub files: Vec<String>,
}

impl PkgInfo {
    fn parse(text: &str) -> Self {
        let mut p = PkgInfo::default();
        for line in text.lines() {
            let Some((k, v)) = line.split_once(" = ") else { continue };
            let v = v.trim().to_string();
            match k {
                "pkgname" => p.name = v,
                "pkgbase" => p.base = v,
                "pkgver" => p.version = v,
                "pkgdesc" => p.desc = v,
                "url" => p.url = v,
                "arch" => p.arch = v,
                "builddate" => p.builddate = v,
                "packager" => p.packager = v,
                "size" => p.size = v,
                "license" => p.license.push(v),
                "depend" => p.depend.push(v),
                "provides" => p.provides.push(v),
                _ => {}
            }
        }
        p
    }
}
