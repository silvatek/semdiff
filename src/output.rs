use std::fs;
use std::process::ExitCode;

/// Writes the semdiff document to `output` and prints the shared paths.
///
/// Returns an exit code of 1 when the file cannot be written.
pub(crate) fn write_output(
    output: &str,
    sources: &crate::Sources,
    differences: &crate::Differences,
) -> ExitCode {
    let document = semdiff_document(
        env!("CARGO_PKG_VERSION"),
        &utc_timestamp(),
        &differences.renames,
    );
    if let Err(error) = fs::write(output, document) {
        eprintln!("cannot write {output}: {error}");
        return ExitCode::from(1);
    }
    println!("Semdiff {}", env!("CARGO_PKG_VERSION"));
    println!("Comparing {} and {}", sources.left, sources.right);
    for path in &differences.paths {
        println!("{path}");
    }
    ExitCode::SUCCESS
}

/// Builds the YAML document, including a `changes` list when any files were renamed.
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

/// Builds the YAML header containing the semdiff version and a UTC timestamp.
fn header_document(version: &str, timestamp: &str) -> String {
    format!("header:\n  semdiff: \"{version}\"\n  timestamp: \"{timestamp}\"\n")
}

/// Quotes `value` as a YAML double-quoted scalar.
fn yaml_string(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

/// Returns the current UTC time as `YYYY-MM-DDTHH:MM:SSZ`.
fn utc_timestamp() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}
