//! Semantic diff of two source trees.

mod apply;
mod args;
mod change;
mod dialect;
mod diff;
mod family;
mod filesystem;
mod help;
mod semdif;

pub use filesystem::{FileSystem, MemoryFileSystem, NodeKind, StdFileSystem};

pub(crate) use args::Sources;

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
            match diff::find_differences(&fs, &args.sources) {
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
    let differences = diff::find_differences(fs, &sources)?;
    Ok(semdif::document(&differences.changes))
}

/// Replays a semdif document onto `source`.
pub fn apply_document(fs: &dyn FileSystem, document: &str, source: &str) -> Result<(), String> {
    apply::apply_document(fs, document, Path::new(source), false)
}
