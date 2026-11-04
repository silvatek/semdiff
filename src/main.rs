use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    match parse_args(env::args().skip(1)) {
        Ok((left, right, output)) => match paired_files(&left, &right) {
            Ok(paths) => {
                let document = header_document(env!("CARGO_PKG_VERSION"), &utc_timestamp());
                if let Err(error) = fs::write(&output, document) {
                    eprintln!("cannot write {output}: {error}");
                    return ExitCode::from(1);
                }
                println!("Semdiff {}", env!("CARGO_PKG_VERSION"));
                println!("Comparing {left} and {right}");
                for path in paths {
                    println!("{path}");
                }
                ExitCode::SUCCESS
            }
            Err(message) => {
                eprintln!("{message}");
                ExitCode::from(1)
            }
        },
        Err(CliError::Usage) => {
            eprintln!("Usage: semdiff <left> <right> <output>");
            ExitCode::from(2)
        }
        Err(CliError::NotDirectory(path)) => {
            eprintln!("not a directory: {path}");
            ExitCode::from(1)
        }
    }
}

enum CliError {
    Usage,
    NotDirectory(String),
}

fn parse_args<I>(args: I) -> Result<(String, String, String), CliError>
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
    Ok((left, right, output))
}

fn header_document(version: &str, timestamp: &str) -> String {
    format!("header:\n  semdiff: \"{version}\"\n  timestamp: \"{timestamp}\"\n")
}

fn utc_timestamp() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

fn require_dir(path: &str) -> Result<(), CliError> {
    if Path::new(path).is_dir() {
        Ok(())
    } else {
        Err(CliError::NotDirectory(path.to_string()))
    }
}

fn paired_files(left: &str, right: &str) -> Result<Vec<String>, String> {
    let right_files: BTreeSet<_> = collect_files(Path::new(right))?.into_iter().collect();
    let mut shared: Vec<_> = collect_files(Path::new(left))?
        .into_iter()
        .filter(|path| right_files.contains(path))
        .collect();
    shared.sort();
    shared.dedup();
    Ok(shared)
}

fn collect_files(root: &Path) -> Result<Vec<String>, String> {
    let mut files = Vec::new();
    visit_dir(root, root, &mut files)?;
    Ok(files)
}

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
