use std::collections::BTreeSet;
use std::path::Path;
use std::process::ExitCode;

use crate::change::{Change, LinePlan};
use crate::filesystem::FileSystem;

/// Edits `source` so it gains the changes recorded in `semdif`.
///
/// The semdif file is the record of what changed, so the same edits can be replayed
/// on another copy of that directory. `ignore_hash` still applies an added,
/// deleted, or renamed file when its recorded hash does not match. A problem
/// is reported on stderr.
pub(crate) fn apply(
    fs: &dyn FileSystem,
    semdif: &str,
    source: &str,
    ignore_hash: bool,
) -> ExitCode {
    let document = match fs.read_text(Path::new(semdif)) {
        Ok(document) => document,
        Err(error) => {
            eprintln!("cannot read {semdif}: {error}");
            return ExitCode::from(1);
        }
    };
    match apply_document(fs, &document, Path::new(source), ignore_hash) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(1)
        }
    }
}

pub(crate) fn apply_document(
    fs: &dyn FileSystem,
    document: &str,
    source: &Path,
    ignore_hash: bool,
) -> Result<(), String> {
    let changes = crate::semdif::parse_changes(document)?;
    let mut applied_content = BTreeSet::new();
    for change in &changes {
        if let Some(content) = change.content() {
            let path = content.file();
            if !applied_content.insert(path.to_string()) {
                continue;
            }
            apply_file(fs, source, path, &changes)?;
        } else if change.directory_scope().is_some() {
            for path in fs.collect_files(source)? {
                if !change.edits_file(&path) {
                    continue;
                }
                if !applied_content.insert(path.clone()) {
                    continue;
                }
                apply_file(fs, source, &path, &changes)?;
            }
        } else {
            apply_change(fs, source, change.as_ref(), ignore_hash)?;
        }
    }
    Ok(())
}

/// Applies every edit of one file, including a directory rename that covers it.
fn apply_file(
    fs: &dyn FileSystem,
    source: &Path,
    path: &str,
    changes: &[Box<dyn Change>],
) -> Result<(), String> {
    let batch: Vec<&dyn Change> = changes
        .iter()
        .filter(|candidate| candidate.edits_file(path))
        .map(|candidate| candidate.as_ref())
        .collect();
    apply_content(fs, source, path, &batch)
}

/// Performs one structural change in the source directory.
///
/// A content edit is applied with the other edits of its file, in `apply_content`.
/// `ignore_hash` still applies an added, deleted, or renamed file when its
/// recorded hash does not match.
fn apply_change(
    fs: &dyn FileSystem,
    source: &Path,
    change: &dyn Change,
    ignore_hash: bool,
) -> Result<(), String> {
    if !ignore_hash {
        change.check_recorded_hash(fs, source)?;
    }
    change.apply(fs, source)
}

/// Checks one changed file, builds its new bytes, and writes them when the hashes match.
///
/// The old hash is checked against the file as it is. The new hash is checked
/// against the bytes about to be written, so a mismatch leaves the file unchanged.
fn apply_content(
    fs: &dyn FileSystem,
    root: &Path,
    relative: &str,
    batch: &[&dyn Change],
) -> Result<(), String> {
    let path = crate::filesystem::path_inside(root, relative)?;
    let bytes = fs
        .read(&path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let digest = file_digest(batch)?;
    if let Some((old_hash, _)) = &digest {
        check_hash(&path, &bytes, old_hash, "old")?;
    }
    let new_bytes = new_contents(&path, &bytes, batch)?;
    if let Some((_, new_hash)) = &digest {
        check_hash(&path, &new_bytes, new_hash, "new")?;
    }
    if new_bytes != bytes {
        fs.write(&path, &new_bytes)
            .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
    }
    Ok(())
}

fn file_digest(batch: &[&dyn Change]) -> Result<Option<(String, String)>, String> {
    let mut found = None;
    for change in batch {
        let hashes = change
            .digest()
            .map(|digest| (digest.old_hash().to_string(), digest.new_hash().to_string()));
        if let Some(hashes) = hashes {
            if found.is_some() {
                return Err(format!(
                    "{} has more than one file-changed hash",
                    change.scope()
                ));
            }
            found = Some(hashes);
        }
    }
    Ok(found)
}

fn check_hash(path: &Path, bytes: &[u8], expected: &str, side: &str) -> Result<(), String> {
    let actual = crate::file_change::sha256_hex(bytes);
    if actual != expected {
        return Err(format!(
            "cannot apply {}: contents do not match the {side} hash",
            path.display()
        ));
    }
    Ok(())
}

fn new_contents(path: &Path, bytes: &[u8], batch: &[&dyn Change]) -> Result<Vec<u8>, String> {
    let mut replacement = None;
    let mut lines = Vec::new();
    for change in batch {
        if let Some(contents) = change.replacement() {
            if replacement.is_some() {
                return Err(format!("{} has more than one replacement", path.display()));
            }
            replacement = Some(contents.bytes().to_vec());
        } else if change.line_edit().is_some() {
            lines.push(*change);
        }
    }
    if let Some(contents) = replacement {
        // A directory rename joins every file it covers. A file whose record is
        // the whole new contents already has the renamed text, so the line edit
        // is not applied again.
        if lines.is_empty()
            || lines
                .iter()
                .all(|change| change.directory_scope().is_some())
        {
            return Ok(contents);
        }
        return Err(format!(
            "{} has both line edits and replacement contents",
            path.display()
        ));
    }
    if lines.is_empty() {
        return Ok(bytes.to_vec());
    }
    let text = match std::str::from_utf8(bytes) {
        Ok(text) => text.to_string(),
        Err(_)
            if lines
                .iter()
                .all(|change| change.directory_scope().is_some()) =>
        {
            return Ok(bytes.to_vec());
        }
        Err(_) => {
            return Err(format!(
                "cannot edit lines in {}: file is not text",
                path.display()
            ));
        }
    };
    let source_lines = crate::line_changes::split_text(&text);
    // The only place a LinePlan is built. Each line edit adds the lines it
    // changes, then `rebuild` produces the new lines of this file. A
    // whole-file replacement returned above and never reaches here.
    let mut plan = LinePlan::new();
    for change in &lines {
        let Some(line) = change.line_edit() else {
            continue;
        };
        line.rebuild(path, &source_lines.lines, &mut plan)?;
    }
    let rebuilt = plan.rebuild(path, &source_lines.lines)?;
    Ok(crate::line_changes::join_text(&crate::line_changes::Text {
        lines: rebuilt,
        trailing_newline: source_lines.trailing_newline,
    })
    .into_bytes())
}
