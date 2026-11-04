mod output;
mod rename;

use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

/// Parses the command-line arguments, compares the two directories, and writes the semdiff file.
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

/// Reads the left directory, right directory, and output path from the command line.
///
/// Exits with code 2 when the arguments do not match the usage, and with code 1 when a
/// path is not a directory.
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

/// Collects the relative paths present in both sources and the renamed files between them.
fn find_differences(sources: &Sources) -> Differences {
    let paths = paired_files(sources).unwrap_or_else(|message| {
        exit_with_message(message);
    });
    let renames = rename::file_renames(sources).unwrap_or_else(|message| {
        exit_with_message(message);
    });
    Differences { paths, renames }
}

/// Prints `message` to stderr and exits with code 1.
fn exit_with_message(message: String) -> ! {
    eprintln!("{message}");
    std::process::exit(1);
}

enum CliError {
    Usage,
    NotDirectory(String),
}

/// Turns three path arguments into [`Args`].
///
/// Returns [`CliError::Usage`] when the argument count is wrong, and
/// [`CliError::NotDirectory`] when either source path is not a directory.
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

/// Accepts `path` when it names a directory.
fn require_dir(path: &str) -> Result<(), CliError> {
    if Path::new(path).is_dir() {
        Ok(())
    } else {
        Err(CliError::NotDirectory(path.to_string()))
    }
}

/// Returns the sorted relative paths that exist in both source directories.
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

/// Lists the files under `root` as relative paths, skipping directories named `target`.
pub(crate) fn collect_files(root: &Path) -> Result<Vec<String>, String> {
    let mut files = Vec::new();
    visit_dir(root, root, &mut files)?;
    Ok(files)
}

/// Walks `dir` and appends file paths relative to `root`.
///
/// Directories named `target` are not entered.
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
