//! Interacting with the filesystem (real or mocked), including walking directory
//! trees.
//!
//! The command uses [`StdFileSystem`]. A test that only cares which bytes are
//! at which path uses [`MemoryFileSystem`] and never touches the disk. The
//! round trip against the sample tree stays on [`StdFileSystem`].

use std::collections::BTreeMap;
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};

use sha2::{Digest, Sha256};

/// A file or a directory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeKind {
    File,
    Directory,
}

/// The operations compare and apply perform on a tree.
///
/// `kind` is `Ok(None)` when the path is absent. A `kind` error counts as neither
/// a file nor a directory. `read` drains [`FileSystem::open`]. `read_text` is that
/// read decoded as UTF-8. [`FileSystem::ignores`] lists relative paths a source
/// walk leaves out. An entry matches that path and everything under it.
/// `create_dir` creates missing parents. `remove_dir` removes an empty directory
/// and fails when the directory still contains something. `rename` moves a file.
/// The edit methods reject a path that leaves the source. `write_file` and
/// `rename_file` create the destination's parent and leave the source root alone.
pub trait FileSystem {
    fn kind(&self, path: &Path) -> io::Result<Option<NodeKind>>;

    fn read_dir(&self, path: &Path) -> io::Result<Vec<(String, NodeKind)>>;

    fn open(&self, path: &Path) -> io::Result<Box<dyn Read>>;

    fn write(&self, path: &Path, bytes: &[u8]) -> io::Result<()>;

    fn create_dir(&self, path: &Path) -> io::Result<()>;

    fn remove_file(&self, path: &Path) -> io::Result<()>;

    fn remove_dir(&self, path: &Path) -> io::Result<()>;

    fn rename(&self, from: &Path, to: &Path) -> io::Result<()>;

    /// Relative paths a source walk leaves out.
    ///
    /// An entry uses `/`. It matches that path and everything under it.
    /// `kind`, `read`, `write`, and the edit methods still see the path.
    fn ignores(&self) -> &[String];

    /// The whole contents of a file.
    fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        let mut file = self.open(path)?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        Ok(bytes)
    }

    /// The SHA-256 of a file, read in chunks through [`FileSystem::open`].
    fn hash(&self, path: &Path) -> io::Result<[u8; 32]> {
        let mut file = self.open(path)?;
        let mut hasher = Sha256::new();
        let mut buffer = [0_u8; HASH_BUFFER_SIZE];
        loop {
            let count = file.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            hasher.update(&buffer[..count]);
        }
        Ok(hasher.finalize().into())
    }

    /// Whether `kind` reports a file. An error counts as no.
    fn is_file(&self, path: &Path) -> bool {
        matches!(self.kind(path).ok().flatten(), Some(NodeKind::File))
    }

    /// Whether `kind` reports a directory. An error counts as no.
    fn is_dir(&self, path: &Path) -> bool {
        matches!(self.kind(path).ok().flatten(), Some(NodeKind::Directory))
    }

    /// Whether a source-relative path is covered by [`FileSystem::ignores`].
    fn is_ignored(&self, relative: &str) -> bool {
        self.ignores().iter().any(|ignore| {
            relative == ignore
                || relative
                    .strip_prefix(ignore.as_str())
                    .is_some_and(|rest| rest.starts_with('/'))
        })
    }

    /// The directory from which a source's relative paths are read.
    ///
    /// A single file and a directory tree can then be read in the same way.
    fn source_base<'a>(&self, path: &'a Path) -> &'a Path {
        if self.is_file(path) {
            path.parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .unwrap_or(Path::new("."))
        } else {
            path
        }
    }

    /// The files that belong to a source.
    ///
    /// A directory contributes the files under it, and a file contributes itself.
    /// A path on [`FileSystem::ignores`] is omitted, along with everything under it.
    fn collect_files(&self, root: &Path) -> Result<Vec<String>, String> {
        if self.is_file(root) {
            let name = root
                .file_name()
                .ok_or_else(|| format!("cannot name {}", root.display()))?;
            return Ok(vec![name.to_string_lossy().into_owned()]);
        }
        let mut files = Vec::new();
        visit_files(self, root, root, &mut files)?;
        Ok(files)
    }

    /// The directories that belong to a source.
    ///
    /// Empty directories are part of the tree even though they contain no files.
    /// A path on [`FileSystem::ignores`] is omitted, along with everything under it.
    fn collect_directories(&self, root: &Path) -> Result<Vec<String>, String> {
        if self.is_file(root) {
            return Ok(Vec::new());
        }
        let mut directories = Vec::new();
        visit_directories(self, root, root, &mut directories)?;
        Ok(directories)
    }

    /// Writes one file in the source directory.
    ///
    /// An added file is created. A changed file is replaced with the recorded bytes.
    fn write_file(&self, root: &Path, relative: &str, contents: &[u8]) -> Result<(), String> {
        let path = path_inside(root, relative)?;
        create_parent(self, root, &path)?;
        self.write(&path, contents)
            .map_err(|error| format!("cannot write {}: {error}", path.display()))
    }

    /// Removes one file from the source directory.
    ///
    /// The semdif records the path relative to that directory.
    fn delete_file(&self, root: &Path, relative: &str) -> Result<(), String> {
        let path = path_inside(root, relative)?;
        self.remove_file(&path)
            .map_err(|error| format!("cannot delete {}: {error}", path.display()))
    }

    /// Moves one file inside the source directory.
    ///
    /// The semdif records the old and new paths relative to that directory.
    fn rename_file(&self, root: &Path, from: &str, to: &str) -> Result<(), String> {
        let from_path = path_inside(root, from)?;
        let to_path = path_inside(root, to)?;
        create_parent(self, root, &to_path)?;
        self.rename(&from_path, &to_path).map_err(|error| {
            format!(
                "cannot rename {} to {}: {error}",
                from_path.display(),
                to_path.display()
            )
        })
    }

    /// Creates one directory in the source.
    ///
    /// The semdif records an empty directory that exists only on the other side.
    fn add_directory(&self, root: &Path, relative: &str) -> Result<(), String> {
        let path = path_inside(root, relative)?;
        self.create_dir(&path)
            .map_err(|error| format!("cannot create {}: {error}", path.display()))
    }

    /// Removes one empty directory from the source.
    ///
    /// Files inside it have already been moved or deleted. A directory that still
    /// holds something is left in place and reported.
    fn remove_directory(&self, root: &Path, relative: &str) -> Result<(), String> {
        let path = path_inside(root, relative)?;
        self.remove_dir(&path)
            .map_err(|error| format!("cannot remove {}: {error}", path.display()))
    }

    /// The whole contents of a file, decoded as UTF-8.
    fn read_text(&self, path: &Path) -> io::Result<String> {
        let bytes = self.read(path)?;
        String::from_utf8(bytes).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "stream did not contain valid UTF-8",
            )
        })
    }
}

const HASH_BUFFER_SIZE: usize = 8 * 1024;

/// The operating system's files.
///
/// A new filesystem ignores nothing. [`StdFileSystem::with_ignores`] sets the paths a
/// source walk leaves out.
pub struct StdFileSystem {
    ignores: Vec<String>,
}

impl StdFileSystem {
    pub fn new() -> Self {
        Self::with_ignores(std::iter::empty::<String>())
    }

    pub fn with_ignores(ignores: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            ignores: ignore_list(ignores),
        }
    }
}

impl FileSystem for StdFileSystem {
    fn ignores(&self) -> &[String] {
        &self.ignores
    }

    fn kind(&self, path: &Path) -> io::Result<Option<NodeKind>> {
        match std::fs::metadata(path) {
            Ok(metadata) if metadata.is_dir() => Ok(Some(NodeKind::Directory)),
            Ok(metadata) if metadata.is_file() => Ok(Some(NodeKind::File)),
            Ok(_) => Ok(None),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    fn read_dir(&self, path: &Path) -> io::Result<Vec<(String, NodeKind)>> {
        let mut found = Vec::new();
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let Some(kind) = self.kind(&entry.path())? else {
                continue;
            };
            found.push((entry.file_name().to_string_lossy().into_owned(), kind));
        }
        Ok(found)
    }

    fn open(&self, path: &Path) -> io::Result<Box<dyn Read>> {
        Ok(Box::new(std::fs::File::open(path)?))
    }

    fn write(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        std::fs::write(path, bytes)
    }

    fn create_dir(&self, path: &Path) -> io::Result<()> {
        std::fs::create_dir_all(path)
    }

    fn remove_file(&self, path: &Path) -> io::Result<()> {
        std::fs::remove_file(path)
    }

    fn remove_dir(&self, path: &Path) -> io::Result<()> {
        std::fs::remove_dir(path)
    }

    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        std::fs::rename(from, to)
    }
}

/// A tree held in memory.
///
/// Paths are a sequence of names. A leading slash does not start a second tree.
pub struct MemoryFileSystem {
    root: std::cell::RefCell<Node>,
    ignores: Vec<String>,
}

enum Node {
    File(Vec<u8>),
    Dir(BTreeMap<String, Node>),
}

impl MemoryFileSystem {
    /// A tree that ignores nothing.
    pub fn new() -> Self {
        Self::with_ignores(std::iter::empty::<String>())
    }

    pub fn with_ignores(ignores: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            root: std::cell::RefCell::new(Node::Dir(BTreeMap::new())),
            ignores: ignore_list(ignores),
        }
    }
}

impl FileSystem for MemoryFileSystem {
    fn ignores(&self) -> &[String] {
        &self.ignores
    }

    fn kind(&self, path: &Path) -> io::Result<Option<NodeKind>> {
        let parts = components(path);
        let root = self.root.borrow();
        Ok(walk(&root, &parts).map(Node::kind))
    }

    fn read_dir(&self, path: &Path) -> io::Result<Vec<(String, NodeKind)>> {
        let parts = components(path);
        let root = self.root.borrow();
        let node = require(walk(&root, &parts))?;
        match node {
            Node::Dir(children) => Ok(children
                .iter()
                .map(|(name, child)| (name.clone(), child.kind()))
                .collect()),
            Node::File(_) => Err(not_dir()),
        }
    }

    fn open(&self, path: &Path) -> io::Result<Box<dyn Read>> {
        let parts = components(path);
        let root = self.root.borrow();
        let node = require(walk(&root, &parts))?;
        match node {
            Node::File(bytes) => Ok(Box::new(std::io::Cursor::new(bytes.clone()))),
            Node::Dir(_) => Err(not_file()),
        }
    }

    fn write(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        let parts = components(path);
        let (parent, name) = split_name(&parts)?;
        let mut root = self.root.borrow_mut();
        let directory = require_mut(walk_mut(&mut root, parent))?;
        match directory {
            Node::Dir(children) => {
                match children.get(name) {
                    Some(Node::Dir(_)) => return Err(not_file()),
                    _ => {}
                }
                children.insert(name.to_string(), Node::File(bytes.to_vec()));
                Ok(())
            }
            Node::File(_) => Err(not_dir()),
        }
    }

    fn create_dir(&self, path: &Path) -> io::Result<()> {
        let parts = components(path);
        let mut root = self.root.borrow_mut();
        ensure_dir(&mut root, &parts)
    }

    fn remove_file(&self, path: &Path) -> io::Result<()> {
        let parts = components(path);
        let (parent, name) = split_name(&parts)?;
        let mut root = self.root.borrow_mut();
        let directory = require_mut(walk_mut(&mut root, parent))?;
        match directory {
            Node::Dir(children) => match children.get(name) {
                Some(Node::File(_)) => {
                    children.remove(name);
                    Ok(())
                }
                Some(Node::Dir(_)) => Err(not_file()),
                None => Err(missing()),
            },
            Node::File(_) => Err(not_dir()),
        }
    }

    fn remove_dir(&self, path: &Path) -> io::Result<()> {
        let parts = components(path);
        let (parent, name) = split_name(&parts)?;
        let mut root = self.root.borrow_mut();
        let directory = require_mut(walk_mut(&mut root, parent))?;
        match directory {
            Node::Dir(children) => match children.get(name) {
                Some(Node::Dir(nested)) if nested.is_empty() => {
                    children.remove(name);
                    Ok(())
                }
                Some(Node::Dir(_)) => Err(io::Error::new(
                    io::ErrorKind::DirectoryNotEmpty,
                    "directory not empty",
                )),
                Some(Node::File(_)) => Err(not_dir()),
                None => Err(missing()),
            },
            Node::File(_) => Err(not_dir()),
        }
    }

    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        let bytes = {
            let from_parts = components(from);
            let root = self.root.borrow();
            let node = require(walk(&root, &from_parts))?;
            match node {
                Node::File(bytes) => bytes.clone(),
                Node::Dir(_) => return Err(not_file()),
            }
        };
        {
            let to_parts = components(to);
            let (parent, name) = split_name(&to_parts)?;
            let root = self.root.borrow();
            match require(walk(&root, parent))? {
                Node::Dir(children) => {
                    if matches!(children.get(name), Some(Node::Dir(_))) {
                        return Err(not_file());
                    }
                }
                Node::File(_) => return Err(not_dir()),
            }
        }
        self.remove_file(from)?;
        self.write(to, &bytes)
    }
}

impl Node {
    fn kind(&self) -> NodeKind {
        match self {
            Node::File(_) => NodeKind::File,
            Node::Dir(_) => NodeKind::Directory,
        }
    }
}

/// The paths listed in an ignore file.
///
/// Each line is one relative path. A blank line is skipped. The caller passes
/// the result to [`StdFileSystem::with_ignores`] or [`MemoryFileSystem::with_ignores`].
pub(crate) fn ignore_entries(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

fn ignore_list(ignores: impl IntoIterator<Item = impl Into<String>>) -> Vec<String> {
    ignores
        .into_iter()
        .map(|entry| entry.into().replace('\\', "/"))
        .collect()
}

fn visit_files(
    fs: &(impl FileSystem + ?Sized),
    root: &Path,
    dir: &Path,
    files: &mut Vec<String>,
) -> Result<(), String> {
    let entries = fs
        .read_dir(dir)
        .map_err(|error| format!("cannot read {}: {error}", dir.display()))?;
    for (name, kind) in entries {
        let path = dir.join(&name);
        let relative = relative_to(root, &path)?;
        if fs.is_ignored(&relative) {
            continue;
        }
        match kind {
            NodeKind::Directory => visit_files(fs, root, &path, files)?,
            NodeKind::File => files.push(relative),
        }
    }
    Ok(())
}

fn visit_directories(
    fs: &(impl FileSystem + ?Sized),
    root: &Path,
    dir: &Path,
    directories: &mut Vec<String>,
) -> Result<(), String> {
    let entries = fs
        .read_dir(dir)
        .map_err(|error| format!("cannot read {}: {error}", dir.display()))?;
    for (name, kind) in entries {
        if kind != NodeKind::Directory {
            continue;
        }
        let path = dir.join(&name);
        let relative = relative_to(root, &path)?;
        if fs.is_ignored(&relative) {
            continue;
        }
        directories.push(relative);
        visit_directories(fs, root, &path, directories)?;
    }
    Ok(())
}

fn relative_to(root: &Path, path: &Path) -> Result<String, String> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| format!("cannot relativize {}", path.display()))?;
    Ok(relative.to_string_lossy().replace('\\', "/"))
}

fn create_parent(fs: &(impl FileSystem + ?Sized), root: &Path, path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if parent != root && !parent.as_os_str().is_empty() {
            fs.create_dir(parent)
                .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
        }
    }
    Ok(())
}

/// A path that stays inside the source directory.
///
/// Change paths come from the semdif file, so a path that leaves the directory is rejected.
pub(crate) fn path_inside(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let path = Path::new(relative);
    if relative.is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(format!(
            "change path must stay inside the source directory: {relative}"
        ));
    }
    Ok(root.join(path))
}

fn components(path: &Path) -> Vec<String> {
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_string_lossy().into_owned()),
            Component::ParentDir => {
                parts.pop();
            }
            Component::CurDir | Component::RootDir | Component::Prefix(_) => {}
        }
    }
    parts
}

fn split_name(parts: &[String]) -> io::Result<(&[String], &str)> {
    match parts.split_last() {
        Some((name, parent)) => Ok((parent, name)),
        None => Err(io::Error::new(io::ErrorKind::InvalidInput, "path is empty")),
    }
}

fn walk<'a>(node: &'a Node, parts: &[String]) -> Option<&'a Node> {
    let mut current = node;
    for part in parts {
        match current {
            Node::Dir(children) => current = children.get(part)?,
            Node::File(_) => return None,
        }
    }
    Some(current)
}

fn walk_mut<'a>(node: &'a mut Node, parts: &[String]) -> Option<&'a mut Node> {
    let mut current = node;
    for part in parts {
        match current {
            Node::Dir(children) => current = children.get_mut(part)?,
            Node::File(_) => return None,
        }
    }
    Some(current)
}

fn require(node: Option<&Node>) -> io::Result<&Node> {
    node.ok_or_else(missing)
}

fn require_mut(node: Option<&mut Node>) -> io::Result<&mut Node> {
    node.ok_or_else(missing)
}

fn ensure_dir(node: &mut Node, parts: &[String]) -> io::Result<()> {
    if parts.is_empty() {
        return match node {
            Node::Dir(_) => Ok(()),
            Node::File(_) => Err(not_dir()),
        };
    }
    match node {
        Node::File(_) => Err(not_dir()),
        Node::Dir(children) => {
            let child = children
                .entry(parts[0].clone())
                .or_insert_with(|| Node::Dir(BTreeMap::new()));
            ensure_dir(child, &parts[1..])
        }
    }
}

fn missing() -> io::Error {
    io::Error::new(io::ErrorKind::NotFound, "not found")
}

fn not_dir() -> io::Error {
    io::Error::new(io::ErrorKind::NotADirectory, "not a directory")
}

fn not_file() -> io::Error {
    io::Error::new(io::ErrorKind::Other, "not a file")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_keeps_its_bytes_and_an_empty_directory_can_be_removed() {
        let fs = MemoryFileSystem::new();
        fs.create_dir(Path::new("left/nested")).unwrap();
        fs.write(Path::new("left/nested/note.txt"), b"hello")
            .unwrap();
        assert_eq!(
            fs.read(Path::new("left/nested/note.txt")).unwrap(),
            b"hello"
        );
        assert_eq!(
            fs.kind(Path::new("left/nested")).unwrap(),
            Some(NodeKind::Directory)
        );
        assert!(fs.remove_dir(Path::new("left/nested")).is_err());
        fs.remove_file(Path::new("left/nested/note.txt")).unwrap();
        fs.remove_dir(Path::new("left/nested")).unwrap();
        assert_eq!(fs.kind(Path::new("left/nested")).unwrap(), None);
    }

    #[test]
    fn rename_moves_a_file_and_open_reads_past_one_chunk() {
        let fs = MemoryFileSystem::new();
        let mut bytes = vec![1_u8; 9_000];
        bytes[8_500] = 7;
        fs.create_dir(Path::new("src")).unwrap();
        fs.write(Path::new("src/old.bin"), &bytes).unwrap();
        fs.create_dir(Path::new("dst")).unwrap();
        fs.rename(Path::new("src/old.bin"), Path::new("dst/new.bin"))
            .unwrap();
        let mut file = fs.open(Path::new("dst/new.bin")).unwrap();
        let mut buffer = [0_u8; 8 * 1024];
        let first = file.read(&mut buffer).unwrap();
        let mut rest = Vec::new();
        file.read_to_end(&mut rest).unwrap();
        assert_eq!(first, 8 * 1024);
        assert_eq!(rest.len(), 9_000 - 8 * 1024);
        assert_eq!(rest[8_500 - 8 * 1024], 7);
    }

    #[test]
    fn an_ignore_path_covers_that_path_and_its_children() {
        let fs = MemoryFileSystem::with_ignores(["target"]);
        assert!(fs.is_ignored("target"));
        assert!(fs.is_ignored("target/skip.txt"));
        assert!(!fs.is_ignored("src/target"));
        assert!(!fs.is_ignored("src/target/lib.rs"));
        assert!(!fs.is_ignored("targeting"));
        let open = MemoryFileSystem::new();
        assert!(open.ignores().is_empty());
        assert!(!open.is_ignored("target"));
        assert!(!open.is_ignored("target/skip.txt"));
    }

    #[test]
    fn ignore_entries_keep_each_path_and_skip_blank_lines() {
        let entries = ignore_entries("target\n\n  build.txt \n");
        assert_eq!(entries, vec!["target".to_string(), "build.txt".to_string()]);
        let fs = MemoryFileSystem::with_ignores(entries);
        assert!(fs.is_ignored("target/skip.txt"));
        assert!(fs.is_ignored("build.txt"));
        assert!(!fs.is_ignored("src/main.rs"));
    }
}
