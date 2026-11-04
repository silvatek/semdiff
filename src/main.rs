use std::collections::{BTreeSet, HashMap};
use std::env;
use std::fs;
use std::io::Read;
use std::path::Path;
use std::process::ExitCode;

use sha2::{Digest, Sha256};

const HASH_BUFFER_SIZE: usize = 8 * 1024;

fn main() -> ExitCode {
    match parse_args(env::args().skip(1)) {
        Ok((left, right, output)) => {
            let paths = match paired_files(&left, &right) {
                Ok(paths) => paths,
                Err(message) => {
                    eprintln!("{message}");
                    return ExitCode::from(1);
                }
            };
            let renames = match file_renames(&left, &right) {
                Ok(renames) => renames,
                Err(message) => {
                    eprintln!("{message}");
                    return ExitCode::from(1);
                }
            };
            let document = semdiff_document(env!("CARGO_PKG_VERSION"), &utc_timestamp(), &renames);
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

fn semdiff_document(version: &str, timestamp: &str, renames: &[(String, String)]) -> String {
    let mut document = header_document(version, timestamp);
    if renames.is_empty() {
        return document;
    }
    document.push_str("changes:\n");
    for (from, to) in renames {
        document.push_str("  - scope: \".\"\n");
        document.push_str("    type: file-renamed\n");
        document.push_str("    details:\n");
        document.push_str(&format!("      from: {}\n", yaml_string(from)));
        document.push_str(&format!("      to: {}\n", yaml_string(to)));
    }
    document
}

fn yaml_string(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
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

fn file_renames(left: &str, right: &str) -> Result<Vec<(String, String)>, String> {
    let left_root = Path::new(left);
    let right_root = Path::new(right);
    let left_files = collect_files(left_root)?;
    let right_files = collect_files(right_root)?;
    let left_names: BTreeSet<_> = left_files.iter().cloned().collect();
    let right_names: BTreeSet<_> = right_files.iter().cloned().collect();

    let mut left_only = HashMap::<[u8; 32], Vec<String>>::new();
    for path in left_files
        .into_iter()
        .filter(|path| !right_names.contains(path))
    {
        left_only
            .entry(file_hash(left_root, &path)?)
            .or_default()
            .push(path);
    }
    let mut right_only = HashMap::<[u8; 32], Vec<String>>::new();
    for path in right_files
        .into_iter()
        .filter(|path| !left_names.contains(path))
    {
        right_only
            .entry(file_hash(right_root, &path)?)
            .or_default()
            .push(path);
    }

    let mut renames = Vec::new();
    for (hash, from_paths) in left_only {
        let Some(to_paths) = right_only.get(&hash) else {
            continue;
        };
        if from_paths.len() == 1 && to_paths.len() == 1 {
            renames.push((from_paths[0].clone(), to_paths[0].clone()));
        }
    }
    renames.sort();
    Ok(renames)
}

fn file_hash(root: &Path, relative: &str) -> Result<[u8; 32], String> {
    let path = root.join(relative);
    let mut file = fs::File::open(&path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; HASH_BUFFER_SIZE];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(hasher.finalize().into())
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
