//! VFS Path Utilities and Normalization

use alloc::string::String;
use alloc::vec::Vec;

#[derive(Debug, Clone)]
pub struct Path {
    raw: String,
    components: Vec<String>,
}

impl Path {
    pub fn new(path_str: &str) -> Self {
        let trimmed = path_str.trim();
        let mut components = Vec::new();

        for part in trimmed.split('/') {
            let p = part.trim();
            if p.is_empty() || p == "." {
                continue;
            }
            if p == ".." {
                components.pop();
            } else {
                components.push(String::from(p));
            }
        }

        let mut normalized = String::from("/");
        for (i, c) in components.iter().enumerate() {
            normalized.push_str(c);
            if i + 1 < components.len() {
                normalized.push('/');
            }
        }

        Self {
            raw: normalized,
            components,
        }
    }

    pub fn as_str(&self) -> &str {
        &self.raw
    }

    pub fn components(&self) -> &[String] {
        &self.components
    }

    pub fn is_root(&self) -> bool {
        self.components.is_empty()
    }

    pub fn file_name(&self) -> Option<&str> {
        self.components.last().map(|s| s.as_str())
    }
}
