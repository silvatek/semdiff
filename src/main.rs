use std::env;
use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    match parse_dirs(env::args().skip(1)) {
        Ok((left, right)) => {
            println!("Semdiff {}", env!("CARGO_PKG_VERSION"));
            println!("Comparing {left} and {right}");
            ExitCode::SUCCESS
        }
        Err(CliError::Usage) => {
            eprintln!("Usage: semdiff <left> <right>");
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

fn parse_dirs<I>(args: I) -> Result<(String, String), CliError>
where
    I: IntoIterator<Item = String>,
{
    let mut args = args.into_iter();
    let left = args.next().ok_or(CliError::Usage)?;
    let right = args.next().ok_or(CliError::Usage)?;
    if args.next().is_some() {
        return Err(CliError::Usage);
    }
    require_dir(&left)?;
    require_dir(&right)?;
    Ok((left, right))
}

fn require_dir(path: &str) -> Result<(), CliError> {
    if Path::new(path).is_dir() {
        Ok(())
    } else {
        Err(CliError::NotDirectory(path.to_string()))
    }
}
