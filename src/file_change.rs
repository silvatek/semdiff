use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Read;
use std::path::Path;

use sha2::{Digest, Sha256};

const HASH_BUFFER_SIZE: usize = 8 * 1024;

/// Renames, deletions, and additions among the files that exist on only one side.
///
/// A hash identifies the content. One file on the left and one file on the right with
/// the same hash are a rename, so identical copies are not paired by guesswork. A file
/// that remains only on the left was removed. A file that remains only on the right
/// was added, and its contents are kept so the file can be recreated.
pub(crate) fn file_changes(
    sources: &crate::Sources,
    left_only: &[String],
    right_only: &[String],
) -> Result<Vec<crate::semdif::Change>, String> {
    let left_root = crate::source_base(Path::new(&sources.left));
    let right_root = crate::source_base(Path::new(&sources.right));
    let mut left_hashes = HashMap::<[u8; 32], Vec<&String>>::new();
    for path in left_only {
        left_hashes
            .entry(file_hash(left_root, path)?)
            .or_default()
            .push(path);
    }
    let mut right_hashes = HashMap::<[u8; 32], Vec<&String>>::new();
    for path in right_only {
        right_hashes
            .entry(file_hash(right_root, path)?)
            .or_default()
            .push(path);
    }

    let mut changes = Vec::new();
    let mut renamed_from = HashSet::new();
    let mut renamed_to = HashSet::new();
    for (hash, from_paths) in left_hashes {
        let Some(to_paths) = right_hashes.get(&hash) else {
            continue;
        };
        if from_paths.len() == 1 && to_paths.len() == 1 {
            renamed_from.insert(from_paths[0].as_str());
            renamed_to.insert(to_paths[0].as_str());
            changes.push(crate::semdif::Change::FileRenamed {
                from: from_paths[0].clone(),
                to: to_paths[0].clone(),
            });
        }
    }
    for path in left_only {
        if !renamed_from.contains(path.as_str()) {
            changes.push(crate::semdif::Change::FileDeleted { path: path.clone() });
        }
    }
    for path in right_only {
        if !renamed_to.contains(path.as_str()) {
            changes.push(crate::semdif::Change::FileAdded {
                path: path.clone(),
                contents: read_file(right_root, path)?,
            });
        }
    }
    changes.sort_by(|left, right| change_key(left).cmp(&change_key(right)));
    Ok(changes)
}

fn change_key(change: &crate::semdif::Change) -> (&str, &str) {
    match change {
        crate::semdif::Change::FileRenamed { from, to } => (from, to),
        crate::semdif::Change::FileDeleted { path }
        | crate::semdif::Change::FileAdded { path, .. }
        | crate::semdif::Change::FileChanged { path, .. }
        | crate::semdif::Change::LineAdded { path, .. }
        | crate::semdif::Change::LineRemoved { path, .. }
        | crate::semdif::Change::LineModified { path, .. }
        | crate::semdif::Change::DirectoryAdded { path }
        | crate::semdif::Change::DirectoryDeleted { path } => (path, ""),
    }
}

fn read_file(root: &Path, relative: &str) -> Result<Vec<u8>, String> {
    let path = root.join(relative);
    fs::read(&path).map_err(|error| format!("cannot read {}: {error}", path.display()))
}

/// A content identity for one file.
///
/// Rename detection can compare files by this identity instead of holding their bytes.
fn file_hash(root: &Path, relative: &str) -> Result<[u8; 32], String> {
    let path = root.join(relative);
    let mut file = fs::File::open(&path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; HASH_BUFFER_SIZE];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(hasher.finalize().into())
}
