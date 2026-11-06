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

/// The hash named in a semdif header.
///
/// File digests are SHA-256, written as lowercase hex. The header records that
/// choice with the comparison.
const HASH_ALGORITHM: &str = "sha256";

/// The text encoding named in a semdif header.
///
/// Quoted text in the file is UTF-8. A reader uses this to turn those values
/// back into text.
const TEXT_ENCODING: &str = "utf-8";

/// The binary encoding named in a semdif header.
///
/// Bytes that are not text cannot sit in a YAML string, so a change stores them
/// with this encoding. The same name is written on that change.
const BINARY_ENCODING: &str = "base64";

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
            document.push_str(&format!("      encoding: \"{BINARY_ENCODING}\"\n"));
            document.push_str(&format!(
                "      contents: {}\n",
                yaml_string(&STANDARD.encode(contents))
            ));
        }
    }
}

/// The header that identifies a semdif file.
///
/// It records which tool version produced the file, the UTC time it was produced,
/// the hash used for file contents, the encoding of text, and the encoding of
/// bytes that are not text.
fn header_document(version: &str, timestamp: &str) -> String {
    format!(
        "\
header:
  semdiff: \"{version}\"
  timestamp: \"{timestamp}\"
  hash: \"{HASH_ALGORITHM}\"
  text-encoding: \"{TEXT_ENCODING}\"
  binary-encoding: \"{BINARY_ENCODING}\"
"
    )
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
            ("named-block-added", parse_named_block_added),
            ("named-block-removed", parse_named_block_removed),
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
    expect_added_lines(&fields, &text, "line-added")?;
    Ok(Box::new(crate::change::LineAdded::new(
        scope,
        required_line(&fields, "old", "line-added")?,
        required_line(&fields, "new", "line-added")?,
        text,
        comment_flag(&fields, "line-added")?,
        whitespace_flag(&fields, "line-added")?,
    )))
}

fn parse_named_block_added(
    scope: String,
    fields: BTreeMap<String, String>,
) -> Result<Box<dyn Change>, String> {
    let text = required(&fields, "text", "named-block-added")?;
    expect_added_lines(&fields, &text, "named-block-added")?;
    Ok(Box::new(crate::change::NamedBlockAdded::new(
        scope,
        required_line(&fields, "old", "named-block-added")?,
        required_line(&fields, "new", "named-block-added")?,
        text,
        required_category(&fields, "named-block-added")?,
        required_name(&fields, "named-block-added")?,
    )))
}

fn parse_named_block_removed(
    scope: String,
    fields: BTreeMap<String, String>,
) -> Result<Box<dyn Change>, String> {
    let old = required_line(&fields, "old", "named-block-removed")?;
    let text = required(&fields, "text", "named-block-removed")?;
    expect_removed_lines(&fields, old, &text, "named-block-removed")?;
    Ok(Box::new(crate::change::NamedBlockRemoved::new(
        scope,
        old,
        required_line(&fields, "new", "named-block-removed")?,
        text,
        required_category(&fields, "named-block-removed")?,
        required_name(&fields, "named-block-removed")?,
    )))
}

/// The shared category recorded on a named-block addition.
///
/// The value has to be one of the categories every dialect shares. A missing
/// or unknown category is rejected.
fn required_category(
    fields: &BTreeMap<String, String>,
    kind: &str,
) -> Result<crate::dialect::BlockCategory, String> {
    let text = required(fields, "category", kind)?;
    crate::dialect::BlockCategory::parse(&text)
        .ok_or_else(|| format!("{kind} has invalid category {text}"))
}

fn parse_line_removed(
    scope: String,
    fields: BTreeMap<String, String>,
) -> Result<Box<dyn Change>, String> {
    let old = required_line(&fields, "old", "line-removed")?;
    let text = required(&fields, "text", "line-removed")?;
    expect_removed_lines(&fields, old, &text, "line-removed")?;
    Ok(Box::new(crate::change::LineRemoved::new(
        scope,
        old,
        required_line(&fields, "new", "line-removed")?,
        text,
        comment_flag(&fields, "line-removed")?,
        whitespace_flag(&fields, "line-removed")?,
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
        comment_flag(&fields, "line-modified")?,
        whitespace_flag(&fields, "line-modified")?,
    )))
}

fn parse_identifier_renamed(
    scope: String,
    fields: BTreeMap<String, String>,
) -> Result<Box<dyn Change>, String> {
    let oldname = required(&fields, "oldname", "identifier-renamed")?;
    let newname = required(&fields, "newname", "identifier-renamed")?;
    match crate::change::parse_rename_scope(&scope)? {
        crate::change::RenameScope::Lines(span) => {
            let new = if span.is_single() {
                Some(required_line(&fields, "new", "identifier-renamed")?)
            } else {
                None
            };
            Ok(Box::new(crate::change::IdentifierRenamed::recorded(
                span, new, oldname, newname,
            )))
        }
        crate::change::RenameScope::Directory(scope) => Ok(Box::new(
            crate::change::IdentifierRenamed::directory(scope, oldname, newname),
        )),
    }
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
        Some(BINARY_ENCODING) => STANDARD
            .decode(contents)
            .map_err(|_| format!("{kind} contents are not valid {BINARY_ENCODING}")),
        Some(other) => Err(format!("{kind} has unsupported encoding {other}")),
    }
}

/// Checks that a removal's `lines` is the negative count of lines in `text`.
///
/// `lines: -1` removes one line. `lines: -2` removes two. The text holds
/// that many lines. `kind` is `line-removed` or `named-block-removed`.
fn expect_removed_lines(
    fields: &BTreeMap<String, String>,
    old: usize,
    text: &str,
    kind: &str,
) -> Result<(), String> {
    let lines = required_lines(fields, kind)?;
    if lines >= 0 {
        return Err(invalid_lines(kind, fields));
    }
    let count = lines
        .checked_neg()
        .ok_or_else(|| invalid_lines(kind, fields))? as usize;
    if text.split('\n').count() != count {
        return Err(invalid_lines(kind, fields));
    }
    old.checked_add(count - 1)
        .ok_or_else(|| invalid_lines(kind, fields))?;
    Ok(())
}

/// Checks that an addition's `lines` is the positive count of lines in `text`.
fn expect_added_lines(
    fields: &BTreeMap<String, String>,
    text: &str,
    kind: &str,
) -> Result<(), String> {
    let lines = required_lines(fields, kind)?;
    if lines <= 0 || text.split('\n').count() != lines as usize {
        return Err(invalid_lines(kind, fields));
    }
    Ok(())
}

/// The block name recorded on a named-block addition.
///
/// The name is the identifier that follows the dialect's keyword. An empty
/// name is rejected so apply is not asked to replay a block that has none.
fn required_name(fields: &BTreeMap<String, String>, kind: &str) -> Result<String, String> {
    let name = required(fields, "name", kind)?;
    if name.is_empty() {
        return Err(format!("{kind} has invalid name"));
    }
    Ok(name)
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

/// The `isComment` detail on a line change.
///
/// A full-line comment sets `true`. The attribute is left out when the change
/// is not a comment, so a missing value is false. An explicit `false` is the
/// same. Any other value is rejected.
fn comment_flag(fields: &BTreeMap<String, String>, kind: &str) -> Result<bool, String> {
    recorded_flag(fields, kind, "isComment")
}

/// The `isWhitespace` detail on a line change.
///
/// Insignificant whitespace sets `true`. The attribute is left out when the
/// change is not whitespace, so a missing value is false. An explicit `false`
/// is the same. Any other value is rejected. This is separate from
/// `isComment`, and a line may carry both.
fn whitespace_flag(fields: &BTreeMap<String, String>, kind: &str) -> Result<bool, String> {
    recorded_flag(fields, kind, "isWhitespace")
}

/// A boolean detail that defaults to false.
///
/// Missing and `false` are false. `true` is true. Any other value is rejected.
fn recorded_flag(
    fields: &BTreeMap<String, String>,
    kind: &str,
    name: &str,
) -> Result<bool, String> {
    match fields.get(name).map(String::as_str) {
        None | Some("false") => Ok(false),
        Some("true") => Ok(true),
        Some(other) => Err(format!("{kind} has invalid {name} {other}")),
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

#[cfg(test)]
mod tests {
    use super::{header_document, parse_changes};

    #[test]
    fn the_header_names_the_hash_and_both_encodings() {
        let header = header_document("0.1.0", "2020-01-01T00:00:00Z");
        assert_eq!(
            header,
            "\
header:
  semdiff: \"0.1.0\"
  timestamp: \"2020-01-01T00:00:00Z\"
  hash: \"sha256\"
  text-encoding: \"utf-8\"
  binary-encoding: \"base64\"
"
        );
    }

    fn line_change(kind: &str, lines: &str, comment: &str, whitespace: &str) -> String {
        let mut flags = String::new();
        if !comment.is_empty() {
            flags.push_str(&format!("      isComment: {comment}\n"));
        }
        if !whitespace.is_empty() {
            flags.push_str(&format!("      isWhitespace: {whitespace}\n"));
        }
        format!(
            "\
changes:
  - scope: \"src/main.rs\"
    type: {kind}
    details:
      old: 1
      lines: {lines}
      new: 1
      text: \"// note\"
{flags}"
        )
    }

    fn assert_flags(document: &str, is_comment: bool, is_whitespace: bool) {
        let changes = parse_changes(document).expect("line change");
        assert_eq!(changes.len(), 1);
        let line = changes[0].line_edit().expect("line edit");
        assert_eq!(line.is_comment(), is_comment);
        assert_eq!(line.is_whitespace(), is_whitespace);
        let mut details = String::new();
        changes[0].write_details(&mut details);
        if is_comment {
            assert!(details.contains("isComment: true\n"), "{details}");
        } else {
            assert!(!details.contains("isComment"), "{details}");
        }
        if is_whitespace {
            assert!(details.contains("isWhitespace: true\n"), "{details}");
        } else {
            assert!(!details.contains("isWhitespace"), "{details}");
        }
    }

    #[test]
    fn a_comment_flag_round_trips_and_defaults_to_false() {
        for (kind, lines) in [
            ("line-added", "+1"),
            ("line-removed", "-1"),
            ("line-modified", "+0"),
        ] {
            assert_flags(&line_change(kind, lines, "true", ""), true, false);
            assert_flags(&line_change(kind, lines, "", ""), false, false);
            assert_flags(&line_change(kind, lines, "false", ""), false, false);
        }
    }

    #[test]
    fn rejects_an_invalid_comment_flag() {
        let message = match parse_changes(&line_change("line-added", "+1", "yes", "")) {
            Err(message) => message,
            Ok(_) => panic!("invalid isComment was accepted"),
        };
        assert!(message.contains("invalid isComment yes"), "{message}");
    }

    #[test]
    fn a_whitespace_flag_round_trips_and_defaults_to_false() {
        for (kind, lines) in [
            ("line-added", "+1"),
            ("line-removed", "-1"),
            ("line-modified", "+0"),
        ] {
            assert_flags(&line_change(kind, lines, "", "true"), false, true);
            assert_flags(&line_change(kind, lines, "", ""), false, false);
            assert_flags(&line_change(kind, lines, "", "false"), false, false);
        }
    }

    #[test]
    fn rejects_an_invalid_whitespace_flag() {
        let message = match parse_changes(&line_change("line-added", "+1", "", "yes")) {
            Err(message) => message,
            Ok(_) => panic!("invalid isWhitespace was accepted"),
        };
        assert!(message.contains("invalid isWhitespace yes"), "{message}");
    }

    #[test]
    fn a_line_can_be_a_comment_and_whitespace() {
        assert_flags(
            &line_change("line-modified", "+0", "true", "true"),
            true,
            true,
        );
    }

    #[test]
    fn a_named_block_records_its_category_and_name() {
        let document = "\
changes:
  - scope: \"src/main.rs\"
    type: named-block-added
    details:
      old: 1
      lines: +1
      new: 1
      text: \"fn fib() {}\"
      category: \"function\"
      name: \"fib\"
";
        let changes = parse_changes(document).expect("named block");
        let mut details = String::new();
        changes[0].write_details(&mut details);
        assert!(details.contains("category: \"function\"\n"), "{details}");
        assert!(details.contains("name: \"fib\"\n"), "{details}");

        let missing_category = "\
changes:
  - scope: \"src/main.rs\"
    type: named-block-added
    details:
      old: 1
      lines: +1
      new: 1
      text: \"fn fib() {}\"
      name: \"fib\"
";
        let message = match parse_changes(missing_category) {
            Err(message) => message,
            Ok(_) => panic!("missing category was accepted"),
        };
        assert!(message.contains("missing category"), "{message}");

        let unknown = "\
changes:
  - scope: \"src/main.rs\"
    type: named-block-added
    details:
      old: 1
      lines: +1
      new: 1
      text: \"fn fib() {}\"
      category: \"widget\"
      name: \"fib\"
";
        let message = match parse_changes(unknown) {
            Err(message) => message,
            Ok(_) => panic!("unknown category was accepted"),
        };
        assert!(message.contains("invalid category widget"), "{message}");

        let missing_name = "\
changes:
  - scope: \"src/main.rs\"
    type: named-block-added
    details:
      old: 1
      lines: +1
      new: 1
      text: \"fn fib() {}\"
      category: \"function\"
";
        let message = match parse_changes(missing_name) {
            Err(message) => message,
            Ok(_) => panic!("missing name was accepted"),
        };
        assert!(message.contains("missing name"), "{message}");
    }

    #[test]
    fn a_named_block_removal_records_its_category_and_name() {
        let document = "\
changes:
  - scope: \"src/main.rs[Count]\"
    type: named-block-removed
    details:
      old: 2
      lines: -1
      new: 2
      text: \"    fn step(&self) {}\"
      category: \"method\"
      name: \"step\"
";
        let changes = parse_changes(document).expect("named block");
        assert_eq!(changes[0].kind(), "named-block-removed");
        assert_eq!(changes[0].scope(), "src/main.rs[Count]");
        assert_eq!(changes[0].content().expect("content").file(), "src/main.rs");
        let mut details = String::new();
        changes[0].write_details(&mut details);
        assert!(details.contains("lines: -1\n"), "{details}");
        assert!(details.contains("category: \"method\"\n"), "{details}");
        assert!(details.contains("name: \"step\"\n"), "{details}");

        let positive = "\
changes:
  - scope: \"src/main.rs\"
    type: named-block-removed
    details:
      old: 1
      lines: +1
      new: 1
      text: \"fn fib() {}\"
      category: \"function\"
      name: \"fib\"
";
        let message = match parse_changes(positive) {
            Err(message) => message,
            Ok(_) => panic!("a positive line count was accepted"),
        };
        assert!(
            message.contains("named-block-removed has invalid lines +1"),
            "{message}"
        );
    }
}
