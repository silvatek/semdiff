use std::fs;
use std::process::ExitCode;

/// Saves the semdiff file and lists the shared paths.
///
/// The file is the command's result. The listing shows which files were eligible for
/// comparison. A failure to save the file returns exit code 1.
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

/// The semdiff text for one comparison.
///
/// The header is always present. Renames are included only when some were found, so an
/// unchanged pair does not gain an empty change list.
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

/// The header that identifies a semdiff file.
///
/// It records which tool version produced the file and the UTC time it was produced.
fn header_document(version: &str, timestamp: &str) -> String {
    format!("header:\n  semdiff: \"{version}\"\n  timestamp: \"{timestamp}\"\n")
}

/// A YAML-safe form of a text value.
///
/// Paths must stay literal when they are written into the semdiff file.
fn yaml_string(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

/// The UTC time to record in a semdiff header.
///
/// Readers can see when the file was produced without depending on a local timezone.
fn utc_timestamp() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}
