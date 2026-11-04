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

/// One change written into the semdif file.
///
/// The scope, type, and details are the record. The line delta is kept for the
/// comparison and is not written, because line numbers in the record are already
/// those of the new text.
fn write_change(document: &mut String, change: &Change) {
    document.push_str(&format!("  - scope: {}\n", yaml_string(&change.scope)));
    document.push_str(&format!("    type: {}\n", change.change_type.as_str()));
    document.push_str("    details:\n");
    match &change.details {
        ChangeDetails::FileRenamed { from, to } | ChangeDetails::SymbolRenamed { from, to } => {
            document.push_str(&format!("      from: {}\n", yaml_string(from)));
            document.push_str(&format!("      to: {}\n", yaml_string(to)));
        }
        ChangeDetails::LineCommentAdded { line, text } => {
            document.push_str(&format!("      line: {line}\n"));
            document.push_str(&format!("      text: {}\n", yaml_string(text)));
        }
    }
}

/// The header that identifies a semdif file.
///
/// It records which semdiff version produced the file and the UTC time it was produced.
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

/// One difference between the sources.
///
/// The scope is where the change applies, the type and details say what changed,
/// and `line_delta` is how many lines that change adds or removes. Adding two lines
/// is `2` and removing one line is `-1`, so a later change can relate a line number
/// in the new text to one in the old text.
pub(crate) struct Change {
    pub scope: String,
    pub change_type: ChangeType,
    pub details: ChangeDetails,
    pub line_delta: i32,
}

/// The kind of difference a change records.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ChangeType {
    FileRenamed,
    SymbolRenamed,
    LineCommentAdded,
}

/// The fields that belong to a change type.
pub(crate) enum ChangeDetails {
    FileRenamed { from: String, to: String },
    SymbolRenamed { from: String, to: String },
    LineCommentAdded { line: usize, text: String },
}

impl ChangeType {
    fn as_str(self) -> &'static str {
        match self {
            Self::FileRenamed => "file-renamed",
            Self::SymbolRenamed => "symbol-renamed",
            Self::LineCommentAdded => "line-comment-added",
        }
    }

    /// How many lines one change of this type adds or removes.
    ///
    /// A renamed symbol or file keeps the same lines. An added comment line occupies
    /// one new line.
    fn line_delta(self) -> i32 {
        match self {
            Self::LineCommentAdded => 1,
            Self::FileRenamed | Self::SymbolRenamed => 0,
        }
    }
}

impl Change {
    /// A file whose name changed and whose contents did not.
    pub(crate) fn file_renamed(from: String, to: String) -> Self {
        Self::new(
            ".".to_string(),
            ChangeType::FileRenamed,
            ChangeDetails::FileRenamed { from, to },
        )
    }

    /// A code identifier renamed everywhere in `scope`.
    pub(crate) fn symbol_renamed(scope: String, from: String, to: String) -> Self {
        Self::new(
            scope,
            ChangeType::SymbolRenamed,
            ChangeDetails::SymbolRenamed { from, to },
        )
    }

    /// One comment line present in the new file and absent from the old one.
    pub(crate) fn line_comment_added(scope: String, line: usize, text: String) -> Self {
        Self::new(
            scope,
            ChangeType::LineCommentAdded,
            ChangeDetails::LineCommentAdded { line, text },
        )
    }

    fn new(scope: String, change_type: ChangeType, details: ChangeDetails) -> Self {
        Self {
            scope,
            line_delta: change_type.line_delta(),
            change_type,
            details,
        }
    }
}

/// The changes recorded in a semdif document.
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
    /// The typed change described by one semdif record.
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
                Ok(Change::line_comment_added(self.scope, line, text))
            }
            "file-renamed" => Ok(Change::file_renamed(
                self.details
                    .get("from")
                    .ok_or("file-renamed is missing from")?
                    .clone(),
                self.details
                    .get("to")
                    .ok_or("file-renamed is missing to")?
                    .clone(),
            )),
            "symbol-renamed" => Ok(Change::symbol_renamed(
                self.scope,
                self.details
                    .get("from")
                    .ok_or("symbol-renamed is missing from")?
                    .clone(),
                self.details
                    .get("to")
                    .ok_or("symbol-renamed is missing to")?
                    .clone(),
            )),
            other => Err(format!("unknown change type {other}")),
        }
    }
}

/// One key and its value from a semdif line.
///
/// Values may be quoted so paths and comments stay literal.
fn split_field(text: &str) -> Result<(String, String), String> {
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

#[cfg(test)]
mod tests {
    use super::{Change, ChangeType};

    #[test]
    fn line_delta_is_the_lines_added_or_removed() {
        let comment =
            Change::line_comment_added("src/main.rs".to_string(), 2, "// note".to_string());
        assert_eq!(comment.change_type, ChangeType::LineCommentAdded);
        assert_eq!(comment.line_delta, 1);
        let rename = Change::symbol_renamed(
            "src/main.rs".to_string(),
            "fibonacci".to_string(),
            "fib".to_string(),
        );
        assert_eq!(rename.line_delta, 0);
        assert_eq!(
            Change::file_renamed("a.txt".to_string(), "b.txt".to_string()).line_delta,
            0
        );
    }
}
