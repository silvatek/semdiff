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
        if let Some(change) = content_change(left_root, right_root, path)? {
            changes.push(change);
        }
    }
    Ok(changes)
}

/// The change for one paired file, when its two copies differ.
///
/// Matching copies are left out. A difference records the new contents so the old
/// copy can be brought up to date.
fn content_change(
    left_root: &Path,
    right_root: &Path,
    path: &str,
) -> Result<Option<crate::semdif::Change>, String> {
    let before = read_file(left_root, path)?;
    let after = read_file(right_root, path)?;
    if before == after {
        return Ok(None);
    }
    Ok(Some(crate::semdif::Change::FileChanged {
        path: path.to_string(),
        contents: after,
    }))
}

fn read_file(root: &Path, relative: &str) -> Result<Vec<u8>, String> {
    let path = root.join(relative);
    fs::read(&path).map_err(|error| format!("cannot read {}: {error}", path.display()))
}
