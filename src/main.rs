mod apply;
mod rename;
mod semdif;

use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

const USAGE: &str = "\
Usage: semdiff <left> <right> <output>
       semdiff --apply <semdif> <directory>
";

/// Compares two sources, or applies a semdif file to one directory.
///
/// Argument errors stop the command before a comparison or edit starts.
fn main() -> ExitCode {
    match parse_args() {
        Command::Compare(args) => {
            let differences = find_differences(&args.sources);
            semdif::write_output(&args.output, &args.sources, &differences)
        }
        Command::Apply { patch, source } => apply::apply(&patch, &source),
    }
}

enum Command {
    Compare(Args),
    Apply { patch: String, source: String },
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

/// The requested command, either a comparison or an apply.
///
/// A comparison source may be a file or a directory. Apply takes one directory,
/// because a rename moves a file inside that tree. An unusable invocation stops
/// the command: code 2 for the wrong arguments, and code 1 when a path is missing
/// or the two comparison sources are different kinds.
fn parse_args() -> Command {
    match parsed_args(env::args().skip(1)) {
        Ok(command) => command,
        Err(CliError::Usage) => {
            eprint!("{USAGE}");
            std::process::exit(2);
        }
        Err(CliError::NotSource(path)) => {
            eprintln!("not a file or directory: {path}");
            std::process::exit(1);
        }
        Err(CliError::NotDirectory(path)) => {
            eprintln!("not a directory: {path}");
            std::process::exit(1);
        }
        Err(CliError::NotFile(path)) => {
            eprintln!("not a file: {path}");
            std::process::exit(1);
        }
        Err(CliError::MixedSources) => {
            eprintln!("sources must both be files or both be directories");
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
    NotSource(String),
    NotDirectory(String),
    NotFile(String),
    MixedSources,
}

/// The command described by the arguments.
///
/// `--apply` edits one directory from a semdif file. Otherwise the three
/// paths are a comparison. A missing source is rejected, and a file is not compared
/// with a directory, so the command is not attempted.
fn parsed_args<I>(args: I) -> Result<Command, CliError>
where
    I: IntoIterator<Item = String>,
{
    let mut args = args.into_iter();
    let first = args.next().ok_or(CliError::Usage)?;
    if first == "--apply" {
        let patch = args.next().ok_or(CliError::Usage)?;
        let source = args.next().ok_or(CliError::Usage)?;
        if args.next().is_some() {
            return Err(CliError::Usage);
        }
        if !Path::new(&patch).is_file() {
            return Err(CliError::NotFile(patch));
        }
        if !Path::new(&source).is_dir() {
            return Err(CliError::NotDirectory(source));
        }
        return Ok(Command::Apply { patch, source });
    }
    let left = first;
    let right = args.next().ok_or(CliError::Usage)?;
    let output = args.next().ok_or(CliError::Usage)?;
    if args.next().is_some() {
        return Err(CliError::Usage);
    }
    require_source(&left)?;
    require_source(&right)?;
    require_same_kind(&left, &right)?;
    Ok(Command::Compare(Args {
        sources: Sources { left, right },
        output,
    }))
}

/// Confirms that a source path is a file or a directory.
///
/// A missing path cannot be compared.
fn require_source(path: &str) -> Result<(), CliError> {
    let path = Path::new(path);
    if path.is_file() || path.is_dir() {
        Ok(())
    } else {
        Err(CliError::NotSource(path.display().to_string()))
    }
}

/// Confirms that both sources are files or both are directories.
///
/// A file and a directory are different comparison units, so they are not paired.
fn require_same_kind(left: &str, right: &str) -> Result<(), CliError> {
    if Path::new(left).is_dir() == Path::new(right).is_dir() {
        Ok(())
    } else {
        Err(CliError::MixedSources)
    }
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
