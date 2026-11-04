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
    let mut scope = String::new();
    let mut earlier_delta = 0;
    for change in changes {
        if change.scope != scope {
            scope = change.scope.clone();
            earlier_delta = 0;
        }
        match &change.details {
            crate::semdif::ChangeDetails::SymbolRenamed { from, to } => {
                crate::symbol::apply_rename(source, &change.scope, from, to)?;
            }
            crate::semdif::ChangeDetails::LineCommentAdded { line, text } => {
                let line = line_in_current_file(*line, earlier_delta);
                insert_line(&source.join(&change.scope), line, text)?;
            }
            crate::semdif::ChangeDetails::FileRenamed { from, to } => {
                rename_file(source, from, to)?;
            }
        }
        earlier_delta += change.line_delta;
    }
    Ok(())
}

/// The line to edit in the file as it stands.
///
/// `new_line` is the line in the final text. `earlier_delta` is how many lines
/// changes already applied in this file added or removed, so the old line and the
/// current line stay related.
fn line_in_current_file(new_line: usize, earlier_delta: i32) -> usize {
    let new_line = i32::try_from(new_line).unwrap_or(i32::MAX);
    let old_line = new_line.saturating_sub(earlier_delta);
    usize::try_from(old_line + earlier_delta).unwrap_or(1)
}

/// Puts symbol renames, then added comments, then file renames.
///
/// A comment is recorded against the original path, and its line number assumes earlier
/// comments are already present. A symbol rename does not add or remove lines.
fn change_order(change: &Change) -> (u8, String, usize) {
    match &change.details {
        crate::semdif::ChangeDetails::SymbolRenamed { from, .. } => (0, from.clone(), 0),
        crate::semdif::ChangeDetails::LineCommentAdded { line, .. } => {
            (1, change.scope.clone(), *line)
        }
        crate::semdif::ChangeDetails::FileRenamed { from, .. } => (2, from.clone(), 0),
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
