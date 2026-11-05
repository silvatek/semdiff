use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

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
        if let Some(path) = content_path(change) {
            if !applied_content.insert(path.to_string()) {
                continue;
            }
            let batch: Vec<_> = changes
                .iter()
                .filter(|candidate| content_path(candidate) == Some(path))
                .collect();
            apply_content(source, path, &batch)?;
        } else {
            apply_change(source, change)?;
        }
    }
    Ok(())
}

/// The path of a content edit, when this change is one.
///
/// Line edits, a whole-file replacement, and the old and new hashes of a changed
/// file are applied together.
fn content_path(change: &crate::semdif::Change) -> Option<&str> {
    match change {
        crate::semdif::Change::LineAdded { path, .. }
        | crate::semdif::Change::LineRemoved { path, .. }
        | crate::semdif::Change::LineModified { path, .. }
        | crate::semdif::Change::FileChanged { path, .. }
        | crate::semdif::Change::FileDigest { path, .. } => Some(path),
        _ => None,
    }
}

/// Performs one recorded change in the source directory.
fn apply_change(source: &Path, change: &crate::semdif::Change) -> Result<(), String> {
    match change {
        crate::semdif::Change::FileRenamed { from, to } => rename_file(source, from, to),
        crate::semdif::Change::FileDeleted { path } => delete_file(source, path),
        crate::semdif::Change::FileAdded { path, contents } => write_file(source, path, contents),
        // Content edits are checked and written together in apply_content.
        crate::semdif::Change::FileChanged { .. }
        | crate::semdif::Change::FileDigest { .. }
        | crate::semdif::Change::LineAdded { .. }
        | crate::semdif::Change::LineRemoved { .. }
        | crate::semdif::Change::LineModified { .. } => Ok(()),
        crate::semdif::Change::DirectoryAdded { path } => add_directory(source, path),
        crate::semdif::Change::DirectoryDeleted { path } => remove_directory(source, path),
    }
}

/// Creates one directory in the source.
///
/// The semdif records an empty directory that exists only on the other side.
fn add_directory(root: &Path, relative: &str) -> Result<(), String> {
    let path = path_inside(root, relative)?;
    fs::create_dir_all(&path).map_err(|error| format!("cannot create {}: {error}", path.display()))
}

/// Removes one empty directory from the source.
///
/// Files inside it have already been moved or deleted. A directory that still
/// holds something is left in place and reported.
fn remove_directory(root: &Path, relative: &str) -> Result<(), String> {
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
        if let crate::semdif::Change::FileDigest {
            path,
            old_hash,
            new_hash,
        } = change
        {
            if found.is_some() {
                return Err(format!("{path} has more than one file-changed hash"));
            }
            found = Some((old_hash.clone(), new_hash.clone()));
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
        match change {
            crate::semdif::Change::FileChanged { contents, .. } => {
                if replacement.is_some() {
                    return Err(format!("{} has more than one replacement", path.display()));
                }
                replacement = Some(contents.clone());
            }
            crate::semdif::Change::LineAdded { .. }
            | crate::semdif::Change::LineRemoved { .. }
            | crate::semdif::Change::LineModified { .. } => lines.push(*change),
            _ => {}
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
    let rebuilt = rebuild_lines(path, &source_lines.lines, &lines)?;
    Ok(crate::line_changes::join_text(&crate::line_changes::Text {
        lines: rebuilt,
        trailing_newline: source_lines.trailing_newline,
    })
    .into_bytes())
}

fn rebuild_lines(
    path: &Path,
    lines: &[String],
    batch: &[&crate::semdif::Change],
) -> Result<Vec<String>, String> {
    let mut modified = std::collections::BTreeMap::new();
    let mut removed = BTreeSet::new();
    let mut added = Vec::new();
    for change in batch {
        match change {
            crate::semdif::Change::LineModified { old, text, .. } => {
                if modified.insert(*old, text.clone()).is_some() {
                    return Err(format!(
                        "line {old} is modified more than once in {}",
                        path.display()
                    ));
                }
            }
            crate::semdif::Change::LineRemoved { old, .. } => {
                if !removed.insert(*old) {
                    return Err(format!(
                        "line {old} is removed more than once in {}",
                        path.display()
                    ));
                }
            }
            crate::semdif::Change::LineAdded { old, new, text, .. } => {
                added.push((*old, *new, text.clone()));
            }
            _ => {}
        }
    }
    added.sort_by_key(|(old, new, _)| (*old, *new));
    let count = lines.len();
    for old in modified.keys().chain(removed.iter()) {
        if *old == 0 || *old > count {
            return Err(format!(
                "line {old} is outside {} which has {count} lines",
                path.display()
            ));
        }
    }
    for (old, _, _) in &added {
        if *old == 0 || *old > count + 1 {
            return Err(format!(
                "cannot insert before line {old} in {}: file has {count} lines",
                path.display()
            ));
        }
    }
    for old in modified.keys() {
        if removed.contains(old) {
            return Err(format!(
                "line {old} is both modified and removed in {}",
                path.display()
            ));
        }
    }

    let mut rebuilt = Vec::new();
    for old in 1..=count {
        for (_, _, text) in added.iter().filter(|(before, _, _)| *before == old) {
            rebuilt.push(text.clone());
        }
        if removed.contains(&old) {
            continue;
        }
        if let Some(text) = modified.get(&old) {
            rebuilt.push(text.clone());
        } else {
            rebuilt.push(lines[old - 1].clone());
        }
    }
    for (_, _, text) in added.iter().filter(|(before, _, _)| *before == count + 1) {
        rebuilt.push(text.clone());
    }
    Ok(rebuilt)
}

/// Writes one file in the source directory.
///
/// An added file is created. A changed file is replaced with the recorded bytes.
fn write_file(root: &Path, relative: &str, contents: &[u8]) -> Result<(), String> {
    let path = path_inside(root, relative)?;
    create_parent(root, &path)?;
    fs::write(&path, contents).map_err(|error| format!("cannot write {}: {error}", path.display()))
}

/// Removes one file from the source directory.
///
/// The semdif records the path relative to that directory.
fn delete_file(root: &Path, relative: &str) -> Result<(), String> {
    let path = path_inside(root, relative)?;
    fs::remove_file(&path).map_err(|error| format!("cannot delete {}: {error}", path.display()))
}

/// Moves one file inside the source directory.
///
/// The semdif records the old and new paths relative to that directory.
fn rename_file(root: &Path, from: &str, to: &str) -> Result<(), String> {
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
