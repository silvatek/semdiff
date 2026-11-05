//! An empty directory added to or removed from the tree.

use std::path::Path;

use super::{Change, RemovedDirectory};

use crate::filesystem::FileSystem;

pub(crate) struct DirectoryAdded {
    pub(crate) scope: String,
    pub(crate) path: String,
}

impl Change for DirectoryAdded {
    fn kind(&self) -> &'static str {
        "directory-added"
    }

    fn scope(&self) -> &str {
        &self.scope
    }

    fn sort_key(&self) -> (&str, &str) {
        (&self.path, "")
    }

    fn write_details(&self, document: &mut String) {
        crate::semdif::write_detail(document, "path", &self.path);
    }

    fn apply(&self, fs: &dyn FileSystem, source: &Path) -> Result<(), String> {
        crate::apply::add_directory(fs, source, &self.path)
    }
}

pub(crate) struct DirectoryDeleted {
    pub(crate) scope: String,
    pub(crate) path: String,
}

impl Change for DirectoryDeleted {
    fn kind(&self) -> &'static str {
        "directory-deleted"
    }

    fn scope(&self) -> &str {
        &self.scope
    }

    fn sort_key(&self) -> (&str, &str) {
        (&self.path, "")
    }

    fn removed_directory(&self) -> Option<&dyn RemovedDirectory> {
        Some(self)
    }

    fn write_details(&self, document: &mut String) {
        crate::semdif::write_detail(document, "path", &self.path);
    }

    fn apply(&self, fs: &dyn FileSystem, source: &Path) -> Result<(), String> {
        crate::apply::remove_directory(fs, source, &self.path)
    }
}

impl RemovedDirectory for DirectoryDeleted {
    fn directory(&self) -> &str {
        &self.path
    }
}
