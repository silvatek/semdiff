use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::io::Read;
use std::path::Path;

use sha2::{Digest, Sha256};

const HASH_BUFFER_SIZE: usize = 8 * 1024;

/// Finds files that were renamed between the two sources.
///
/// A rename is a relative path that exists on only one side and whose SHA-256 digest
/// matches exactly one unpaired file on the other side.
pub(crate) fn file_renames(sources: &crate::Sources) -> Result<Vec<(String, String)>, String> {
    let left_root = Path::new(&sources.left);
    let right_root = Path::new(&sources.right);
    let left_files = crate::collect_files(left_root)?;
    let right_files = crate::collect_files(right_root)?;
    let left_names: BTreeSet<_> = left_files.iter().cloned().collect();
    let right_names: BTreeSet<_> = right_files.iter().cloned().collect();

    let mut left_only = HashMap::<[u8; 32], Vec<String>>::new();
    for path in left_files
        .into_iter()
        .filter(|path| !right_names.contains(path))
    {
        left_only
            .entry(file_hash(left_root, &path)?)
            .or_default()
            .push(path);
    }
    let mut right_only = HashMap::<[u8; 32], Vec<String>>::new();
    for path in right_files
        .into_iter()
        .filter(|path| !left_names.contains(path))
    {
        right_only
            .entry(file_hash(right_root, &path)?)
            .or_default()
            .push(path);
    }

    let mut renames = Vec::new();
    for (hash, from_paths) in left_only {
        let Some(to_paths) = right_only.get(&hash) else {
            continue;
        };
        if from_paths.len() == 1 && to_paths.len() == 1 {
            renames.push((from_paths[0].clone(), to_paths[0].clone()));
        }
    }
    renames.sort();
    Ok(renames)
}

/// Streams `relative` under `root` through an 8 KiB buffer and returns its SHA-256 digest.
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
