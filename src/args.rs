use std::env;
use std::path::Path;

const USAGE: &str = "\
Usage: semdiff <left> <right> <output>
       semdiff --apply <semdif> <directory>
";

/// The requested command, either a comparison or an apply.
pub(crate) enum Command {
    Compare(Args),
    Apply { semdif: String, source: String },
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
        let semdif = args.next().ok_or(CliError::Usage)?;
        let source = args.next().ok_or(CliError::Usage)?;
        if args.next().is_some() {
            return Err(CliError::Usage);
        }
        if !Path::new(&semdif).is_file() {
            return Err(CliError::NotFile(semdif));
        }
        if !Path::new(&source).is_dir() {
            return Err(CliError::NotDirectory(source));
        }
        return Ok(Command::Apply { semdif, source });
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
