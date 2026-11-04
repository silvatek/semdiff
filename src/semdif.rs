use std::collections::BTreeMap;
use std::fs;
use std::process::ExitCode;

/// Saves the semdiff file and lists the shared paths.
///
/// The file is the command's result. The listing shows which files were eligible for
/// comparison. A failure to save the file returns exit code 1.
pub(crate) fn write_output(
    output: &str,
    sources: &crate::Sources,
    differences: &crate::Differences,
) -> ExitCode {
    let document = semdiff_document(
        env!("CARGO_PKG_VERSION"),
        &utc_timestamp(),
        &differences.renames,
        &differences.added_comments,
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

/// The semdiff text for one comparison.
///
/// The header is always present. Changes are included only when some were found, so an
/// unchanged pair does not gain an empty change list.
fn semdiff_document(
    version: &str,
    timestamp: &str,
    renames: &[(String, String)],
    added_comments: &[crate::comment::AddedLineComment],
) -> String {
    let mut document = header_document(version, timestamp);
    if renames.is_empty() && added_comments.is_empty() {
        return document;
    }
    document.push_str("changes:\n");
    for (from, to) in renames {
        document.push_str("  - scope: \".\"\n");
        document.push_str("    type: file-renamed\n");
        document.push_str("    details:\n");
        document.push_str(&format!("      from: {}\n", yaml_string(from)));
        document.push_str(&format!("      to: {}\n", yaml_string(to)));
    }
    for comment in added_comments {
        document.push_str(&format!("  - scope: {}\n", yaml_string(&comment.scope)));
        document.push_str("    type: line-comment-added\n");
        document.push_str("    details:\n");
        document.push_str(&format!("      line: {}\n", comment.line));
        document.push_str(&format!("      text: {}\n", yaml_string(&comment.text)));
    }
    document
}

/// The header that identifies a semdiff file.
///
/// It records which tool version produced the file and the UTC time it was produced.
fn header_document(version: &str, timestamp: &str) -> String {
    format!("header:\n  semdiff: \"{version}\"\n  timestamp: \"{timestamp}\"\n")
}

/// A YAML-safe form of a text value.
///
/// Paths must stay literal when they are written into the semdiff file.
fn yaml_string(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

/// The UTC time to record in a semdiff header.
///
/// Readers can see when the file was produced without depending on a local timezone.
fn utc_timestamp() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

/// One recorded edit from a semdiff file.
pub(crate) enum Change {
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

/// The changes recorded in a semdiff document.
///
/// Apply replays those records and does not need the header.
pub(crate) fn parse_changes(document: &str) -> Result<Vec<Change>, String> {
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
