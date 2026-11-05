use std::fs;
use std::path::Path;

/// Content changes for files present on both sides.
///
/// Each paired path is compared on its own. Only pairs whose contents differ are recorded.
pub(crate) fn content_changes(
    sources: &crate::Sources,
    paired_files: &[String],
) -> Result<Vec<crate::semdif::Change>, String> {
    let left_root = crate::source_base(Path::new(&sources.left));
    let right_root = crate::source_base(Path::new(&sources.right));
    let mut changes = Vec::new();
    for path in paired_files {
        changes.extend(content_change(left_root, right_root, path)?);
    }
    Ok(changes)
}

/// The changes for one paired file, when its two copies differ.
///
/// Matching copies are left out. Text is recorded as one change per added,
/// removed, or modified line. Bytes that are not text stay one whole-file change,
/// because they have no lines to compare. Every changed file also records the
/// SHA-256 of the old bytes and the new bytes, so apply can check both.
fn content_change(
    left_root: &Path,
    right_root: &Path,
    path: &str,
) -> Result<Vec<crate::semdif::Change>, String> {
    let before = read_file(left_root, path)?;
    let after = read_file(right_root, path)?;
    if before == after {
        return Ok(Vec::new());
    }
    let mut changes = vec![Box::new(crate::change::FileDigest {
        path: path.to_string(),
        old_hash: crate::file_change::sha256_hex(&before),
        new_hash: crate::file_change::sha256_hex(&after),
    }) as crate::semdif::Change];
    match (std::str::from_utf8(&before), std::str::from_utf8(&after)) {
        (Ok(before_text), Ok(after_text)) => {
            let line_edits = crate::line_changes::line_changes(path, before_text, after_text);
            // The line lists match, and the bytes still differ. That difference is
            // the final newline, which is not itself a line.
            if line_edits.is_empty() {
                changes.push(Box::new(crate::change::FileChanged {
                    path: path.to_string(),
                    contents: after,
                }));
            } else {
                changes.extend(line_edits);
            }
        }
        _ => changes.push(Box::new(crate::change::FileChanged {
            path: path.to_string(),
            contents: after,
        })),
    }
    Ok(changes)
}

fn read_file(root: &Path, relative: &str) -> Result<Vec<u8>, String> {
    let path = root.join(relative);
    fs::read(&path).map_err(|error| format!("cannot read {}: {error}", path.display()))
}
