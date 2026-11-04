use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Edits `source` so it gains the changes recorded in `patch`.
///
/// The semdif file is the record of what changed, so the same edits can be replayed
/// on another copy of that directory. A problem is reported on stderr.
pub(crate) fn apply(patch: &str, source: &str) -> ExitCode {
    match apply_changes(Path::new(patch), Path::new(source)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(1)
        }
    }
}

fn apply_changes(patch: &Path, source: &Path) -> Result<(), String> {
    let document = fs::read_to_string(patch)
        .map_err(|error| format!("cannot read {}: {error}", patch.display()))?;
    for change in crate::semdif::parse_changes(&document)? {
        apply_change(source, &change)?;
    }
    Ok(())
}

/// Performs one recorded change in the source directory.
fn apply_change(source: &Path, change: &crate::semdif::Change) -> Result<(), String> {
    match change {
        crate::semdif::Change::FileRenamed { from, to } => rename_file(source, from, to),
    }
}

/// Moves one file inside the source directory.
///
/// The semdif records the old and new paths relative to that directory.
fn rename_file(root: &Path, from: &str, to: &str) -> Result<(), String> {
    let from_path = path_inside(root, from)?;
    let to_path = path_inside(root, to)?;
    if let Some(parent) = to_path.parent() {
        if parent != root && !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
        }
    }
    fs::rename(&from_path, &to_path).map_err(|error| {
        format!(
            "cannot rename {} to {}: {error}",
            from_path.display(),
            to_path.display()
        )
    })
}

/// A path that stays inside the source directory.
///
/// Rename paths come from the semdif file, so a path that leaves the directory is rejected.
fn path_inside(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let path = Path::new(relative);
    if relative.is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(format!(
            "rename path must stay inside the source directory: {relative}"
        ));
    }
    Ok(root.join(path))
}
