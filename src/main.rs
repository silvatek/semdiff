mod apply;
mod args;
mod comment;
mod dialect;
mod rename;
mod semdif;
mod symbol;

pub(crate) use args::Sources;

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

/// Compares two sources, or applies a semdif file to one source.
///
/// Argument errors stop the command before any comparison or edit starts.
fn main() -> ExitCode {
    match args::parse_args() {
        args::Command::Compare(args) => {
            let differences = find_differences(&args.sources);
            semdif::write_output(&args.output, &args.sources, &differences)
        }
        args::Command::Apply { source, patch } => apply::apply(&source, &patch),
    }
}

struct Differences {
    paths: Vec<String>,
    renames: Vec<(String, String)>,
    symbol_renames: Vec<symbol::SymbolRename>,
    added_comments: Vec<comment::AddedLineComment>,
}

/// The shared files, renames, and content changes between the two sources.
///
/// File renames are found first. Each paired file is then compared once, and each
/// change in its content is classified.
fn find_differences(sources: &Sources) -> Differences {
    let paths = paired_files(sources).unwrap_or_else(|message| {
        exit_with_message(message);
    });
    let renames = rename::file_renames(sources).unwrap_or_else(|message| {
        exit_with_message(message);
    });
    let (symbol_renames, added_comments) =
        changed_content(sources, &paths).unwrap_or_else(|message| {
            exit_with_message(message);
        });
    Differences {
        paths,
        renames,
        symbol_renames,
        added_comments,
    }
}

/// Symbol renames and added line comments in the paired files.
///
/// A file whose text changed is classified from that one comparison. An unchanged
/// file is kept only so a symbol rename can see whether any use of the old name remains.
fn changed_content(
    sources: &Sources,
    paths: &[String],
) -> Result<(Vec<symbol::SymbolRename>, Vec<comment::AddedLineComment>), String> {
    let mut added_comments = Vec::new();
    let mut compared = Vec::new();
    for path in paths {
        let Some(dialect) = crate::dialect::for_path(path) else {
            continue;
        };
        let left = read_text(&sources.left, path)?;
        let right = read_text(&sources.right, path)?;
        if left != right {
            added_comments.extend(comment::added_line_comments(path, dialect, &left, &right));
        }
        if let Some(classify) = dialect.spans {
            compared.push(symbol::compared_file(path, left, right, classify));
        }
    }
    Ok((symbol::symbol_renames(&compared), added_comments))
}

/// The text of one paired file.
///
/// The comparison classifies this text, so the file is not read again for each change type.
fn read_text(source: &str, relative: &str) -> Result<String, String> {
    let path = source_base(Path::new(source)).join(relative);
    fs::read_to_string(&path).map_err(|error| format!("cannot read {}: {error}", path.display()))
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

/// The files that can be compared in place because they exist in both sources.
///
/// The paths are in a stable order so repeated runs list the same files.
fn paired_files(sources: &Sources) -> Result<Vec<String>, String> {
    let right_files: BTreeSet<_> = collect_files(Path::new(&sources.right))?
        .into_iter()
        .collect();
    let mut shared: Vec<_> = collect_files(Path::new(&sources.left))?
        .into_iter()
        .filter(|path| right_files.contains(path))
        .collect();
    shared.sort();
    shared.dedup();
    Ok(shared)
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
