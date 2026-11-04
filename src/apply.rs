use std::fs;
use std::path::Path;
use std::process::ExitCode;

use crate::semdif::Change;

/// Edits `source` so it gains the changes recorded in `patch`.
///
/// The semdif file is the record of what changed, so the same edit can be replayed
/// on another copy of the original source. A problem is reported on stderr.
pub(crate) fn apply(source: &str, patch: &str) -> ExitCode {
    match apply_patch(Path::new(source), Path::new(patch)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(1)
        }
    }
}

fn apply_patch(source: &Path, patch: &Path) -> Result<(), String> {
    let document = fs::read_to_string(patch)
        .map_err(|error| format!("cannot read {}: {error}", patch.display()))?;
    let mut changes = crate::semdif::parse_changes(&document)?;
    changes.sort_by(|left, right| change_order(left).cmp(&change_order(right)));
    for change in changes {
        match change {
            Change::LineCommentAdded { scope, line, text } => {
                insert_line(&source.join(&scope), line, &text)?;
            }
            Change::FileRenamed { from, to } => rename_file(source, &from, &to)?,
        }
    }
    Ok(())
}

/// Puts content edits before renames, and earlier comment lines before later ones.
///
/// A comment is recorded against the original path, and its line number assumes earlier
/// comments are already present.
fn change_order(change: &Change) -> (u8, String, usize) {
    match change {
        Change::LineCommentAdded { scope, line, .. } => (0, scope.clone(), *line),
        Change::FileRenamed { from, .. } => (1, from.clone(), 0),
    }
}

/// Inserts `text` so it occupies the 1-based line from the semdif record.
///
/// Those line numbers refer to the file after earlier additions, so comments land
/// where the comparison saw them.
fn insert_line(path: &Path, line: usize, text: &str) -> Result<(), String> {
    if line == 0 {
        return Err(format!("line numbers start at 1 in {}", path.display()));
    }
    let original = fs::read_to_string(path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let newline_at_end = original.ends_with('\n');
    let mut lines: Vec<String> = original.lines().map(str::to_string).collect();
    let index = line - 1;
    if index > lines.len() {
        return Err(format!(
            "cannot insert at line {line} in {}",
            path.display()
        ));
    }
    lines.insert(index, text.to_string());
    let mut updated = lines.join("\n");
    if newline_at_end {
        updated.push('\n');
    }
    fs::write(path, updated).map_err(|error| format!("cannot write {}: {error}", path.display()))
}

/// Moves a file within the source tree.
///
/// The comparison records a rename by its old and new relative paths.
fn rename_file(root: &Path, from: &str, to: &str) -> Result<(), String> {
    let from_path = root.join(from);
    let to_path = root.join(to);
    if let Some(parent) = to_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    }
    fs::rename(&from_path, &to_path).map_err(|error| {
        format!(
            "cannot rename {} to {}: {error}",
            from_path.display(),
            to_path.display()
        )
    })
}
