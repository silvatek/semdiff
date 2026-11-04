use super::run;

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

#[test]
fn applying_the_rust_sample_makes_a_copy_of_v1_match_v2() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let sample = manifest.join("TestCode/rust/main");
    let v1_source = sample.join("v1");
    let v2_source = sample.join("v2");
    let tmp_root = std::env::temp_dir().join(format!("semdiff-apply-rust-{}", std::process::id()));
    let target_source = tmp_root.join("v1");
    copy_tree(&v1_source, &target_source);
    let semdif_file = tmp_root.join("diff.yml");
    let v1_source_arg = v1_source.to_str().unwrap();
    let v2_source_arg = v2_source.to_str().unwrap();
    let semdif_file_arg = semdif_file.to_str().unwrap();
    let target_source_arg = target_source.to_str().unwrap();

    let compared = run(&[v1_source_arg, v2_source_arg, semdif_file_arg]);
    assert!(
        compared.status.success(),
        "{}",
        String::from_utf8_lossy(&compared.stderr)
    );
    let document = fs::read_to_string(&semdif_file).unwrap();
    assert!(
        document.contains(
            "  - scope: \".\"\n    type: file-deleted\n    details:\n      path: \"note.txt\"\n"
        ),
        "{document}"
    );
    assert!(
        document.contains("type: file-added") && document.contains("path: \"added.txt\""),
        "{document}"
    );
    assert!(
        document.contains("He said \\\"hello\\\"") && document.contains("\\\\temp"),
        "{document}"
    );
    let applied_output = run(&["--apply", semdif_file_arg, target_source_arg]);
    assert!(
        applied_output.status.success(),
        "{}",
        String::from_utf8_lossy(&applied_output.stderr)
    );

    assert_eq!(tree_difference(&target_source, &v2_source), "");

    let _ = fs::remove_dir_all(tmp_root);
}

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        if entry.file_name() == "target" {
            continue;
        }
        let dest = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_tree(&entry.path(), &dest);
        } else {
            fs::copy(entry.path(), dest).unwrap();
        }
    }
}

/// The files that differ between the applied copy and v2.
///
/// A failure should show which path differs, and the lines that differ, so the
/// round trip can be checked without a second comparison.
fn tree_difference(actual: &Path, expected: &Path) -> String {
    let actual_files = project_files(actual);
    let expected_files = project_files(expected);
    let paths: BTreeSet<_> = actual_files.union(&expected_files).cloned().collect();
    let mut report = String::new();
    for path in paths {
        let actual_path = actual.join(&path);
        let expected_path = expected.join(&path);
        match (actual_files.contains(&path), expected_files.contains(&path)) {
            (false, true) => report.push_str(&format!("only in v2: {path}\n")),
            (true, false) => report.push_str(&format!("only in applied copy: {path}\n")),
            (true, true) => {
                let actual_bytes = fs::read(&actual_path).unwrap();
                let expected_bytes = fs::read(&expected_path).unwrap();
                if actual_bytes != expected_bytes {
                    report.push_str(&path);
                    report.push('\n');
                    report.push_str(&line_difference(
                        &String::from_utf8_lossy(&actual_bytes),
                        &String::from_utf8_lossy(&expected_bytes),
                    ));
                }
            }
            (false, false) => {}
        }
    }
    report
}

fn project_files(root: &Path) -> BTreeSet<String> {
    let mut files = BTreeSet::new();
    collect_project_files(root, root, &mut files);
    files
}

fn collect_project_files(root: &Path, dir: &Path, files: &mut BTreeSet<String>) {
    for entry in fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        if entry.file_name() == "target" {
            continue;
        }
        if entry.path().is_dir() {
            collect_project_files(root, &entry.path(), files);
        } else {
            let path = entry.path();
            let relative = path.strip_prefix(root).unwrap();
            files.insert(relative.to_string_lossy().replace('\\', "/"));
        }
    }
}

fn line_difference(actual: &str, expected: &str) -> String {
    let actual_lines: Vec<_> = actual.lines().collect();
    let expected_lines: Vec<_> = expected.lines().collect();
    let mut report = String::new();
    for index in 0..actual_lines.len().max(expected_lines.len()) {
        match (actual_lines.get(index), expected_lines.get(index)) {
            (Some(actual_line), Some(expected_line)) if actual_line == expected_line => {}
            (Some(actual_line), Some(expected_line)) => {
                report.push('-');
                report.push_str(actual_line);
                report.push('\n');
                report.push('+');
                report.push_str(expected_line);
                report.push('\n');
            }
            (Some(actual_line), None) => {
                report.push('-');
                report.push_str(actual_line);
                report.push('\n');
            }
            (None, Some(expected_line)) => {
                report.push('+');
                report.push_str(expected_line);
                report.push('\n');
            }
            (None, None) => {}
        }
    }
    report
}
