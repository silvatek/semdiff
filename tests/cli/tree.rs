use std::path::Path;

use semdiff::{apply_document, compare_sources, FileSystem, MemoryFileSystem, NodeKind};

/// One in-memory tree for a comparison and the directory it is applied to.
pub struct Tree {
    fs: MemoryFileSystem,
}

impl Tree {
    pub fn new() -> Self {
        Self {
            fs: MemoryFileSystem::new(),
        }
    }

    pub fn with_ignores(ignores: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            fs: MemoryFileSystem::with_ignores(ignores),
        }
    }

    pub fn write(&self, path: &str, contents: impl AsRef<[u8]>) {
        let path = Path::new(path);
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                self.fs.create_dir(parent).unwrap();
            }
        }
        self.fs.write(path, contents.as_ref()).unwrap();
    }

    pub fn dir(&self, path: &str) {
        self.fs.create_dir(Path::new(path)).unwrap();
    }

    pub fn read(&self, path: &str) -> Vec<u8> {
        self.fs.read(Path::new(path)).unwrap()
    }

    pub fn exists(&self, path: &str) -> bool {
        self.fs.kind(Path::new(path)).unwrap().is_some()
    }

    pub fn is_dir(&self, path: &str) -> bool {
        matches!(
            self.fs.kind(Path::new(path)).unwrap(),
            Some(NodeKind::Directory)
        )
    }

    pub fn compare(&self, left: &str, right: &str) -> String {
        compare_sources(&self.fs, left, right).expect("comparison")
    }

    pub fn apply(&self, source: &str, document: &str) -> Result<(), String> {
        apply_document(&self.fs, document, source)
    }
}
