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
    for change in crate::semdif::parse_changes(&document)? {
        apply_change(source, &change)?;
    }
    Ok(())
}

/// Performs one recorded change in the source directory.
fn apply_change(source: &Path, change: &crate::semdif::Change) -> Result<(), String> {
    match change {
        crate::semdif::Change::FileRenamed { from, to } => rename_file(source, from, to),
        crate::semdif::Change::FileDeleted { path } => delete_file(source, path),
        crate::semdif::Change::FileAdded { path, contents }
        | crate::semdif::Change::FileChanged { path, contents } => {
            write_file(source, path, contents)
        }
        crate::semdif::Change::LineAdded { path, line, text } => {
            edit_lines(source, path, |file, lines| {
                insert_line(file, lines, *line, text)
            })
        }
        crate::semdif::Change::LineRemoved { path, line, .. } => {
            edit_lines(source, path, |file, lines| remove_line(file, lines, *line))
        }
        crate::semdif::Change::LineModified { path, line, text } => {
            edit_lines(source, path, |file, lines| {
                replace_line(file, lines, *line, text)
            })
        }
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

/// Inserts, removes, or replaces one line in a text file.
///
/// The file keeps its trailing newline. The line records describe the lines, and
/// the newline that ends the file stays as it was.
fn edit_lines(
    root: &Path,
    relative: &str,
    edit: impl FnOnce(&Path, &mut crate::line_changes::Text) -> Result<(), String>,
) -> Result<(), String> {
    let path = path_inside(root, relative)?;
    let bytes =
        fs::read(&path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let text = String::from_utf8(bytes)
        .map_err(|_| format!("cannot edit lines in {}: file is not text", path.display()))?;
    let mut lines = crate::line_changes::split_text(&text);
    edit(&path, &mut lines)?;
    fs::write(&path, crate::line_changes::join_text(&lines))
        .map_err(|error| format!("cannot write {}: {error}", path.display()))
}

fn insert_line(
    path: &Path,
    lines: &mut crate::line_changes::Text,
    line: usize,
    text: &str,
) -> Result<(), String> {
    if line == 0 || line > lines.lines.len() + 1 {
        return Err(format!(
            "cannot add line {line} in {}: file has {} lines",
            path.display(),
            lines.lines.len()
        ));
    }
    lines.lines.insert(line - 1, text.to_string());
    Ok(())
}

fn remove_line(
    path: &Path,
    lines: &mut crate::line_changes::Text,
    line: usize,
) -> Result<(), String> {
    if line == 0 || line > lines.lines.len() {
        return Err(format!(
            "cannot remove line {line} in {}: file has {} lines",
            path.display(),
            lines.lines.len()
        ));
    }
    lines.lines.remove(line - 1);
    Ok(())
}

fn replace_line(
    path: &Path,
    lines: &mut crate::line_changes::Text,
    line: usize,
    text: &str,
) -> Result<(), String> {
    if line == 0 || line > lines.lines.len() {
        return Err(format!(
            "cannot modify line {line} in {}: file has {} lines",
            path.display(),
            lines.lines.len()
        ));
    }
    lines.lines[line - 1] = text.to_string();
    Ok(())
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
