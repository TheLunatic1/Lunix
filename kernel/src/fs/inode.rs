//! VFS Inode and Metadata Definitions

use alloc::string::String;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum INodeType {
    File,
    Directory,
    BlockDevice,
    CharDevice,
    SymLink,
}

#[derive(Debug, Clone)]
pub struct INode {
    pub id: u64,
    pub size: u64,
    pub node_type: INodeType,
    pub permissions: u16,
    pub name: String,
}

impl INode {
    pub fn is_file(&self) -> bool {
        self.node_type == INodeType::File
    }

    pub fn is_dir(&self) -> bool {
        self.node_type == INodeType::Directory
    }
}
