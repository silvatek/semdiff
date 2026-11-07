//! Identifying and classifying changes at the file level.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::change::Change;
use crate::filesystem::FileSystem;

/// Renames, deletions, and additions among the files that exist on only one side,
/// and content changes for a path that exists on both sides.
///
/// A hash identifies the content. One file on the left and one file on the right with
/// the same hash are a rename, so identical copies are not paired by guesswork. A file
/// that remains only on the left was removed. A file that remains only on the right
/// was added, and its contents are kept so the file can be recreated. Each record
/// includes that SHA-256, so apply can check the bytes it moves, removes, or writes.
pub(crate) fn file_changes(
    fs: &dyn FileSystem,
    sources: &crate::Sources,
    left_only: &[String],
    right_only: &[String],
) -> Result<Vec<Box<dyn Change>>, String> {
    let left_root = fs.source_base(Path::new(&sources.left));
    let right_root = fs.source_base(Path::new(&sources.right));
    let mut left_by_path = HashMap::<&str, [u8; 32]>::new();
    let mut left_hashes = HashMap::<[u8; 32], Vec<&String>>::new();
    for path in left_only {
        let hash = file_hash(fs, left_root, path)?;
        left_by_path.insert(path.as_str(), hash);
        left_hashes.entry(hash).or_default().push(path);
    }
    let mut right_by_path = HashMap::<&str, [u8; 32]>::new();
    let mut right_hashes = HashMap::<[u8; 32], Vec<&String>>::new();
    for path in right_only {
        let hash = file_hash(fs, right_root, path)?;
        right_by_path.insert(path.as_str(), hash);
        right_hashes.entry(hash).or_default().push(path);
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
                hash: digest_hex(&hash),
            }));
        }
    }
    for path in left_only {
        if !renamed_from.contains(path.as_str()) {
            changes.push(Box::new(crate::change::FileDeleted {
                scope: crate::change::TREE_SCOPE.to_string(),
                path: path.clone(),
                hash: digest_hex(&left_by_path[path.as_str()]),
            }));
        }
    }
    for path in right_only {
        if !renamed_to.contains(path.as_str()) {
            changes.push(Box::new(crate::change::FileAdded {
                scope: crate::change::TREE_SCOPE.to_string(),
                path: path.clone(),
                hash: digest_hex(&right_by_path[path.as_str()]),
                contents: read_file(fs, right_root, path)?,
            }));
        }
    }
    changes.sort_by(|left, right| left.sort_key().cmp(&right.sort_key()));
    Ok(changes)
}

/// Changes for files present at the same path on both sides.
///
/// Matching copies are left out. A file whose bytes differ records both
/// SHA-256s. When both copies are text and a line differs, the line records
/// follow. Otherwise the new bytes are one whole-file change. That includes a
/// file that is not text, and a text file whose only difference is the final
/// newline.
pub(crate) fn paired_changes(
    fs: &dyn FileSystem,
    sources: &crate::Sources,
    paired_files: &[String],
) -> Result<Vec<Box<dyn Change>>, String> {
    let left_root = fs.source_base(Path::new(&sources.left));
    let right_root = fs.source_base(Path::new(&sources.right));
    let mut changes = Vec::new();
    for path in paired_files {
        changes.extend(paired_change(fs, left_root, right_root, path)?);
    }
    Ok(changes)
}

/// The records for one paired path, when its two copies differ.
fn paired_change(
    fs: &dyn FileSystem,
    left_root: &Path,
    right_root: &Path,
    path: &str,
) -> Result<Vec<Box<dyn Change>>, String> {
    let before = read_file(fs, left_root, path)?;
    let after = read_file(fs, right_root, path)?;
    if before == after {
        return Ok(Vec::new());
    }
    let mut changes = vec![Box::new(crate::change::FileChanged::hashes(
        path.to_string(),
        sha256_hex(&before),
        sha256_hex(&after),
    )) as Box<dyn Change>];
    match super::line_changes::text_line_changes(path, &before, &after) {
        Some(line_edits) if !line_edits.is_empty() => changes.extend(line_edits),
        _ => changes.push(Box::new(crate::change::FileChanged::contents(
            path.to_string(),
            after,
        ))),
    }
    Ok(changes)
}

fn read_file(fs: &dyn FileSystem, root: &Path, relative: &str) -> Result<Vec<u8>, String> {
    let path = root.join(relative);
    fs.read(&path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))
}

/// A content identity for one file.
///
/// Rename detection can compare files by this identity instead of holding their bytes.
fn file_hash(fs: &dyn FileSystem, root: &Path, relative: &str) -> Result<[u8; 32], String> {
    let path = root.join(relative);
    fs.hash(&path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))
}

/// The SHA-256 identity of a file's bytes, as lowercase hex.
pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    digest_hex(&Sha256::digest(bytes))
}

/// Lowercase hex for a SHA-256 digest.
fn digest_hex(digest: &[u8]) -> String {
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        hex.push(char::from(b"0123456789abcdef"[(byte >> 4) as usize]));
        hex.push(char::from(b"0123456789abcdef"[(byte & 0xf) as usize]));
    }
    hex
}

/// The proportion of lines two files have in common.
///
/// A later move check uses this to tell a renamed file with edited contents
/// from an unrelated deletion and addition. `None` means a file is not UTF-8
/// text, so it has no lines to compare.
///
/// Lines are split the same way as a line diff. A trailing newline is not a
/// line, and a carriage return stays on its line. Each occurrence of a line
/// is common when the other file has that same line left to match, wherever
/// it sits. The proportion is that count divided by the number of lines in
/// the longer file. Two empty files share every line, so the proportion is 1.
/// A file with lines and an empty file share none, so the proportion is 0.
#[allow(dead_code)]
pub(crate) fn common_line_proportion(left: &[u8], right: &[u8]) -> Option<f64> {
    let (Ok(left_text), Ok(right_text)) = (std::str::from_utf8(left), std::str::from_utf8(right))
    else {
        return None;
    };
    let left_lines = super::line_changes::split_text(left_text).lines;
    let right_lines = super::line_changes::split_text(right_text).lines;
    let longer = left_lines.len().max(right_lines.len());
    if longer == 0 {
        return Some(1.0);
    }
    Some(shared_lines(&left_lines, &right_lines) as f64 / longer as f64)
}

/// Occurrences of a line that both files still have left to match.
///
/// A line that occurs three times in one file and once in the other contributes
/// one. The match is the line's text.
fn shared_lines(left: &[String], right: &[String]) -> usize {
    let mut remaining = HashMap::<&str, usize>::new();
    for line in left {
        *remaining.entry(line.as_str()).or_default() += 1;
    }
    let mut shared = 0;
    for line in right {
        let Some(count) = remaining.get_mut(line.as_str()) else {
            continue;
        };
        if *count == 0 {
            continue;
        }
        *count -= 1;
        shared += 1;
    }
    shared
}

#[cfg(test)]
mod tests {
    use super::common_line_proportion;

    #[test]
    fn identical_text_shares_every_line() {
        assert_eq!(common_line_proportion(b"a\nb\n", b"a\nb\n"), Some(1.0));
        assert_eq!(common_line_proportion(b"\n", b"\n"), Some(1.0));
        assert_eq!(common_line_proportion(b"", b""), Some(1.0));
        assert_eq!(
            common_line_proportion("héllo\n".as_bytes(), "héllo\n".as_bytes()),
            Some(1.0)
        );
    }

    #[test]
    fn a_trailing_newline_is_not_an_extra_line() {
        assert_eq!(common_line_proportion(b"a\nb\n", b"a\nb"), Some(1.0));
        assert_eq!(common_line_proportion(b"a\nb", b"a\nb\n"), Some(1.0));
    }

    #[test]
    fn the_same_lines_match_in_either_order() {
        assert_eq!(common_line_proportion(b"b\na\n", b"a\nb\n"), Some(1.0));
        assert_eq!(
            common_line_proportion(b"c\nb\na\n", b"a\nb\nc\n"),
            Some(1.0)
        );
    }

    #[test]
    fn the_proportion_is_shared_lines_over_the_longer_file() {
        assert_eq!(
            common_line_proportion(b"a\nb\n", b"a\nb\nc\nd\n"),
            Some(0.5)
        );
        assert_eq!(
            common_line_proportion(b"a\nb\nc\nd\n", b"a\nb\n"),
            Some(0.5)
        );
        assert_eq!(
            common_line_proportion(b"a\nx\nc\ny\n", b"a\nb\nc\nd\n"),
            Some(0.5)
        );
        assert_eq!(common_line_proportion(b"a\n", b"b\n"), Some(0.0));
        assert_eq!(common_line_proportion(b"", b"a\n"), Some(0.0));
        assert_eq!(common_line_proportion(b"a\n", b""), Some(0.0));
        assert_eq!(common_line_proportion(b"\n", b""), Some(0.0));
    }

    #[test]
    fn each_occurrence_and_each_blank_line_counts() {
        assert_eq!(common_line_proportion(b"a\na\na\na\n", b"a\n"), Some(0.25));
        assert_eq!(common_line_proportion(b"a\na\n", b"a\nb\n"), Some(0.5));
        assert_eq!(common_line_proportion(b"a\n\n\n\n", b"a\n"), Some(0.25));
        assert_eq!(
            common_line_proportion(b"a\n\n\n\n", b"\n\n\n\n"),
            Some(0.75)
        );
        assert_eq!(common_line_proportion(b"a\r\nb\n", b"a\nb\n"), Some(0.5));
    }

    #[test]
    fn a_non_text_file_has_no_line_proportion() {
        assert_eq!(common_line_proportion(b"a\n", b"\xff"), None);
        assert_eq!(common_line_proportion(b"\xff", b"a\n"), None);
        assert_eq!(common_line_proportion(b"\xff", b"\xfe"), None);
    }
}
