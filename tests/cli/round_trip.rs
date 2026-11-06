use super::run;

use std::collections::BTreeSet;

use semdiff::FileSystem;
use std::fs;
use std::path::Path;

#[test]
fn applying_the_rust_sample_makes_a_copy_of_v1_match_v2() {
    assert_sample_round_trip("TestCode/rust/main", "semdiff-apply-rust");
}

#[test]
fn applying_the_python_sample_makes_a_copy_of_v1_match_v2() {
    assert_sample_round_trip("TestCode/python/main", "semdiff-apply-python");
}

#[test]
fn applying_the_javascript_sample_makes_a_copy_of_v1_match_v2() {
    assert_sample_round_trip("TestCode/javascript/main", "semdiff-apply-javascript");
}

#[test]
fn applying_the_c_sample_makes_a_copy_of_v1_match_v2() {
    assert_sample_round_trip("TestCode/c/main", "semdiff-apply-c");
}

#[test]
fn applying_the_text_sample_makes_a_copy_of_v1_match_v2() {
    assert_sample_round_trip("TestCode/text", "semdiff-apply-text");
}

/// Compares a sample's v1 with v2, applies that semdif to a copy of v1, and
/// checks the copy matches v2.
fn assert_sample_round_trip(relative_sample: &str, tmp_prefix: &str) {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let sample = manifest.join(relative_sample);
    let v1_source = sample.join("v1");
    let v2_source = sample.join("v2");
    let tmp_root = std::env::temp_dir().join(format!("{tmp_prefix}-{}", std::process::id()));
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
    let fs = semdiff::StdFileSystem::new();
    copy_children(&fs, from, from, to);
}

fn copy_children(fs: &semdiff::StdFileSystem, root: &Path, from: &Path, to: &Path) {
    fs.create_dir(to).unwrap();
    for (name, kind) in fs.read_dir(from).unwrap() {
        let path = from.join(&name);
        let relative = path
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        if fs.is_ignored(&relative) {
            continue;
        }
        let dest = to.join(name);
        match kind {
            semdiff::NodeKind::Directory => copy_children(fs, root, &path, &dest),
            semdiff::NodeKind::File => {
                fs::copy(&path, &dest).unwrap();
            }
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
    semdiff::StdFileSystem::new()
        .collect_files(root)
        .unwrap()
        .into_iter()
        .collect()
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
