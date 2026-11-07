//! The record of a directory-level change, including how apply carries it out.

use std::path::Path;

use super::{Change, RemovedDirectory};

use crate::filesystem::FileSystem;
use crate::semdif::write_detail;

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
        write_detail(document, "path", &self.path);
    }

    fn apply(&self, fs: &dyn FileSystem, source: &Path) -> Result<(), String> {
        fs.add_directory(source, &self.path)
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
        write_detail(document, "path", &self.path);
    }

    fn apply(&self, fs: &dyn FileSystem, source: &Path) -> Result<(), String> {
        fs.remove_directory(source, &self.path)
    }
}

impl RemovedDirectory for DirectoryDeleted {
    fn directory(&self) -> &str {
        &self.path
    }
}
