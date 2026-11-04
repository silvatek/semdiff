use std::fs;
use std::path::Path;

/// Content changes for files present on both sides.
///
/// A pair with the same bytes needs no record. A pair that differs records the new
/// contents so the old copy can be brought up to date.
pub(crate) fn content_changes(
    sources: &crate::Sources,
    paired_files: &[String],
) -> Result<Vec<crate::semdif::Change>, String> {
    let left_root = crate::source_base(Path::new(&sources.left));
    let right_root = crate::source_base(Path::new(&sources.right));
    let mut changes = Vec::new();
    for path in paired_files {
        let before = read_file(left_root, path)?;
        let after = read_file(right_root, path)?;
        if before == after {
            continue;
        }
        changes.push(crate::semdif::Change::FileChanged {
            path: path.clone(),
            contents: after,
        });
    }
    Ok(changes)
}

fn read_file(root: &Path, relative: &str) -> Result<Vec<u8>, String> {
    let path = root.join(relative);
    fs::read(&path).map_err(|error| format!("cannot read {}: {error}", path.display()))
}
