use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

/// Edits `source` so it gains the changes recorded in `patch`.
///
/// The semdiff file is the record of what changed, so the same edit can be replayed
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
    let mut changes = parse_changes(&document)?;
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

enum Change {
    LineCommentAdded {
        scope: String,
        line: usize,
        text: String,
    },
    FileRenamed {
        from: String,
        to: String,
    },
}

/// Inserts `text` so it occupies the 1-based line from the semdiff record.
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

/// The changes recorded in a semdiff document.
///
/// Apply replays those records and does not need the header.
fn parse_changes(document: &str) -> Result<Vec<Change>, String> {
    let mut changes = Vec::new();
    let mut current: Option<RawChange> = None;
    let mut in_changes = false;
    for line in document.lines() {
        let trimmed = line.trim();
        if trimmed == "changes:" {
            in_changes = true;
            continue;
        }
        if !in_changes || trimmed.is_empty() {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("- ") {
            if let Some(finished) = current.take() {
                changes.push(finished.into_change()?);
            }
            let (key, value) = split_field(rest)?;
            if key != "scope" {
                return Err(format!("expected scope, found {key}"));
            }
            current = Some(RawChange {
                scope: value,
                kind: String::new(),
                details: BTreeMap::new(),
            });
            continue;
        }
        let (key, value) = split_field(trimmed)?;
        let current = current
            .as_mut()
            .ok_or_else(|| format!("unexpected field {key}"))?;
        if key == "type" {
            current.kind = value;
        } else if key != "details" {
            current.details.insert(key, value);
        }
    }
    if let Some(finished) = current {
        changes.push(finished.into_change()?);
    }
    Ok(changes)
}

struct RawChange {
    scope: String,
    kind: String,
    details: BTreeMap<String, String>,
}

impl RawChange {
    /// The typed change described by one semdiff record.
    ///
    /// An unknown record is rejected so an unsupported edit is not skipped.
    fn into_change(self) -> Result<Change, String> {
        match self.kind.as_str() {
            "line-comment-added" => {
                let line = self
                    .details
                    .get("line")
                    .ok_or("line-comment-added is missing line")?
                    .parse()
                    .map_err(|_| format!("invalid line in {}", self.scope))?;
                let text = self
                    .details
                    .get("text")
                    .ok_or("line-comment-added is missing text")?
                    .clone();
                Ok(Change::LineCommentAdded {
                    scope: self.scope,
                    line,
                    text,
                })
            }
            "file-renamed" => Ok(Change::FileRenamed {
                from: self
                    .details
                    .get("from")
                    .ok_or("file-renamed is missing from")?
                    .clone(),
                to: self
                    .details
                    .get("to")
                    .ok_or("file-renamed is missing to")?
                    .clone(),
            }),
            other => Err(format!("unknown change type {other}")),
        }
    }
}

/// One key and its value from a semdiff line.
///
/// Values may be quoted so paths and comments stay literal.
fn split_field(text: &str) -> Result<(String, String), String> {
    let (key, value) = text
        .split_once(':')
        .ok_or_else(|| format!("invalid semdiff line: {text}"))?;
    Ok((key.trim().to_string(), unquote(value.trim())?))
}

/// The text stored in a semdiff value.
///
/// A quoted value can contain characters that would otherwise change the record.
fn unquote(value: &str) -> Result<String, String> {
    let Some(inner) = value.strip_prefix('"') else {
        return Ok(value.to_string());
    };
    let Some(inner) = inner.strip_suffix('"') else {
        return Err(format!("invalid quoted value: {value}"));
    };
    let mut decoded = String::new();
    let mut chars = inner.chars();
    while let Some(character) = chars.next() {
        if character == '\\' {
            match chars.next() {
                Some('\\') => decoded.push('\\'),
                Some('"') => decoded.push('"'),
                Some(other) => {
                    decoded.push('\\');
                    decoded.push(other);
                }
                None => return Err(format!("invalid quoted value: {value}")),
            }
        } else {
            decoded.push(character);
        }
    }
    Ok(decoded)
}
