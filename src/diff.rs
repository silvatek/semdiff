//! Comparing two trees, including the order of the stages and the passes that
//! combine records.
//!
//! One pass collects the files and directories, splits the paths, and records
//! file, content, and directory changes. Later passes merge adjacent lines,
//! name a complete block, and join identifier renames that cover one directory.
//!
//! A file that exists on only one side, a paired file, an empty directory, and
//! the lines of a text file each live in their own module. A paired file whose
//! bytes differ is compared in `file_change`, which asks `line_changes` when
//! both copies are text. A changed line is split into spans, and one classified
//! span becomes its own record.

mod directory_changes;
mod file_change;
mod line_changes;
pub(crate) mod span_changes;

pub(crate) use file_change::sha256_hex;
pub(crate) use line_changes::{join_text, split_text, Text};

use std::collections::BTreeSet;
use std::path::Path;

use crate::change::Change;
use crate::filesystem::FileSystem;

/// The shared paths and the change records for one comparison.
///
/// `paths` are the files present on both sides. `changes` are the records
/// written into the semdif.
pub(crate) struct Differences {
    pub(crate) paths: Vec<String>,
    pub(crate) changes: Vec<Box<dyn Change>>,
}

/// The shared files and file changes between the two sources.
///
/// The two file lists are split once, so a later step does not search the trees again.
/// Files that exist on only one side may be a rename, a move, a deletion, or an
/// addition. A move is one deleted file and one added file that share a filename
/// and keep more than half their lines in order.
/// Files that exist on both sides are paired and compared in place.
/// Empty directories that appear or disappear are recorded too, including one
/// left empty because its files moved. Adjacent removed, added, or modified
/// lines in one file become one record before the semdif file is written
/// when every line in the run is a comment, or none of them is. An added or
/// removed run that is one complete named block is then recorded as that
/// block.
/// Identifier renames of one name then join across files when the directory
/// that covers them has no other occurrence of that name.
pub(crate) fn find_differences(
    fs: &dyn FileSystem,
    sources: &crate::Sources,
) -> Result<Differences, String> {
    let left_files = fs.collect_files(Path::new(&sources.left))?;
    let right_files = fs.collect_files(Path::new(&sources.right))?;
    let left_dirs = fs.collect_directories(Path::new(&sources.left))?;
    let right_dirs = fs.collect_directories(Path::new(&sources.right))?;
    let (left_only, right_only, paired_files) = partition_files(&left_files, &right_files);
    let mut changes = file_change::file_changes(fs, sources, &left_only, &right_only)?;
    changes.extend(file_change::paired_changes(fs, sources, &paired_files)?);
    changes.extend(directory_changes::directory_changes(
        &left_dirs,
        &right_dirs,
        &right_files,
    ));
    let changes = directory_changes::place_replacement_files(changes);
    let changes = crate::change::merge_adjacent(changes);
    let right_root = fs.source_base(Path::new(&sources.right));
    let right_texts = source_texts(fs, right_root, &paired_files)?;
    let left_root = fs.source_base(Path::new(&sources.left));
    let texts = source_texts(fs, left_root, &left_files)?;
    let changes = crate::change::promote_named_blocks(&right_texts, &texts, changes);
    let changes = crate::change::merge_identifier_files(&texts, changes);
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
) -> Result<Vec<crate::change::SourceText>, String> {
    let mut texts = Vec::new();
    for path in files {
        let bytes = fs
            .read(&root.join(path))
            .map_err(|error| format!("cannot read {}: {error}", root.join(path).display()))?;
        let lines = match std::str::from_utf8(&bytes) {
            Ok(text) => line_changes::split_text(text).lines,
            Err(_) => Vec::new(),
        };
        texts.push(crate::change::SourceText {
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
