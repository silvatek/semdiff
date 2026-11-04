use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::process::Command;

fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_semdiff"))
        .args(args)
        .output()
        .expect("failed to run semdiff")
}

#[test]
fn prints_name_and_version_when_comparing_directories() {
    let root = std::env::temp_dir().join(format!("semdiff-cli-{}", std::process::id()));
    let left = root.join("left");
    let right = root.join("right");
    fs::create_dir_all(&left).unwrap();
    fs::create_dir_all(&right).unwrap();

    let output_path = root.join("out.yml");
    let left = left.to_str().unwrap();
    let right = right.to_str().unwrap();
    let output_path = output_path.to_str().unwrap();
    let output = run(&[left, right, output_path]);
    let stdout = String::from_utf8(output.stdout).expect("stdout was not utf-8");
    let document = fs::read_to_string(output_path).expect("failed to read semdif file");

    assert!(output.status.success());
    assert_eq!(
        stdout,
        format!(
            "Semdiff {}\nComparing {left} and {right}\n",
            env!("CARGO_PKG_VERSION")
        )
    );
    let version_line = format!("  semdiff: \"{}\"", env!("CARGO_PKG_VERSION"));
    let mut lines = document.lines();
    assert_eq!(lines.next(), Some("header:"));
    assert_eq!(lines.next(), Some(version_line.as_str()));
    let timestamp = lines
        .next()
        .and_then(|line| line.strip_prefix("  timestamp: \""))
        .and_then(|value| value.strip_suffix('"'))
        .expect("timestamp line");
    assert_eq!(timestamp.len(), 20, "{timestamp}");
    assert_eq!(&timestamp[4..5], "-");
    assert_eq!(&timestamp[10..11], "T");
    assert!(timestamp.ends_with('Z'), "{timestamp}");
    assert!(lines.next().is_none());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn prints_paths_present_in_both_directories() {
    let root = std::env::temp_dir().join(format!("semdiff-pairs-{}", std::process::id()));
    let left = root.join("left");
    let right = root.join("right");
    fs::create_dir_all(left.join("src")).unwrap();
    fs::create_dir_all(right.join("src")).unwrap();
    fs::write(left.join("src/main.rs"), "fn fibonacci() {}\n").unwrap();
    fs::write(right.join("src/main.rs"), "fn fib() {}\n").unwrap();
    fs::write(left.join("only-left.rs"), "").unwrap();
    fs::create_dir_all(left.join("target")).unwrap();
    fs::write(left.join("target/skip.txt"), "").unwrap();
    fs::create_dir_all(right.join("target")).unwrap();
    fs::write(right.join("target/skip.txt"), "").unwrap();

    let output_path = root.join("out.yml");
    let left = left.to_str().unwrap();
    let right = right.to_str().unwrap();
    let output_path = output_path.to_str().unwrap();
    let output = run(&[left, right, output_path]);
    let stdout = String::from_utf8(output.stdout).expect("stdout was not utf-8");

    assert!(output.status.success());
    assert_eq!(
        stdout,
        format!(
            "Semdiff {}\nComparing {left} and {right}\nsrc/main.rs\n",
            env!("CARGO_PKG_VERSION")
        )
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn writes_a_file_renamed_change() {
    let root = std::env::temp_dir().join(format!("semdiff-rename-{}", std::process::id()));
    let left = root.join("left");
    let right = root.join("right");
    fs::create_dir_all(&left).unwrap();
    fs::create_dir_all(&right).unwrap();
    let quote = "Brevity is the soul of wit.\n";
    fs::write(left.join("quote.txt"), quote).unwrap();
    fs::write(right.join("quotation.txt"), quote).unwrap();
    fs::write(left.join("notes.txt"), "left only\n").unwrap();
    fs::write(right.join("other.txt"), "right only\n").unwrap();

    let output_path = root.join("out.yml");
    let left = left.to_str().unwrap();
    let right = right.to_str().unwrap();
    let output_path = output_path.to_str().unwrap();
    let output = run(&[left, right, output_path]);
    let document = fs::read_to_string(output_path).expect("failed to read semdif file");

    assert!(output.status.success());
    let changes = document.split_once("changes:\n").expect("changes list").1;
    assert_eq!(
        changes,
        "  - scope: \".\"\n    type: file-renamed\n    details:\n      from: \"quote.txt\"\n      to: \"quotation.txt\"\n"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn hashes_past_the_first_buffer() {
    let root = std::env::temp_dir().join(format!("semdiff-hash-{}", std::process::id()));
    let left = root.join("left");
    let right = root.join("right");
    fs::create_dir_all(&left).unwrap();
    fs::create_dir_all(&right).unwrap();

    let mut same = vec![1_u8; 9_000];
    same[8_500] = 7;
    let shared_prefix = vec![2_u8; 9_000];
    let mut shared_prefix_changed = shared_prefix.clone();
    shared_prefix_changed[8_500] = 4;
    fs::write(left.join("same.bin"), &same).unwrap();
    fs::write(right.join("same-renamed.bin"), &same).unwrap();
    fs::write(left.join("left.bin"), &shared_prefix).unwrap();
    fs::write(right.join("right.bin"), &shared_prefix_changed).unwrap();

    let output_path = root.join("out.yml");
    let left = left.to_str().unwrap();
    let right = right.to_str().unwrap();
    let output_path = output_path.to_str().unwrap();
    let output = run(&[left, right, output_path]);
    let document = fs::read_to_string(output_path).expect("failed to read semdif file");

    assert!(output.status.success());
    let changes = document.split_once("changes:\n").expect("changes list").1;
    assert_eq!(
        changes,
        "  - scope: \".\"\n    type: file-renamed\n    details:\n      from: \"same.bin\"\n      to: \"same-renamed.bin\"\n"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn records_an_added_rust_line_comment() {
    let root = std::env::temp_dir().join(format!("semdiff-comment-{}", std::process::id()));
    let left = root.join("left");
    let right = root.join("right");
    fs::create_dir_all(left.join("src")).unwrap();
    fs::create_dir_all(right.join("src")).unwrap();
    fs::write(left.join("src/main.rs"), "fn fibonacci() {}\n/// Shared.\n").unwrap();
    fs::write(
        right.join("src/main.rs"),
        "/// Returns the nth Fibonacci number.\n// Keep this.\nfn fib() {} // trailing\n/// Shared.\n    /// Inner detail.\n",
    )
    .unwrap();
    fs::write(left.join("notes.txt"), "").unwrap();
    fs::write(right.join("notes.txt"), "/// not rust\n").unwrap();

    let output_path = root.join("out.yml");
    let left = left.to_str().unwrap();
    let right = right.to_str().unwrap();
    let output_path = output_path.to_str().unwrap();
    let output = run(&[left, right, output_path]);
    let document = fs::read_to_string(output_path).expect("failed to read semdif file");

    assert!(output.status.success());
    let changes = document.split_once("changes:\n").expect("changes list").1;
    assert_eq!(
        changes,
        concat!(
            "  - scope: \"src/main.rs\"\n",
            "    type: line-comment-added\n",
            "    details:\n",
            "      line: 1\n",
            "      text: \"/// Returns the nth Fibonacci number.\"\n",
            "  - scope: \"src/main.rs\"\n",
            "    type: line-comment-added\n",
            "    details:\n",
            "      line: 2\n",
            "      text: \"// Keep this.\"\n",
            "  - scope: \"src/main.rs\"\n",
            "    type: line-comment-added\n",
            "    details:\n",
            "      line: 5\n",
            "      text: \"    /// Inner detail.\"\n",
        )
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn requires_an_output_path() {
    let output = run(&[]);
    let stderr = String::from_utf8(output.stderr).expect("stderr was not utf-8");

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        stderr,
        "Usage: semdiff <left> <right> <output>\n       semdiff --apply <source> <semdif>\n"
    );
}

#[test]
fn rejects_a_missing_source() {
    let output = run(&["missing-left", "missing-right", "out.yml"]);
    let stderr = String::from_utf8(output.stderr).expect("stderr was not utf-8");

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stderr, "not a file or directory: missing-left\n");
}

#[test]
fn compares_individual_files() {
    let root = std::env::temp_dir().join(format!("semdiff-files-{}", std::process::id()));
    let left_dir = root.join("left");
    let right_dir = root.join("right");
    fs::create_dir_all(&left_dir).unwrap();
    fs::create_dir_all(&right_dir).unwrap();
    fs::write(left_dir.join("quote.txt"), "Brevity is the soul of wit.\n").unwrap();
    fs::write(right_dir.join("quote.txt"), "Brevity is the soul of wit.\n").unwrap();

    let output_path = root.join("out.yml");
    let left = left_dir.join("quote.txt");
    let right = right_dir.join("quote.txt");
    let left = left.to_str().unwrap();
    let right = right.to_str().unwrap();
    let output_path = output_path.to_str().unwrap();
    let output = run(&[left, right, output_path]);
    let stdout = String::from_utf8(output.stdout).expect("stdout was not utf-8");
    let document = fs::read_to_string(output_path).expect("failed to read semdif file");

    assert!(output.status.success());
    assert!(stdout.ends_with("quote.txt\n"), "{stdout}");
    assert!(!document.contains("changes:"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn detects_a_rename_between_individual_files() {
    let root = std::env::temp_dir().join(format!("semdiff-file-rename-{}", std::process::id()));
    let left_dir = root.join("left");
    let right_dir = root.join("right");
    fs::create_dir_all(&left_dir).unwrap();
    fs::create_dir_all(&right_dir).unwrap();
    let quote = "Brevity is the soul of wit.\n";
    fs::write(left_dir.join("quote.txt"), quote).unwrap();
    fs::write(right_dir.join("quotation.txt"), quote).unwrap();

    let output_path = root.join("out.yml");
    let left = left_dir.join("quote.txt");
    let right = right_dir.join("quotation.txt");
    let left = left.to_str().unwrap();
    let right = right.to_str().unwrap();
    let output_path = output_path.to_str().unwrap();
    let output = run(&[left, right, output_path]);
    let document = fs::read_to_string(output_path).expect("failed to read semdif file");

    assert!(output.status.success());
    let changes = document.split_once("changes:\n").expect("changes list").1;
    assert_eq!(
        changes,
        "  - scope: \".\"\n    type: file-renamed\n    details:\n      from: \"quote.txt\"\n      to: \"quotation.txt\"\n"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rejects_a_file_compared_with_a_directory() {
    let root = std::env::temp_dir().join(format!("semdiff-mixed-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let file = root.join("quote.txt");
    fs::write(&file, "Brevity is the soul of wit.\n").unwrap();

    let file = file.to_str().unwrap();
    let directory = root.to_str().unwrap();
    let output = run(&[file, directory, "out.yml"]);
    let stderr = String::from_utf8(output.stderr).expect("stderr was not utf-8");

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        stderr,
        "sources must both be files or both be directories\n"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn applying_the_sample_diff_makes_v3_identical_to_v2() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let v1 = manifest.join("TestCode/rust/main/v1");
    let v2 = manifest.join("TestCode/rust/main/v2");
    let v3 = manifest.join("TestCode/rust/main/v3");
    let root = std::env::temp_dir().join(format!("semdiff-apply-{}", std::process::id()));
    let applied = root.join("v3");
    copy_tree(&v3, &applied);
    let diff = root.join("diff.yml");

    let compared = run(&[
        v1.to_str().unwrap(),
        v2.to_str().unwrap(),
        diff.to_str().unwrap(),
    ]);
    assert!(
        compared.status.success(),
        "{}",
        String::from_utf8_lossy(&compared.stderr)
    );
    let applied_output = run(&["--apply", applied.to_str().unwrap(), diff.to_str().unwrap()]);
    assert!(
        applied_output.status.success(),
        "{}",
        String::from_utf8_lossy(&applied_output.stderr)
    );

    assert_eq!(tree_difference(&applied, &v2), "");

    let _ = fs::remove_dir_all(root);
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

fn tree_difference(actual: &Path, expected: &Path) -> String {
    let actual_files = project_files(actual);
    let expected_files = project_files(expected);
    let paths: BTreeSet<_> = actual_files.union(&expected_files).cloned().collect();
    let mut report = String::new();
    for path in paths {
        let actual_path = actual.join(&path);
        let expected_path = expected.join(&path);
        match (actual_files.contains(&path), expected_files.contains(&path)) {
            (false, true) => report.push_str(&format!("only in expected: {path}\n")),
            (true, false) => report.push_str(&format!("only in actual: {path}\n")),
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
