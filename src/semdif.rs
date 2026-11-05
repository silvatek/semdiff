use std::collections::BTreeMap;
use std::fs;
use std::process::ExitCode;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;

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
        Change::FileAdded { path, contents } => {
            document.push_str("  - scope: \".\"\n");
            document.push_str("    type: file-added\n");
            document.push_str("    details:\n");
            document.push_str(&format!("      path: {}\n", yaml_string(path)));
            write_file_contents(document, contents);
        }
        Change::FileChanged { path, contents } => {
            document.push_str("  - scope: \".\"\n");
            document.push_str("    type: file-changed\n");
            document.push_str("    details:\n");
            document.push_str(&format!("      path: {}\n", yaml_string(path)));
            write_file_contents(document, contents);
        }
        Change::LineAdded { path, line, text } => {
            write_line(document, "line-added", path, *line, text)
        }
        Change::LineRemoved { path, line, text } => {
            write_line(document, "line-removed", path, *line, text)
        }
        Change::LineModified { path, line, text } => {
            write_line(document, "line-modified", path, *line, text)
        }
        Change::DirectoryAdded { path } => write_directory(document, "directory-added", path),
        Change::DirectoryDeleted { path } => write_directory(document, "directory-deleted", path),
    }
}

fn write_line(document: &mut String, kind: &str, path: &str, line: usize, text: &str) {
    document.push_str("  - scope: \".\"\n");
    document.push_str(&format!("    type: {kind}\n"));
    document.push_str("    details:\n");
    document.push_str(&format!("      path: {}\n", yaml_string(path)));
    document.push_str(&format!("      line: {line}\n"));
    document.push_str(&format!("      text: {}\n", yaml_string(text)));
}

fn write_directory(document: &mut String, kind: &str, path: &str) {
    document.push_str("  - scope: \".\"\n");
    document.push_str(&format!("    type: {kind}\n"));
    document.push_str("    details:\n");
    document.push_str(&format!("      path: {}\n", yaml_string(path)));
}

/// Stores one file's bytes inside the semdif.
///
/// Text stays in the record as a quoted value, with quotes and line breaks escaped so
/// they do not change the record. Bytes that are not text are encoded, because a YAML
/// text value cannot hold them, and the original bytes still have to be restored.
fn write_file_contents(document: &mut String, contents: &[u8]) {
    match std::str::from_utf8(contents) {
        Ok(text) => {
            document.push_str(&format!("      contents: {}\n", yaml_string(text)));
        }
        Err(_) => {
            document.push_str("      encoding: \"base64\"\n");
            document.push_str(&format!(
                "      contents: {}\n",
                yaml_string(&STANDARD.encode(contents))
            ));
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
/// Paths and file contents must stay literal when they are written into the semdif
/// file, including when the text contains quotes or line breaks.
fn yaml_string(value: &str) -> String {
    let mut escaped = String::from("\"");
    for character in value.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            c if (c as u32) < 0x20 || matches!(c, '\u{0085}' | '\u{2028}' | '\u{2029}') => {
                escaped.push_str(&format!("\\u{code:04x}", code = c as u32));
            }
            c => escaped.push(c),
        }
    }
    escaped.push('"');
    escaped
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
    FileRenamed {
        from: String,
        to: String,
    },
    FileDeleted {
        path: String,
    },
    FileAdded {
        path: String,
        contents: Vec<u8>,
    },
    FileChanged {
        path: String,
        contents: Vec<u8>,
    },
    LineAdded {
        path: String,
        line: usize,
        text: String,
    },
    LineRemoved {
        path: String,
        line: usize,
        text: String,
    },
    LineModified {
        path: String,
        line: usize,
        text: String,
    },
    DirectoryAdded {
        path: String,
    },
    DirectoryDeleted {
        path: String,
    },
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
            "file-added" => Ok(Change::FileAdded {
                path: required(&self.fields, "path", "file-added")?,
                contents: stored_bytes(&self.fields, "file-added")?,
            }),
            "file-changed" => Ok(Change::FileChanged {
                path: required(&self.fields, "path", "file-changed")?,
                contents: stored_bytes(&self.fields, "file-changed")?,
            }),
            "line-added" => Ok(Change::LineAdded {
                path: required(&self.fields, "path", "line-added")?,
                line: required_line(&self.fields, "line-added")?,
                text: required(&self.fields, "text", "line-added")?,
            }),
            "line-removed" => Ok(Change::LineRemoved {
                path: required(&self.fields, "path", "line-removed")?,
                line: required_line(&self.fields, "line-removed")?,
                text: required(&self.fields, "text", "line-removed")?,
            }),
            "line-modified" => Ok(Change::LineModified {
                path: required(&self.fields, "path", "line-modified")?,
                line: required_line(&self.fields, "line-modified")?,
                text: required(&self.fields, "text", "line-modified")?,
            }),
            "directory-added" => Ok(Change::DirectoryAdded {
                path: required(&self.fields, "path", "directory-added")?,
            }),
            "directory-deleted" => Ok(Change::DirectoryDeleted {
                path: required(&self.fields, "path", "directory-deleted")?,
            }),
            "" => Err("change is missing a type".to_string()),
            other => Err(format!("unknown change type {other}")),
        }
    }
}

/// The bytes recorded for a file, restored from the semdif value.
///
/// Text is stored directly. Encoded bytes are decoded so the file can be written
/// back unchanged.
fn stored_bytes(fields: &BTreeMap<String, String>, kind: &str) -> Result<Vec<u8>, String> {
    let contents = required(fields, "contents", kind)?;
    match fields.get("encoding").map(String::as_str) {
        None => Ok(contents.into_bytes()),
        Some("base64") => STANDARD
            .decode(contents)
            .map_err(|_| format!("{kind} contents are not valid base64")),
        Some(other) => Err(format!("{kind} has unsupported encoding {other}")),
    }
}

/// A 1-based line number recorded for a line change.
fn required_line(fields: &BTreeMap<String, String>, kind: &str) -> Result<usize, String> {
    let text = required(fields, "line", kind)?;
    match text.parse::<usize>() {
        Ok(line) if line >= 1 => Ok(line),
        _ => Err(format!("{kind} has invalid line {text}")),
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
                Some('n') => decoded.push('\n'),
                Some('r') => decoded.push('\r'),
                Some('t') => decoded.push('\t'),
                Some('u') => decoded.push(decode_hex_char(&mut chars, value)?),
                _ => return Err(format!("invalid quoted value: {value}")),
            }
        } else {
            decoded.push(character);
        }
    }
    Ok(decoded)
}

fn decode_hex_char(chars: &mut std::str::Chars<'_>, value: &str) -> Result<char, String> {
    let mut hex = String::new();
    for _ in 0..4 {
        let Some(digit) = chars.next() else {
            return Err(format!("invalid quoted value: {value}"));
        };
        hex.push(digit);
    }
    let code =
        u32::from_str_radix(&hex, 16).map_err(|_| format!("invalid quoted value: {value}"))?;
    char::from_u32(code).ok_or_else(|| format!("invalid quoted value: {value}"))
}
