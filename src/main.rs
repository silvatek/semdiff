mod apply;
mod args;
mod content_changes;
mod directory_changes;
mod file_change;
mod line_changes;
mod semdif;

pub(crate) use args::Sources;

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

/// Compares two sources, or applies a semdif file to one directory.
///
/// Argument errors stop the command before a comparison or edit starts.
fn main() -> ExitCode {
    match args::parse_args() {
        args::Command::Compare(args) => {
            let differences = find_differences(&args.sources);
            semdif::write_output(&args.output, &args.sources, &differences)
        }
        args::Command::Apply { semdif, source } => apply::apply(&semdif, &source),
    }
}

struct Differences {
    paths: Vec<String>,
    changes: Vec<semdif::Change>,
}

/// The shared files and file changes between the two sources.
///
/// The two file lists are split once, so a later step does not search the trees again.
/// Files that exist on only one side may be a rename, a deletion, or an addition.
/// Files that exist on both sides are paired and compared in place.
/// Empty directories that appear or disappear are recorded too, including one
/// left empty because its files moved.
fn find_differences(sources: &Sources) -> Differences {
    let left_files = collect_files(Path::new(&sources.left)).unwrap_or_else(|message| {
        exit_with_message(message);
    });
    let right_files = collect_files(Path::new(&sources.right)).unwrap_or_else(|message| {
        exit_with_message(message);
    });
    let left_dirs = collect_directories(Path::new(&sources.left)).unwrap_or_else(|message| {
        exit_with_message(message);
    });
    let right_dirs = collect_directories(Path::new(&sources.right)).unwrap_or_else(|message| {
        exit_with_message(message);
    });
    let (left_only, right_only, paired_files) = partition_files(&left_files, &right_files);
    let mut changes =
        file_change::file_changes(sources, &left_only, &right_only).unwrap_or_else(|message| {
            exit_with_message(message);
        });
    changes.extend(
        content_changes::content_changes(sources, &paired_files).unwrap_or_else(|message| {
            exit_with_message(message);
        }),
    );
    changes.extend(directory_changes::directory_changes(
        &left_dirs,
        &right_dirs,
        &right_files,
    ));
    let changes = directory_changes::place_replacement_files(changes);
    Differences {
        paths: paired_files,
        changes,
    }
}

/// Stops the command after a comparison failure.
///
/// The reason is shown on stderr and the process exits with code 1.
fn exit_with_message(message: String) -> ! {
    eprintln!("{message}");
    std::process::exit(1);
}

/// The directory from which a source's relative paths are read.
///
/// A single file and a directory tree can then be read in the same way.
pub(crate) fn source_base(path: &Path) -> &Path {
    if path.is_file() {
        path.parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or(Path::new("."))
    } else {
        path
    }
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

/// The directories that belong to a source.
///
/// Empty directories are part of the tree even though they contain no files.
/// Generated build output is omitted, as it is for files.
pub(crate) fn collect_directories(root: &Path) -> Result<Vec<String>, String> {
    if root.is_file() {
        return Ok(Vec::new());
    }
    let mut directories = Vec::new();
    visit_directories(root, root, &mut directories)?;
    Ok(directories)
}

fn visit_directories(root: &Path, dir: &Path, directories: &mut Vec<String>) -> Result<(), String> {
    let entries =
        fs::read_dir(dir).map_err(|error| format!("cannot read {}: {error}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("cannot read {}: {error}", dir.display()))?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if entry.file_name() == "target" {
            continue;
        }
        let relative = path
            .strip_prefix(root)
            .map_err(|_| format!("cannot relativize {}", path.display()))?;
        directories.push(relative.to_string_lossy().replace('\\', "/"));
        visit_directories(root, &path, directories)?;
    }
    Ok(())
}

/// The files that belong to a source.
///
/// A directory contributes its project files, and a file contributes itself. Generated
/// build output is omitted so it is not treated as project content.
pub(crate) fn collect_files(root: &Path) -> Result<Vec<String>, String> {
    if root.is_file() {
        let name = root
            .file_name()
            .ok_or_else(|| format!("cannot name {}", root.display()))?;
        return Ok(vec![name.to_string_lossy().into_owned()]);
    }
    let mut files = Vec::new();
    visit_dir(root, root, &mut files)?;
    Ok(files)
}

/// The files contained in a directory and its subdirectories.
///
/// Nested source files belong in the comparison, and generated build output does not.
fn visit_dir(root: &Path, dir: &Path, files: &mut Vec<String>) -> Result<(), String> {
    let entries =
        fs::read_dir(dir).map_err(|error| format!("cannot read {}: {error}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("cannot read {}: {error}", dir.display()))?;
        let path = entry.path();
        if path.is_dir() {
            if entry.file_name() == "target" {
                continue;
            }
            visit_dir(root, &path, files)?;
        } else if path.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(|_| format!("cannot relativize {}", path.display()))?;
            files.push(relative.to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(())
}
