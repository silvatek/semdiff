mod output;
mod rename;

use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

/// Compares two directories and writes a semdiff file.
///
/// Argument errors stop the command before a comparison starts.
fn main() -> ExitCode {
    let args = parse_args();
    let differences = find_differences(&args.sources);
    output::write_output(&args.output, &args.sources, &differences)
}

struct Args {
    sources: Sources,
    output: String,
}

struct Sources {
    left: String,
    right: String,
}

struct Differences {
    paths: Vec<String>,
    renames: Vec<(String, String)>,
}

/// The directories to compare and where the semdiff file should be written.
///
/// An unusable invocation stops the command: code 2 for the wrong arguments, and code 1
/// when a source is not a directory.
fn parse_args() -> Args {
    match parsed_args(env::args().skip(1)) {
        Ok(args) => args,
        Err(CliError::Usage) => {
            eprintln!("Usage: semdiff <left> <right> <output>");
            std::process::exit(2);
        }
        Err(CliError::NotDirectory(path)) => {
            eprintln!("not a directory: {path}");
            std::process::exit(1);
        }
    }
}

/// The shared files and renames between the two sources.
///
/// Later steps use this result instead of searching the trees again.
fn find_differences(sources: &Sources) -> Differences {
    let paths = paired_files(sources).unwrap_or_else(|message| {
        exit_with_message(message);
    });
    let renames = rename::file_renames(sources).unwrap_or_else(|message| {
        exit_with_message(message);
    });
    Differences { paths, renames }
}

/// Stops the command after a comparison failure.
///
/// The reason is shown on stderr and the process exits with code 1.
fn exit_with_message(message: String) -> ! {
    eprintln!("{message}");
    std::process::exit(1);
}

enum CliError {
    Usage,
    NotDirectory(String),
}

/// The command arguments taken from the three expected paths.
///
/// The wrong number of paths is a usage error. A source that is not a directory is
/// rejected so the comparison is not attempted.
fn parsed_args<I>(args: I) -> Result<Args, CliError>
where
    I: IntoIterator<Item = String>,
{
    let mut args = args.into_iter();
    let left = args.next().ok_or(CliError::Usage)?;
    let right = args.next().ok_or(CliError::Usage)?;
    let output = args.next().ok_or(CliError::Usage)?;
    if args.next().is_some() {
        return Err(CliError::Usage);
    }
    require_dir(&left)?;
    require_dir(&right)?;
    Ok(Args {
        sources: Sources { left, right },
        output,
    })
}

/// Confirms that a source path is a directory.
///
/// The comparison is defined on two file trees, so a file or a missing path cannot be a source.
fn require_dir(path: &str) -> Result<(), CliError> {
    if Path::new(path).is_dir() {
        Ok(())
    } else {
        Err(CliError::NotDirectory(path.to_string()))
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

/// The project files in a source tree.
///
/// Generated build output is omitted so it is not treated as project content.
pub(crate) fn collect_files(root: &Path) -> Result<Vec<String>, String> {
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
