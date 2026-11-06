use std::env;
use std::path::Path;

use crate::filesystem::{FileSystem, StdFileSystem};

const USAGE: &str = "\
Usage: semdiff <left> <right> <output>
       semdiff --apply [--ignore-hash] <semdif> <directory>
";

/// The requested command, either a comparison or an apply.
pub(crate) enum Command {
    Compare(Args),
    Apply {
        semdif: String,
        source: String,
        /// Apply an added, deleted, or renamed file even when its hash does not match.
        ignore_hash: bool,
    },
}

pub(crate) struct Args {
    pub(crate) sources: Sources,
    pub(crate) output: String,
}

/// The two paths being compared.
///
/// Keeping them together lets comparison steps take one value instead of a pair of paths.
pub(crate) struct Sources {
    pub(crate) left: String,
    pub(crate) right: String,
}

/// The requested command, either a comparison or an apply.
///
/// A comparison source may be a file or a directory. Apply takes one directory,
/// because a rename moves a file inside that tree. An unusable invocation stops
/// the command: code 2 for the wrong arguments, and code 1 when a path is missing
/// or the two comparison sources are different kinds.
pub(crate) fn parse_args() -> Command {
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

enum CliError {
    Usage,
    NotSource(String),
    NotDirectory(String),
    NotFile(String),
    MixedSources,
}

/// The command described by the arguments.
///
/// `--apply` edits one directory from a semdif file. `--ignore-hash` still
/// applies an added, deleted, or renamed file when its hash does not match.
/// Otherwise the three paths are a comparison. A missing source is rejected,
/// and a file is not compared with a directory, so the command is not attempted.
fn parsed_args<I>(args: I) -> Result<Command, CliError>
where
    I: IntoIterator<Item = String>,
{
    let args: Vec<String> = args.into_iter().collect();
    if args
        .iter()
        .any(|arg| arg == "--apply" || arg == "--ignore-hash")
    {
        return parse_apply(args);
    }
    if args.len() != 3 {
        return Err(CliError::Usage);
    }
    let left = args[0].clone();
    let right = args[1].clone();
    let output = args[2].clone();
    require_source(&left)?;
    require_source(&right)?;
    require_same_kind(&left, &right)?;
    Ok(Command::Compare(Args {
        sources: Sources { left, right },
        output,
    }))
}

/// `--apply`, with an optional `--ignore-hash`, and the semdif and directory.
///
/// The flag may sit anywhere among those arguments. It does not apply a
/// comparison, and a second copy of either flag is rejected.
fn parse_apply(args: Vec<String>) -> Result<Command, CliError> {
    let mut ignore_hash = false;
    let mut saw_apply = false;
    let mut paths = Vec::new();
    for arg in args {
        match arg.as_str() {
            "--apply" => {
                if saw_apply {
                    return Err(CliError::Usage);
                }
                saw_apply = true;
            }
            "--ignore-hash" => {
                if ignore_hash {
                    return Err(CliError::Usage);
                }
                ignore_hash = true;
            }
            other if other.starts_with('-') => return Err(CliError::Usage),
            other => paths.push(other.to_string()),
        }
    }
    if !saw_apply || paths.len() != 2 {
        return Err(CliError::Usage);
    }
    let semdif = paths.remove(0);
    let source = paths.remove(0);
    let fs = StdFileSystem::new();
    if !fs.is_file(Path::new(&semdif)) {
        return Err(CliError::NotFile(semdif));
    }
    if !fs.is_dir(Path::new(&source)) {
        return Err(CliError::NotDirectory(source));
    }
    Ok(Command::Apply {
        semdif,
        source,
        ignore_hash,
    })
}

/// Confirms that a source path is a file or a directory.
///
/// A missing path cannot be compared.
fn require_source(path: &str) -> Result<(), CliError> {
    let fs = StdFileSystem::new();
    if fs.is_file(Path::new(path)) || fs.is_dir(Path::new(path)) {
        Ok(())
    } else {
        Err(CliError::NotSource(path.to_string()))
    }
}

/// Confirms that both sources are files or both are directories.
///
/// A file and a directory are different comparison units, so they are not paired.
fn require_same_kind(left: &str, right: &str) -> Result<(), CliError> {
    let fs = StdFileSystem::new();
    if fs.is_dir(Path::new(left)) == fs.is_dir(Path::new(right)) {
        Ok(())
    } else {
        Err(CliError::MixedSources)
    }
}
