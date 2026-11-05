//! The files and directories a comparison or apply reads and writes.
//!
//! The command uses [`StdFileSystem`]. A test that only cares which bytes are
//! at which path uses [`MemoryFileSystem`] and never touches the disk. The
//! round trip against the sample tree stays on [`StdFileSystem`].

use std::collections::BTreeMap;
use std::io::{self, Read};
use std::path::{Component, Path};

/// A file or a directory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeKind {
    File,
    Directory,
}

/// The operations compare and apply perform on a tree.
///
/// `kind` is `Ok(None)` when the path is absent. `read` drains [`FileSystem::open`].
/// `create_dir` creates missing parents. `remove_dir` removes an empty directory
/// and fails when the directory still contains something. `rename` moves a file;
/// the caller creates the destination's parent first.
pub trait FileSystem {
    fn kind(&self, path: &Path) -> io::Result<Option<NodeKind>>;

    fn read_dir(&self, path: &Path) -> io::Result<Vec<(String, NodeKind)>>;

    fn open(&self, path: &Path) -> io::Result<Box<dyn Read>>;

    fn write(&self, path: &Path, bytes: &[u8]) -> io::Result<()>;

    fn create_dir(&self, path: &Path) -> io::Result<()>;

    fn remove_file(&self, path: &Path) -> io::Result<()>;

    fn remove_dir(&self, path: &Path) -> io::Result<()>;

    fn rename(&self, from: &Path, to: &Path) -> io::Result<()>;

    /// The whole contents of a file.
    fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        let mut file = self.open(path)?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        Ok(bytes)
    }
}

/// The operating system's files.
pub struct StdFileSystem;

impl FileSystem for StdFileSystem {
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
}

enum Node {
    File(Vec<u8>),
    Dir(BTreeMap<String, Node>),
}

impl MemoryFileSystem {
    pub fn new() -> Self {
        Self {
            root: std::cell::RefCell::new(Node::Dir(BTreeMap::new())),
        }
    }
}

impl FileSystem for MemoryFileSystem {
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
}
