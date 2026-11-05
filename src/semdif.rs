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
            write_content(document, path, "file-changed");
            write_file_contents(document, contents);
        }
        Change::FileDigest {
            path,
            old_hash,
            new_hash,
        } => {
            write_content(document, path, "file-changed");
            document.push_str(&format!("      old: {}\n", yaml_string(old_hash)));
            document.push_str(&format!("      new: {}\n", yaml_string(new_hash)));
        }
        Change::LineAdded {
            path,
            old,
            new,
            text,
        } => write_line(
            document,
            "line-added",
            path,
            *old,
            added_lines(text),
            *new,
            text,
        ),
        Change::LineRemoved {
            path,
            old,
            old_end,
            new,
            text,
        } => write_line(
            document,
            "line-removed",
            path,
            *old,
            removed_lines(*old, *old_end),
            *new,
            text,
        ),
        Change::LineModified {
            path,
            old,
            new,
            text,
        } => write_line(document, "line-modified", path, *old, 0, *new, text),
        Change::IdentifierRenamed {
            path,
            old,
            new,
            oldname,
            newname,
        } => write_identifier(document, path, *old, *new, oldname, newname),
        Change::DirectoryAdded { path } => write_directory(document, "directory-added", path),
        Change::DirectoryDeleted { path } => write_directory(document, "directory-deleted", path),
    }
}

fn write_identifier(
    document: &mut String,
    path: &str,
    old: usize,
    new: usize,
    oldname: &str,
    newname: &str,
) {
    write_content(document, path, "identifier-renamed");
    document.push_str(&format!("      old: {old}\n"));
    document.push_str(&format!("      new: {new}\n"));
    document.push_str(&format!("      oldname: {}\n", yaml_string(oldname)));
    document.push_str(&format!("      newname: {}\n", yaml_string(newname)));
}

fn write_line(
    document: &mut String,
    kind: &str,
    path: &str,
    old: usize,
    lines: isize,
    new: usize,
    text: &str,
) {
    write_content(document, path, kind);
    document.push_str(&format!("      old: {old}\n"));
    document.push_str(&format!("      lines: {}\n", format_lines(lines)));
    document.push_str(&format!("      new: {new}\n"));
    document.push_str(&format!("      text: {}\n", yaml_string(text)));
}

/// The `lines` value, with its sign written out.
///
/// A removal is negative, so one removed line is `-1` and two are `-2`.
/// An addition is positive, so one added line is `+1` and two are `+2`.
/// A modification does not change the line count, so it is `+0`.
fn format_lines(lines: isize) -> String {
    if lines > 0 {
        format!("+{lines}")
    } else if lines == 0 {
        "+0".to_string()
    } else {
        lines.to_string()
    }
}

/// How many old lines a removal covers, as a negative count.
///
/// One line is `-1`. A run of two is `-2`.
/// How many lines an addition record contains, as a positive count.
fn added_lines(text: &str) -> isize {
    text.split('\n').count() as isize
}

fn removed_lines(old: usize, old_end: usize) -> isize {
    let count = old_end - old + 1;
    -(count as isize)
}

/// The start of a change to a file's contents.
///
/// The scope is that file. The path is not repeated inside the details.
fn write_content(document: &mut String, path: &str, kind: &str) {
    document.push_str(&format!("  - scope: {}\n", yaml_string(path)));
    document.push_str(&format!("    type: {kind}\n"));
    document.push_str("    details:\n");
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
/// File and directory changes are replayed in the order they were recorded.
/// Line changes for one path are applied together from their old line numbers,
/// so those records can be listed in line order. Each kind carries the details
/// that kind needs.
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
    FileDigest {
        path: String,
        old_hash: String,
        new_hash: String,
    },
    LineAdded {
        path: String,
        old: usize,
        new: usize,
        text: String,
    },
    LineRemoved {
        path: String,
        /// First removed line in the old file.
        old: usize,
        /// Last removed line in the old file. Equal to `old` when `lines` is `-1`.
        old_end: usize,
        new: usize,
        text: String,
    },
    LineModified {
        path: String,
        old: usize,
        new: usize,
        text: String,
    },
    /// One identifier on a line was renamed.
    ///
    /// The rest of the line is unchanged, and `oldname` occurs once on the old
    /// line. The record does not carry a column.
    IdentifierRenamed {
        path: String,
        old: usize,
        new: usize,
        oldname: String,
        newname: String,
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
            "file-changed" => file_changed(self.scope, self.fields),
            "line-added" => {
                let text = required(&self.fields, "text", "line-added")?;
                expect_added_lines(&self.fields, &text)?;
                Ok(Change::LineAdded {
                    path: self.scope,
                    old: required_line(&self.fields, "old", "line-added")?,
                    new: required_line(&self.fields, "new", "line-added")?,
                    text,
                })
            }
            "line-removed" => {
                let old = required_line(&self.fields, "old", "line-removed")?;
                let text = required(&self.fields, "text", "line-removed")?;
                Ok(Change::LineRemoved {
                    path: self.scope,
                    old,
                    old_end: removed_end(&self.fields, old, &text)?,
                    new: required_line(&self.fields, "new", "line-removed")?,
                    text,
                })
            }
            "line-modified" => {
                let text = required(&self.fields, "text", "line-modified")?;
                expect_modified_lines(&self.fields)?;
                Ok(Change::LineModified {
                    path: self.scope,
                    old: required_line(&self.fields, "old", "line-modified")?,
                    new: required_line(&self.fields, "new", "line-modified")?,
                    text,
                })
            }
            "identifier-renamed" => Ok(Change::IdentifierRenamed {
                path: self.scope,
                old: required_line(&self.fields, "old", "identifier-renamed")?,
                new: required_line(&self.fields, "new", "identifier-renamed")?,
                oldname: required(&self.fields, "oldname", "identifier-renamed")?,
                newname: required(&self.fields, "newname", "identifier-renamed")?,
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

/// A whole-file replacement, or the old and new SHA-256 hashes of a changed file.
///
/// The file path is the record's scope. The hash record has `old` and `new`.
/// A replacement has `contents` instead.
fn file_changed(path: String, fields: BTreeMap<String, String>) -> Result<Change, String> {
    if fields.contains_key("contents") {
        return Ok(Change::FileChanged {
            path,
            contents: stored_bytes(&fields, "file-changed")?,
        });
    }
    Ok(Change::FileDigest {
        path,
        old_hash: required_hash(&fields, "old", "file-changed")?,
        new_hash: required_hash(&fields, "new", "file-changed")?,
    })
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

/// The last old line of a removal.
///
/// `lines: -1` removes the line at `old`. `lines: -2` removes `old` and the
/// next line. The text holds that many lines.
fn removed_end(fields: &BTreeMap<String, String>, old: usize, text: &str) -> Result<usize, String> {
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
        .ok_or_else(|| invalid_lines("line-removed", fields))
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
