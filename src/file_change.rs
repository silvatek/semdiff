use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Read;
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::change::Change;

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
) -> Result<Vec<Box<dyn Change>>, String> {
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

    let mut changes: Vec<Box<dyn Change>> = Vec::new();
    let mut renamed_from = HashSet::new();
    let mut renamed_to = HashSet::new();
    for (hash, from_paths) in left_hashes {
        let Some(to_paths) = right_hashes.get(&hash) else {
            continue;
        };
        if from_paths.len() == 1 && to_paths.len() == 1 {
            renamed_from.insert(from_paths[0].as_str());
            renamed_to.insert(to_paths[0].as_str());
            changes.push(Box::new(crate::change::FileRenamed {
                scope: crate::change::TREE_SCOPE.to_string(),
                from: from_paths[0].clone(),
                to: to_paths[0].clone(),
            }));
        }
    }
    for path in left_only {
        if !renamed_from.contains(path.as_str()) {
            changes.push(Box::new(crate::change::FileDeleted {
                scope: crate::change::TREE_SCOPE.to_string(),
                path: path.clone(),
            }));
        }
    }
    for path in right_only {
        if !renamed_to.contains(path.as_str()) {
            changes.push(Box::new(crate::change::FileAdded {
                scope: crate::change::TREE_SCOPE.to_string(),
                path: path.clone(),
                contents: read_file(right_root, path)?,
            }));
        }
    }
    changes.sort_by(|left, right| left.sort_key().cmp(&right.sort_key()));
    Ok(changes)
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

/// The SHA-256 identity of a file's bytes, as lowercase hex.
pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        hex.push(char::from(b"0123456789abcdef"[(byte >> 4) as usize]));
        hex.push(char::from(b"0123456789abcdef"[(byte & 0xf) as usize]));
    }
    hex
}
