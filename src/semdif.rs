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
        &differences.renames,
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
/// The header is always present. Renames are included only when some were found, so an
/// unchanged pair does not gain an empty change list.
fn semdif_document(version: &str, timestamp: &str, renames: &[(String, String)]) -> String {
    let mut document = header_document(version, timestamp);
    if renames.is_empty() {
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
    document
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

/// One file rename recorded in a semdif file.
pub(crate) struct FileRenamed {
    pub from: String,
    pub to: String,
}

/// The file renames recorded in a semdif file.
///
/// Apply replays those records onto one directory. Any other change is rejected
/// so an unsupported edit is not skipped.
pub(crate) fn parse_renames(document: &str) -> Result<Vec<FileRenamed>, String> {
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

    fn finish(self) -> Result<FileRenamed, String> {
        match self.kind.as_str() {
            "file-renamed" => Ok(FileRenamed {
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
