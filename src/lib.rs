//! Semantic diff of two source trees.

mod apply;
mod args;
mod change;
mod content_changes;
mod dialect;
mod directory_changes;
mod file_change;
mod filesystem;
mod identifier;
mod line_changes;
mod semdif;

pub use filesystem::{FileSystem, MemoryFileSystem, NodeKind, StdFileSystem};

pub(crate) use args::Sources;

use std::collections::BTreeSet;
use std::path::Path;
use std::process::ExitCode;

/// Compares two sources, or applies a semdif file to one directory.
///
/// Argument errors stop the command before a comparison or edit starts.
pub fn run() -> ExitCode {
    let fs = StdFileSystem::new();
    match args::parse_args() {
        args::Command::Compare(args) => match find_differences(&fs, &args.sources) {
            Ok(differences) => semdif::write_output(&fs, &args.output, &args.sources, &differences),
            Err(message) => {
                eprintln!("{message}");
                ExitCode::from(1)
            }
        },
        args::Command::Apply { semdif, source } => apply::apply(&fs, &semdif, &source),
    }
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
    apply::apply_document(fs, document, Path::new(source))
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
/// lines in one file become one record before the semdif file is written.
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
    Ok(Differences {
        paths: paired_files,
        changes,
    })
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
