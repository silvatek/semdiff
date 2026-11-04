use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Renames files in `source` as recorded in `patch`.
///
/// The semdif file is the record of what moved, so the same renames can be replayed
/// on another copy of that directory. A rename is the only change that can be applied.
/// A problem is reported on stderr.
pub(crate) fn apply(source: &str, patch: &str) -> ExitCode {
    match apply_renames(Path::new(source), Path::new(patch)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(1)
        }
    }
}

fn apply_renames(source: &Path, patch: &Path) -> Result<(), String> {
    let document = fs::read_to_string(patch)
        .map_err(|error| format!("cannot read {}: {error}", patch.display()))?;
    for rename in renames(&document)? {
        rename_file(source, &rename.from, &rename.to)?;
    }
    Ok(())
}

/// One file that moved inside the source directory.
struct Rename {
    from: String,
    to: String,
}

/// The renames recorded in a semdif file.
///
/// Apply replays those records onto one directory. Any other change is rejected
/// so an unsupported edit is not skipped.
fn renames(document: &str) -> Result<Vec<Rename>, String> {
    let mut lines = document.lines();
    if lines.find(|line| line.trim() == "changes:").is_none() {
        return Ok(Vec::new());
    }
    let mut found = Vec::new();
    let mut record: Option<PartialRename> = None;
    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("- ") {
            if let Some(done) = record.take() {
                found.push(done.finish()?);
            }
            record = Some(PartialRename::start(rest)?);
            continue;
        }
        let record = record
            .as_mut()
            .ok_or_else(|| format!("unexpected field in semdif: {trimmed}"))?;
        record.read(trimmed)?;
    }
    if let Some(done) = record {
        found.push(done.finish()?);
    }
    Ok(found)
}

/// Fields of one change record, gathered before the rename is known to be complete.
struct PartialRename {
    kind: String,
    from: Option<String>,
    to: Option<String>,
}

impl PartialRename {
    fn start(text: &str) -> Result<Self, String> {
        let mut record = Self {
            kind: String::new(),
            from: None,
            to: None,
        };
        record.read(text)?;
        Ok(record)
    }

    fn read(&mut self, text: &str) -> Result<(), String> {
        let (key, value) = field(text)?;
        match key.as_str() {
            "type" => self.kind = value,
            "from" => self.from = Some(value),
            "to" => self.to = Some(value),
            _ => {}
        }
        Ok(())
    }

    fn finish(self) -> Result<Rename, String> {
        match self.kind.as_str() {
            "file-renamed" => Ok(Rename {
                from: self.from.ok_or("file-renamed is missing from")?,
                to: self.to.ok_or("file-renamed is missing to")?,
            }),
            "" => Err("change is missing a type".to_string()),
            other => Err(format!("unknown change type {other}")),
        }
    }
}

/// One key and its value from a semdif line.
fn field(text: &str) -> Result<(String, String), String> {
    let (key, value) = text
        .split_once(':')
        .ok_or_else(|| format!("invalid semdif line: {text}"))?;
    Ok((key.trim().to_string(), unquote(value.trim())?))
}

/// The text stored in a semdif value.
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
