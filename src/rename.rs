use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::Path;

use sha2::{Digest, Sha256};

const HASH_BUFFER_SIZE: usize = 8 * 1024;

/// File renames among the files that exist on only one side.
///
/// A hash identifies the content. One file on the left and one file on the right with
/// the same hash are a rename, so identical copies are not paired by guesswork.
pub(crate) fn find_renames(
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

    let mut renames = Vec::new();
    for (hash, from_paths) in left_hashes {
        let Some(to_paths) = right_hashes.get(&hash) else {
            continue;
        };
        if from_paths.len() == 1 && to_paths.len() == 1 {
            renames.push(crate::semdif::Change::FileRenamed {
                from: from_paths[0].clone(),
                to: to_paths[0].clone(),
            });
        }
    }
    renames.sort_by(|left, right| rename_paths(left).cmp(&rename_paths(right)));
    Ok(renames)
}

fn rename_paths(change: &crate::semdif::Change) -> (&str, &str) {
    let crate::semdif::Change::FileRenamed { from, to } = change;
    (from, to)
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
