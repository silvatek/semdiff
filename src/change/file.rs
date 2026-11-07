//! The record of a file-level change, including how apply carries it out.

use std::path::Path;

use super::{applied_with_its_file, AddedFile, Change, Content, Digest, Replacement};

use crate::filesystem::FileSystem;

/// A file moved to a new path without a content change.
///
/// `hash` is the SHA-256 of those bytes. Apply renames the file only when the
/// bytes still at `from` have that hash. `--ignore-hash` renames it anyway.
pub(crate) struct FileRenamed {
    pub(crate) scope: String,
    pub(crate) from: String,
    pub(crate) to: String,
    pub(crate) hash: String,
}

impl Change for FileRenamed {
    fn kind(&self) -> &'static str {
        "file-renamed"
    }

    fn scope(&self) -> &str {
        &self.scope
    }

    fn sort_key(&self) -> (&str, &str) {
        (&self.from, &self.to)
    }

    fn write_details(&self, document: &mut String) {
        crate::semdif::write_detail(document, "from", &self.from);
        crate::semdif::write_detail(document, "to", &self.to);
        crate::semdif::write_detail(document, "hash", &self.hash);
    }

    fn check_recorded_hash(&self, fs: &dyn FileSystem, source: &Path) -> Result<(), String> {
        let path = crate::filesystem::path_inside(source, &self.from)?;
        let bytes = fs
            .read(&path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        require_recorded_hash(&path, &bytes, &self.hash)
    }

    fn apply(&self, fs: &dyn FileSystem, source: &Path) -> Result<(), String> {
        fs.rename_file(source, &self.from, &self.to)
    }
}

/// A file moved to a new path with edited contents.
///
/// `from` is the old path and `to` is the new path. `old` is the SHA-256 of
/// the bytes still at `from`. `new` is the SHA-256 of `contents`, the bytes
/// written at `to`. Apply deletes `from` and writes `to` only when both hashes
/// match. `--ignore-hash` does both anyway.
pub(crate) struct FileMoved {
    pub(crate) scope: String,
    pub(crate) from: String,
    pub(crate) to: String,
    pub(crate) old: String,
    pub(crate) new: String,
    pub(crate) contents: Vec<u8>,
}

impl Change for FileMoved {
    fn kind(&self) -> &'static str {
        "file-moved"
    }

    fn scope(&self) -> &str {
        &self.scope
    }

    fn sort_key(&self) -> (&str, &str) {
        (&self.from, &self.to)
    }

    fn added_file(&self) -> Option<&dyn AddedFile> {
        Some(self)
    }

    fn write_details(&self, document: &mut String) {
        crate::semdif::write_detail(document, "from", &self.from);
        crate::semdif::write_detail(document, "to", &self.to);
        crate::semdif::write_detail(document, "old", &self.old);
        crate::semdif::write_detail(document, "new", &self.new);
        crate::semdif::write_file_contents(document, &self.contents);
    }

    fn check_recorded_hash(&self, fs: &dyn FileSystem, source: &Path) -> Result<(), String> {
        let from = crate::filesystem::path_inside(source, &self.from)?;
        let bytes = fs
            .read(&from)
            .map_err(|error| format!("cannot read {}: {error}", from.display()))?;
        require_recorded_hash(&from, &bytes, &self.old)?;
        let to = crate::filesystem::path_inside(source, &self.to)?;
        require_recorded_hash(&to, &self.contents, &self.new)
    }

    fn apply(&self, fs: &dyn FileSystem, source: &Path) -> Result<(), String> {
        fs.delete_file(source, &self.from)?;
        fs.write_file(source, &self.to, &self.contents)
    }
}

impl AddedFile for FileMoved {
    fn path(&self) -> &str {
        &self.to
    }
}

/// A file removed from the tree.
///
/// `hash` is the SHA-256 of the removed bytes. Apply deletes the file only
/// when its bytes still have that hash. `--ignore-hash` deletes it anyway.
pub(crate) struct FileDeleted {
    pub(crate) scope: String,
    pub(crate) path: String,
    pub(crate) hash: String,
}

impl Change for FileDeleted {
    fn kind(&self) -> &'static str {
        "file-deleted"
    }

    fn scope(&self) -> &str {
        &self.scope
    }

    fn sort_key(&self) -> (&str, &str) {
        (&self.path, "")
    }

    fn write_details(&self, document: &mut String) {
        crate::semdif::write_detail(document, "path", &self.path);
        crate::semdif::write_detail(document, "hash", &self.hash);
    }

    fn check_recorded_hash(&self, fs: &dyn FileSystem, source: &Path) -> Result<(), String> {
        let path = crate::filesystem::path_inside(source, &self.path)?;
        let bytes = fs
            .read(&path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        require_recorded_hash(&path, &bytes, &self.hash)
    }

    fn apply(&self, fs: &dyn FileSystem, source: &Path) -> Result<(), String> {
        fs.delete_file(source, &self.path)
    }
}

/// A file added to the tree.
///
/// `hash` is the SHA-256 of `contents`. Apply writes the file only when those
/// bytes have that hash. `--ignore-hash` writes them anyway.
pub(crate) struct FileAdded {
    pub(crate) scope: String,
    pub(crate) path: String,
    pub(crate) hash: String,
    pub(crate) contents: Vec<u8>,
}

impl Change for FileAdded {
    fn kind(&self) -> &'static str {
        "file-added"
    }

    fn scope(&self) -> &str {
        &self.scope
    }

    fn sort_key(&self) -> (&str, &str) {
        (&self.path, "")
    }

    fn added_file(&self) -> Option<&dyn AddedFile> {
        Some(self)
    }

    fn write_details(&self, document: &mut String) {
        crate::semdif::write_detail(document, "path", &self.path);
        crate::semdif::write_detail(document, "hash", &self.hash);
        crate::semdif::write_file_contents(document, &self.contents);
    }

    fn check_recorded_hash(&self, _fs: &dyn FileSystem, source: &Path) -> Result<(), String> {
        let path = crate::filesystem::path_inside(source, &self.path)?;
        require_recorded_hash(&path, &self.contents, &self.hash)
    }

    fn apply(&self, fs: &dyn FileSystem, source: &Path) -> Result<(), String> {
        fs.write_file(source, &self.path, &self.contents)
    }
}

impl AddedFile for FileAdded {
    fn path(&self) -> &str {
        &self.path
    }
}

/// Rejects a tree edit whose bytes are not the recorded SHA-256.
///
/// A deletion and a rename check the file still in the tree. An addition
/// checks the bytes about to be written. A move checks both. A mismatch leaves
/// the tree unchanged.
fn require_recorded_hash(path: &Path, bytes: &[u8], expected: &str) -> Result<(), String> {
    let actual = crate::diff::sha256_hex(bytes);
    if actual == expected {
        Ok(())
    } else {
        Err(format!(
            "cannot apply {}: contents do not match the hash",
            path.display()
        ))
    }
}

/// A `file-changed` change.
///
/// One type, and one semdif type name. The change carries either the old and
/// new SHA-256 hashes of a changed file, or the bytes that replace the whole
/// file.
pub(crate) struct FileChanged {
    scope: String,
    body: FileChangedBody,
}

enum FileChangedBody {
    Hashes(FileHashes),
    Contents(FileContents),
}

struct FileHashes {
    old_hash: String,
    new_hash: String,
}

struct FileContents(Vec<u8>);

impl FileChanged {
    /// The hashes written for every changed file.
    pub(crate) fn hashes(scope: String, old_hash: String, new_hash: String) -> Self {
        Self {
            scope,
            body: FileChangedBody::Hashes(FileHashes { old_hash, new_hash }),
        }
    }

    /// The bytes that replace a file which has no line edits.
    pub(crate) fn contents(scope: String, contents: Vec<u8>) -> Self {
        Self {
            scope,
            body: FileChangedBody::Contents(FileContents(contents)),
        }
    }
}

impl Change for FileChanged {
    fn kind(&self) -> &'static str {
        "file-changed"
    }

    fn scope(&self) -> &str {
        &self.scope
    }

    fn content(&self) -> Option<&dyn Content> {
        Some(self)
    }

    fn digest(&self) -> Option<&dyn Digest> {
        match &self.body {
            FileChangedBody::Hashes(hashes) => Some(hashes),
            FileChangedBody::Contents(_) => None,
        }
    }

    fn replacement(&self) -> Option<&dyn Replacement> {
        match &self.body {
            FileChangedBody::Contents(contents) => Some(contents),
            FileChangedBody::Hashes(_) => None,
        }
    }

    fn write_details(&self, document: &mut String) {
        match &self.body {
            FileChangedBody::Hashes(hashes) => {
                crate::semdif::write_detail(document, "old", &hashes.old_hash);
                crate::semdif::write_detail(document, "new", &hashes.new_hash);
            }
            FileChangedBody::Contents(contents) => {
                crate::semdif::write_file_contents(document, &contents.0);
            }
        }
    }

    fn apply(&self, _fs: &dyn FileSystem, _source: &Path) -> Result<(), String> {
        applied_with_its_file(self.kind())
    }
}

impl Content for FileChanged {
    fn file(&self) -> &str {
        &self.scope
    }
}

impl Digest for FileHashes {
    fn old_hash(&self) -> &str {
        &self.old_hash
    }

    fn new_hash(&self) -> &str {
        &self.new_hash
    }
}

impl Replacement for FileContents {
    fn bytes(&self) -> &[u8] {
        &self.0
    }
}
