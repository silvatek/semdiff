use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::change::LinePlan;

/// Edits `source` so it gains the changes recorded in `semdif`.
///
/// The semdif file is the record of what changed, so the same edits can be replayed
/// on another copy of that directory. A problem is reported on stderr.
pub(crate) fn apply(semdif: &str, source: &str) -> ExitCode {
    match apply_changes(Path::new(semdif), Path::new(source)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(1)
        }
    }
}

fn apply_changes(semdif: &Path, source: &Path) -> Result<(), String> {
    let document = fs::read_to_string(semdif)
        .map_err(|error| format!("cannot read {}: {error}", semdif.display()))?;
    let changes = crate::semdif::parse_changes(&document)?;
    let mut applied_content = BTreeSet::new();
    for change in &changes {
        if let Some(content) = change.content() {
            let path = content.file();
            if !applied_content.insert(path.to_string()) {
                continue;
            }
            let batch: Vec<_> = changes
                .iter()
                .filter(|candidate| candidate.content().is_some_and(|item| item.file() == path))
                .collect();
            apply_content(source, path, &batch)?;
        } else {
            apply_change(source, change)?;
        }
    }
    Ok(())
}

/// Performs one structural change in the source directory.
///
/// A content edit is applied with the other edits of its file, in `apply_content`.
fn apply_change(source: &Path, change: &crate::semdif::Change) -> Result<(), String> {
    change.apply(source)
}

/// Creates one directory in the source.
///
/// The semdif records an empty directory that exists only on the other side.
pub(crate) fn add_directory(root: &Path, relative: &str) -> Result<(), String> {
    let path = path_inside(root, relative)?;
    fs::create_dir_all(&path).map_err(|error| format!("cannot create {}: {error}", path.display()))
}

/// Removes one empty directory from the source.
///
/// Files inside it have already been moved or deleted. A directory that still
/// holds something is left in place and reported.
pub(crate) fn remove_directory(root: &Path, relative: &str) -> Result<(), String> {
    let path = path_inside(root, relative)?;
    fs::remove_dir(&path).map_err(|error| format!("cannot remove {}: {error}", path.display()))
}

/// Checks one changed file, builds its new bytes, and writes them when the hashes match.
///
/// The old hash is checked against the file as it is. The new hash is checked
/// against the bytes about to be written, so a mismatch leaves the file unchanged.
fn apply_content(
    root: &Path,
    relative: &str,
    batch: &[&crate::semdif::Change],
) -> Result<(), String> {
    let path = path_inside(root, relative)?;
    let bytes =
        fs::read(&path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let digest = file_digest(batch)?;
    if let Some((old_hash, _)) = &digest {
        check_hash(&path, &bytes, old_hash, "old")?;
    }
    let new_bytes = new_contents(&path, &bytes, batch)?;
    if let Some((_, new_hash)) = &digest {
        check_hash(&path, &new_bytes, new_hash, "new")?;
    }
    if new_bytes != bytes {
        fs::write(&path, new_bytes)
            .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
    }
    Ok(())
}

fn file_digest(batch: &[&crate::semdif::Change]) -> Result<Option<(String, String)>, String> {
    let mut found = None;
    for change in batch {
        let hashes = change
            .digest()
            .map(|digest| (digest.old_hash().to_string(), digest.new_hash().to_string()));
        if let Some(hashes) = hashes {
            if found.is_some() {
                return Err(format!(
                    "{} has more than one file-changed hash",
                    change.scope()
                ));
            }
            found = Some(hashes);
        }
    }
    Ok(found)
}

fn check_hash(path: &Path, bytes: &[u8], expected: &str, side: &str) -> Result<(), String> {
    let actual = crate::file_change::sha256_hex(bytes);
    if actual != expected {
        return Err(format!(
            "cannot apply {}: contents do not match the {side} hash",
            path.display()
        ));
    }
    Ok(())
}

fn new_contents(
    path: &Path,
    bytes: &[u8],
    batch: &[&crate::semdif::Change],
) -> Result<Vec<u8>, String> {
    let mut replacement = None;
    let mut lines = Vec::new();
    for change in batch {
        if let Some(contents) = change.replacement() {
            if replacement.is_some() {
                return Err(format!("{} has more than one replacement", path.display()));
            }
            replacement = Some(contents.bytes().to_vec());
        } else if change.line_edit().is_some() {
            lines.push(*change);
        }
    }
    if replacement.is_some() && !lines.is_empty() {
        return Err(format!(
            "{} has both line edits and replacement contents",
            path.display()
        ));
    }
    if let Some(contents) = replacement {
        return Ok(contents);
    }
    if lines.is_empty() {
        return Ok(bytes.to_vec());
    }
    let text = String::from_utf8(bytes.to_vec())
        .map_err(|_| format!("cannot edit lines in {}: file is not text", path.display()))?;
    let source_lines = crate::line_changes::split_text(&text);
    let mut plan = LinePlan::new();
    for change in &lines {
        let Some(line) = change.line_edit() else {
            continue;
        };
        line.rebuild(path, &source_lines.lines, &mut plan)?;
    }
    let rebuilt = plan.rebuild(path, &source_lines.lines)?;
    Ok(crate::line_changes::join_text(&crate::line_changes::Text {
        lines: rebuilt,
        trailing_newline: source_lines.trailing_newline,
    })
    .into_bytes())
}

/// Writes one file in the source directory.
///
/// An added file is created. A changed file is replaced with the recorded bytes.
pub(crate) fn write_file(root: &Path, relative: &str, contents: &[u8]) -> Result<(), String> {
    let path = path_inside(root, relative)?;
    create_parent(root, &path)?;
    fs::write(&path, contents).map_err(|error| format!("cannot write {}: {error}", path.display()))
}

/// Removes one file from the source directory.
///
/// The semdif records the path relative to that directory.
pub(crate) fn delete_file(root: &Path, relative: &str) -> Result<(), String> {
    let path = path_inside(root, relative)?;
    fs::remove_file(&path).map_err(|error| format!("cannot delete {}: {error}", path.display()))
}

/// Moves one file inside the source directory.
///
/// The semdif records the old and new paths relative to that directory.
pub(crate) fn rename_file(root: &Path, from: &str, to: &str) -> Result<(), String> {
    let from_path = path_inside(root, from)?;
    let to_path = path_inside(root, to)?;
    create_parent(root, &to_path)?;
    fs::rename(&from_path, &to_path).map_err(|error| {
        format!(
            "cannot rename {} to {}: {error}",
            from_path.display(),
            to_path.display()
        )
    })
}

/// Creates the directory that will hold a changed file.
///
/// A rename or addition can name a path in a directory that does not exist yet.
fn create_parent(root: &Path, path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if parent != root && !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
        }
    }
    Ok(())
}

/// A path that stays inside the source directory.
///
/// Change paths come from the semdif file, so a path that leaves the directory is rejected.
fn path_inside(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let path = Path::new(relative);
    if relative.is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(format!(
            "change path must stay inside the source directory: {relative}"
        ));
    }
    Ok(root.join(path))
}
