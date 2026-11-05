use std::collections::BTreeMap;
use std::path::Path;
use std::process::ExitCode;

use crate::filesystem::FileSystem;

use crate::change::Change;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;

/// Saves the semdif file and lists the shared paths.
///
/// The file is the command's result. The listing shows which files were eligible for
/// comparison. A failure to save the file returns exit code 1.
pub(crate) fn document(changes: &[Box<dyn Change>]) -> String {
    semdif_document(env!("CARGO_PKG_VERSION"), &utc_timestamp(), changes)
}

pub(crate) fn write_output(
    fs: &dyn FileSystem,
    output: &str,
    sources: &crate::Sources,
    differences: &crate::Differences,
) -> ExitCode {
    let document = document(&differences.changes);
    if let Err(error) = fs.write(Path::new(output), document.as_bytes()) {
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
fn semdif_document(version: &str, timestamp: &str, changes: &[Box<dyn Change>]) -> String {
    let mut document = header_document(version, timestamp);
    if changes.is_empty() {
        return document;
    }
    document.push_str("changes:\n");
    for change in changes {
        write_change(&mut document, change.as_ref());
    }
    document
}

fn write_change(document: &mut String, change: &dyn Change) {
    document.push_str(&format!("  - scope: {}\n", yaml_string(change.scope())));
    document.push_str(&format!("    type: {}\n", change.kind()));
    document.push_str("    details:\n");
    change.write_details(document);
}

/// One named detail whose value is quoted.
pub(crate) fn write_detail(document: &mut String, key: &str, value: &str) {
    document.push_str(&format!("      {key}: {}\n", yaml_string(value)));
}

/// One named detail whose value is already in the form the file should show.
pub(crate) fn write_plain(document: &mut String, key: &str, value: &str) {
    document.push_str(&format!("      {key}: {value}\n"));
}

/// Stores one file's bytes inside the semdif.
///
/// Text stays in the record as a quoted value, with quotes and line breaks escaped so
/// they do not change the record. Bytes that are not text are encoded, because a YAML
/// text value cannot hold them, and the original bytes still have to be restored.
pub(crate) fn write_file_contents(document: &mut String, contents: &[u8]) {
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

/// The changes recorded in a semdif file.
///
/// The header is not needed to replay the edits. A change whose kind is not known
/// is rejected so an unsupported edit is not skipped.
pub(crate) fn parse_changes(document: &str) -> Result<Vec<Box<dyn Change>>, String> {
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

    fn finish(self) -> Result<Box<dyn Change>, String> {
        if self.scope.is_empty() {
            return Err("change is missing a scope".to_string());
        }
        if self.kind.is_empty() {
            return Err("change is missing a type".to_string());
        }
        let parsers: &[(&str, Parser)] = &[
            ("file-renamed", parse_file_renamed),
            ("file-deleted", parse_file_deleted),
            ("file-added", parse_file_added),
            ("file-changed", file_changed),
            ("line-added", parse_line_added),
            ("line-removed", parse_line_removed),
            ("line-modified", parse_line_modified),
            ("identifier-renamed", parse_identifier_renamed),
            ("directory-added", parse_directory_added),
            ("directory-deleted", parse_directory_deleted),
        ];
        match parsers.iter().find(|(kind, _)| *kind == self.kind) {
            Some((_, parse)) => parse(self.scope, self.fields),
            None => Err(format!("unknown change type {}", self.kind)),
        }
    }
}

type Parser = fn(String, BTreeMap<String, String>) -> Result<Box<dyn Change>, String>;

fn parse_file_renamed(
    scope: String,
    fields: BTreeMap<String, String>,
) -> Result<Box<dyn Change>, String> {
    Ok(Box::new(crate::change::FileRenamed {
        scope,
        from: required(&fields, "from", "file-renamed")?,
        to: required(&fields, "to", "file-renamed")?,
    }))
}

fn parse_file_deleted(
    scope: String,
    fields: BTreeMap<String, String>,
) -> Result<Box<dyn Change>, String> {
    Ok(Box::new(crate::change::FileDeleted {
        scope,
        path: required(&fields, "path", "file-deleted")?,
    }))
}

fn parse_file_added(
    scope: String,
    fields: BTreeMap<String, String>,
) -> Result<Box<dyn Change>, String> {
    Ok(Box::new(crate::change::FileAdded {
        scope,
        path: required(&fields, "path", "file-added")?,
        contents: stored_bytes(&fields, "file-added")?,
    }))
}

fn parse_line_added(
    scope: String,
    fields: BTreeMap<String, String>,
) -> Result<Box<dyn Change>, String> {
    let text = required(&fields, "text", "line-added")?;
    expect_added_lines(&fields, &text)?;
    Ok(Box::new(crate::change::LineAdded::new(
        scope,
        required_line(&fields, "old", "line-added")?,
        required_line(&fields, "new", "line-added")?,
        text,
    )))
}

fn parse_line_removed(
    scope: String,
    fields: BTreeMap<String, String>,
) -> Result<Box<dyn Change>, String> {
    let old = required_line(&fields, "old", "line-removed")?;
    let text = required(&fields, "text", "line-removed")?;
    expect_removed_lines(&fields, old, &text)?;
    Ok(Box::new(crate::change::LineRemoved::new(
        scope,
        old,
        required_line(&fields, "new", "line-removed")?,
        text,
    )))
}

fn parse_line_modified(
    scope: String,
    fields: BTreeMap<String, String>,
) -> Result<Box<dyn Change>, String> {
    let text = required(&fields, "text", "line-modified")?;
    expect_modified_lines(&fields)?;
    Ok(Box::new(crate::change::LineModified::new(
        scope,
        required_line(&fields, "old", "line-modified")?,
        required_line(&fields, "new", "line-modified")?,
        text,
    )))
}

fn parse_identifier_renamed(
    scope: String,
    fields: BTreeMap<String, String>,
) -> Result<Box<dyn Change>, String> {
    let span = crate::change::parse_span_scope(&scope)?;
    Ok(Box::new(crate::change::IdentifierRenamed::new(
        span,
        required_line(&fields, "new", "identifier-renamed")?,
        required(&fields, "oldname", "identifier-renamed")?,
        required(&fields, "newname", "identifier-renamed")?,
    )))
}

fn parse_directory_added(
    scope: String,
    fields: BTreeMap<String, String>,
) -> Result<Box<dyn Change>, String> {
    Ok(Box::new(crate::change::DirectoryAdded {
        scope,
        path: required(&fields, "path", "directory-added")?,
    }))
}

fn parse_directory_deleted(
    scope: String,
    fields: BTreeMap<String, String>,
) -> Result<Box<dyn Change>, String> {
    Ok(Box::new(crate::change::DirectoryDeleted {
        scope,
        path: required(&fields, "path", "directory-deleted")?,
    }))
}

/// A `file-changed` change: the old and new hashes, or bytes that replace the file.
///
/// The file path is the change's scope. Hashes are `old` and `new`. A
/// replacement has `contents` instead. Both are the same change type.
fn file_changed(
    scope: String,
    fields: BTreeMap<String, String>,
) -> Result<Box<dyn Change>, String> {
    if fields.contains_key("contents") {
        return Ok(Box::new(crate::change::FileChanged::contents(
            scope,
            stored_bytes(&fields, "file-changed")?,
        )));
    }
    Ok(Box::new(crate::change::FileChanged::hashes(
        scope,
        required_hash(&fields, "old", "file-changed")?,
        required_hash(&fields, "new", "file-changed")?,
    )))
}

/// A SHA-256 hex digest recorded for a changed file.
fn required_hash(
    fields: &BTreeMap<String, String>,
    key: &str,
    kind: &str,
) -> Result<String, String> {
    let text = required(fields, key, kind)?;
    let hex = text.to_ascii_lowercase();
    if hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(hex)
    } else {
        Err(format!("{kind} has invalid {key}"))
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

/// Checks that a removal's `lines` is the negative count of lines in `text`.
///
/// `lines: -1` removes one line. `lines: -2` removes two. The text holds
/// that many lines.
fn expect_removed_lines(
    fields: &BTreeMap<String, String>,
    old: usize,
    text: &str,
) -> Result<(), String> {
    let lines = required_lines(fields, "line-removed")?;
    if lines >= 0 {
        return Err(invalid_lines("line-removed", fields));
    }
    let count = lines
        .checked_neg()
        .ok_or_else(|| invalid_lines("line-removed", fields))? as usize;
    if text.split('\n').count() != count {
        return Err(invalid_lines("line-removed", fields));
    }
    old.checked_add(count - 1)
        .ok_or_else(|| invalid_lines("line-removed", fields))?;
    Ok(())
}

/// Checks that an addition's `lines` is the positive count of lines in `text`.
fn expect_added_lines(fields: &BTreeMap<String, String>, text: &str) -> Result<(), String> {
    let lines = required_lines(fields, "line-added")?;
    if lines <= 0 || text.split('\n').count() != lines as usize {
        return Err(invalid_lines("line-added", fields));
    }
    Ok(())
}

/// Checks that a modification's `lines` is zero.
///
/// The text holds the replacement lines. Several updated lines are still `+0`,
/// because a modification does not change how many lines the file has.
fn expect_modified_lines(fields: &BTreeMap<String, String>) -> Result<(), String> {
    let lines = required_lines(fields, "line-modified")?;
    if lines != 0 {
        return Err(invalid_lines("line-modified", fields));
    }
    Ok(())
}

fn required_lines(fields: &BTreeMap<String, String>, kind: &str) -> Result<isize, String> {
    let text = required(fields, "lines", kind)?;
    text.parse::<isize>()
        .map_err(|_| format!("{kind} has invalid lines {text}"))
}

fn invalid_lines(kind: &str, fields: &BTreeMap<String, String>) -> String {
    match fields.get("lines") {
        Some(text) => format!("{kind} has invalid lines {text}"),
        None => format!("{kind} is missing lines"),
    }
}

/// A 1-based line number recorded for a line change.
fn required_line(
    fields: &BTreeMap<String, String>,
    key: &str,
    kind: &str,
) -> Result<usize, String> {
    let text = required(fields, key, kind)?;
    match text.parse::<usize>() {
        Ok(line) if line >= 1 => Ok(line),
        _ => Err(format!("{kind} has invalid {key} {text}")),
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
