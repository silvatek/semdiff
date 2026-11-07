//! Semantic diff of two source trees.

mod apply;
mod args;
mod change;
mod classify;
mod constant;
mod content_changes;
mod dialect;
mod directory_changes;
mod file_change;
mod filesystem;
mod help;
mod identifier;
mod line_changes;
mod semdif;
mod span;

pub use filesystem::{FileSystem, MemoryFileSystem, NodeKind, StdFileSystem};

pub(crate) use args::Sources;

use std::collections::BTreeSet;
use std::path::Path;
use std::process::ExitCode;

/// Compares two sources, or applies a semdif file to one directory.
///
/// Argument errors stop the command before a comparison or edit starts.
pub fn run() -> ExitCode {
    match args::parse_args() {
        args::Command::Help => {
            help::print_help();
            ExitCode::SUCCESS
        }
        args::Command::Compare(args) => {
            let fs = match filesystem_for(args.ignore.as_deref()) {
                Ok(fs) => fs,
                Err(message) => {
                    eprintln!("{message}");
                    return ExitCode::from(1);
                }
            };
            match find_differences(&fs, &args.sources) {
                Ok(differences) => {
                    semdif::write_output(&fs, &args.output, &args.sources, &differences)
                }
                Err(message) => {
                    eprintln!("{message}");
                    ExitCode::from(1)
                }
            }
        }
        args::Command::Apply {
            semdif,
            source,
            ignore_hash,
            ignore,
        } => {
            let fs = match filesystem_for(ignore.as_deref()) {
                Ok(fs) => fs,
                Err(message) => {
                    eprintln!("{message}");
                    return ExitCode::from(1);
                }
            };
            apply::apply(&fs, &semdif, &source, ignore_hash)
        }
    }
}

/// The filesystem for this command.
///
/// Without an ignore file, nothing is left out. The file's paths are the
/// filesystem's ignore list.
fn filesystem_for(ignore: Option<&str>) -> Result<StdFileSystem, String> {
    let Some(path) = ignore else {
        return Ok(StdFileSystem::new());
    };
    let text =
        std::fs::read_to_string(path).map_err(|error| format!("cannot read {path}: {error}"))?;
    Ok(StdFileSystem::with_ignores(filesystem::ignore_entries(
        &text,
    )))
}

/// The semdif text for `left` compared with `right`.
pub fn compare_sources(fs: &dyn FileSystem, left: &str, right: &str) -> Result<String, String> {
    let sources = Sources {
        left: left.to_string(),
        right: right.to_string(),
    };
    let differences = find_differences(fs, &sources)?;
    Ok(semdif::document(&differences.changes))
}

/// Replays a semdif document onto `source`.
pub fn apply_document(fs: &dyn FileSystem, document: &str, source: &str) -> Result<(), String> {
    apply::apply_document(fs, document, Path::new(source), false)
}

struct Differences {
    paths: Vec<String>,
    changes: Vec<Box<dyn change::Change>>,
}

/// The shared files and file changes between the two sources.
///
/// The two file lists are split once, so a later step does not search the trees again.
/// Files that exist on only one side may be a rename, a deletion, or an addition.
/// Files that exist on both sides are paired and compared in place.
/// Empty directories that appear or disappear are recorded too, including one
/// left empty because its files moved. Adjacent removed, added, or modified
/// lines in one file become one record before the semdif file is written
/// when every line in the run is a comment, or none of them is. An added or
/// removed run that is one complete named block is then recorded as that
/// block.
/// Identifier renames of one name then join across files when the directory
/// that covers them has no other occurrence of that name.
fn find_differences(fs: &dyn FileSystem, sources: &Sources) -> Result<Differences, String> {
    let left_files = fs.collect_files(Path::new(&sources.left))?;
    let right_files = fs.collect_files(Path::new(&sources.right))?;
    let left_dirs = fs.collect_directories(Path::new(&sources.left))?;
    let right_dirs = fs.collect_directories(Path::new(&sources.right))?;
    let (left_only, right_only, paired_files) = partition_files(&left_files, &right_files);
    let mut changes = file_change::file_changes(fs, sources, &left_only, &right_only)?;
    changes.extend(content_changes::content_changes(
        fs,
        sources,
        &paired_files,
    )?);
    changes.extend(directory_changes::directory_changes(
        &left_dirs,
        &right_dirs,
        &right_files,
    ));
    let changes = directory_changes::place_replacement_files(changes);
    let changes = change::merge_adjacent(changes);
    let right_root = fs.source_base(Path::new(&sources.right));
    let right_texts = source_texts(fs, right_root, &paired_files)?;
    let left_root = fs.source_base(Path::new(&sources.left));
    let texts = source_texts(fs, left_root, &left_files)?;
    let changes = change::promote_named_blocks(&right_texts, &texts, changes);
    let changes = change::merge_identifier_files(&texts, changes);
    Ok(Differences {
        paths: paired_files,
        changes,
    })
}

/// The old text of each file, so a rename can see every occurrence of a name.
///
/// A file that is not text has no lines. It holds no identifiers.
fn source_texts(
    fs: &dyn FileSystem,
    root: &Path,
    files: &[String],
) -> Result<Vec<change::SourceText>, String> {
    let mut texts = Vec::new();
    for path in files {
        let bytes = fs
            .read(&root.join(path))
            .map_err(|error| format!("cannot read {}: {error}", root.join(path).display()))?;
        let lines = match std::str::from_utf8(&bytes) {
            Ok(text) => line_changes::split_text(text).lines,
            Err(_) => Vec::new(),
        };
        texts.push(change::SourceText {
            path: path.clone(),
            lines,
        });
    }
    Ok(texts)
}

/// Files only on the left, only on the right, and present under the same path on both sides.
///
/// A paired path is the same file in both sources. The other two lists are the candidates
/// for a rename.
fn partition_files(
    left_files: &[String],
    right_files: &[String],
) -> (Vec<String>, Vec<String>, Vec<String>) {
    let left_names: BTreeSet<_> = left_files.iter().cloned().collect();
    let right_names: BTreeSet<_> = right_files.iter().cloned().collect();
    let mut left_only: Vec<_> = left_files
        .iter()
        .filter(|path| !right_names.contains(*path))
        .cloned()
        .collect();
    let mut right_only: Vec<_> = right_files
        .iter()
        .filter(|path| !left_names.contains(*path))
        .cloned()
        .collect();
    let mut paired_files: Vec<_> = left_names.intersection(&right_names).cloned().collect();
    left_only.sort();
    right_only.sort();
    paired_files.sort();
    (left_only, right_only, paired_files)
}
