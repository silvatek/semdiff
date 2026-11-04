use std::collections::BTreeMap;
use std::fs;
use std::process::ExitCode;

/// Saves the semdif file and lists the shared paths.
///
/// The file is the command's result. The listing shows which files were eligible for
/// comparison. A failure to save the file returns exit code 1.
pub(crate) fn write_output(
    output: &str,
    sources: &crate::Sources,
    differences: &crate::Differences,
) -> ExitCode {
    let document = semdif_document(
        env!("CARGO_PKG_VERSION"),
        &utc_timestamp(),
        &differences.changes,
    );
    if let Err(error) = fs::write(output, document) {
        eprintln!("cannot write {output}: {error}");
        return ExitCode::from(1);
    }
    println!("Semdiff {}", env!("CARGO_PKG_VERSION"));
    println!("Comparing {} and {}", sources.left, sources.right);
    for path in &differences.paths {
        println!("{path}");
    }
    ExitCode::SUCCESS
}

/// The semdif text for one comparison.
///
/// The header is always present. Changes are included only when some were found, so an
/// unchanged pair does not gain an empty change list.
fn semdif_document(version: &str, timestamp: &str, changes: &[Change]) -> String {
    let mut document = header_document(version, timestamp);
    if changes.is_empty() {
        return document;
    }
    document.push_str("changes:\n");
    for change in changes {
        write_change(&mut document, change);
    }
    document
}

fn write_change(document: &mut String, change: &Change) {
    match change {
        Change::FileRenamed { from, to } => {
            document.push_str("  - scope: \".\"\n");
            document.push_str("    type: file-renamed\n");
            document.push_str("    details:\n");
            document.push_str(&format!("      from: {}\n", yaml_string(from)));
            document.push_str(&format!("      to: {}\n", yaml_string(to)));
        }
        Change::FileDeleted { path } => {
            document.push_str("  - scope: \".\"\n");
            document.push_str("    type: file-deleted\n");
            document.push_str("    details:\n");
            document.push_str(&format!("      path: {}\n", yaml_string(path)));
        }
    }
}

/// The header that identifies a semdif file.
///
/// It records which tool version produced the file and the UTC time it was produced.
fn header_document(version: &str, timestamp: &str) -> String {
    format!("header:\n  semdiff: \"{version}\"\n  timestamp: \"{timestamp}\"\n")
}

/// A YAML-safe form of a text value.
///
/// Paths must stay literal when they are written into the semdif file.
fn yaml_string(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

/// The UTC time to record in a semdif header.
///
/// Readers can see when the file was produced without depending on a local timezone.
fn utc_timestamp() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

/// One change recorded in a semdif file.
///
/// Apply replays these in the order they were recorded. Each kind carries the
/// details that kind needs.
pub(crate) enum Change {
    FileRenamed { from: String, to: String },
    FileDeleted { path: String },
}

/// The changes recorded in a semdif file.
///
/// The header is not needed to replay the edits. A change whose kind is not known
/// is rejected so an unsupported edit is not skipped.
pub(crate) fn parse_changes(document: &str) -> Result<Vec<Change>, String> {
    let mut lines = document.lines();
    if lines.find(|line| line.trim() == "changes:").is_none() {
        return Ok(Vec::new());
    }
    let mut found = Vec::new();
    let mut record: Option<PartialChange> = None;
    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("- ") {
            if let Some(done) = record.take() {
                found.push(done.finish()?);
            }
            record = Some(PartialChange::start(rest)?);
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

/// Fields of one change record, gathered before its kind is known.
struct PartialChange {
    scope: String,
    kind: String,
    fields: BTreeMap<String, String>,
}

impl PartialChange {
    fn start(text: &str) -> Result<Self, String> {
        let mut record = Self {
            scope: String::new(),
            kind: String::new(),
            fields: BTreeMap::new(),
        };
        record.read(text)?;
        Ok(record)
    }

    fn read(&mut self, text: &str) -> Result<(), String> {
        let (key, value) = field(text)?;
        match key.as_str() {
            "scope" => self.scope = value,
            "type" => self.kind = value,
            "details" => {}
            other => {
                self.fields.insert(other.to_string(), value);
            }
        }
        Ok(())
    }

    fn finish(self) -> Result<Change, String> {
        if self.scope.is_empty() {
            return Err("change is missing a scope".to_string());
        }
        match self.kind.as_str() {
            "file-renamed" => Ok(Change::FileRenamed {
                from: required(&self.fields, "from", "file-renamed")?,
                to: required(&self.fields, "to", "file-renamed")?,
            }),
            "file-deleted" => Ok(Change::FileDeleted {
                path: required(&self.fields, "path", "file-deleted")?,
            }),
            "" => Err("change is missing a type".to_string()),
            other => Err(format!("unknown change type {other}")),
        }
    }
}

/// One detail that a change kind requires.
fn required(fields: &BTreeMap<String, String>, key: &str, kind: &str) -> Result<String, String> {
    fields
        .get(key)
        .cloned()
        .ok_or_else(|| format!("{kind} is missing {key}"))
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
