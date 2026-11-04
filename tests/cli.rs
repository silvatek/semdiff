use std::fs;
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
    let document = fs::read_to_string(output_path).expect("failed to read semdiff file");

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
    let document = fs::read_to_string(output_path).expect("failed to read semdiff file");

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
    let document = fs::read_to_string(output_path).expect("failed to read semdiff file");

    assert!(output.status.success());
    let changes = document.split_once("changes:\n").expect("changes list").1;
    assert_eq!(
        changes,
        "  - scope: \".\"\n    type: file-renamed\n    details:\n      from: \"same.bin\"\n      to: \"same-renamed.bin\"\n"
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
        "Usage: semdiff <left> <right> <output>\n       semdiff --apply <directory> <semdif>\n"
    );
}

#[test]
fn applies_a_rename_inside_a_directory() {
    let root = std::env::temp_dir().join(format!("semdiff-apply-{}", std::process::id()));
    let left = root.join("left");
    let right = root.join("right");
    let edited = root.join("edited");
    fs::create_dir_all(left.join("src")).unwrap();
    fs::create_dir_all(right.join("src")).unwrap();
    fs::create_dir_all(right.join("notes")).unwrap();
    fs::create_dir_all(edited.join("src")).unwrap();
    let quote = "Brevity is the soul of wit.\n";
    let source = "fn fibonacci() {}\n";
    fs::write(left.join("quote.txt"), quote).unwrap();
    fs::write(right.join("notes/quotation.txt"), quote).unwrap();
    fs::write(left.join("src/main.rs"), source).unwrap();
    fs::write(right.join("src/main.rs"), source).unwrap();
    fs::write(edited.join("quote.txt"), quote).unwrap();
    fs::write(edited.join("src/main.rs"), source).unwrap();

    let output_path = root.join("out.yml");
    let left = left.to_str().unwrap();
    let right = right.to_str().unwrap();
    let edited_path = edited.to_str().unwrap();
    let output_path = output_path.to_str().unwrap();
    let compared = run(&[left, right, output_path]);
    let applied = run(&["--apply", edited_path, output_path]);

    assert!(compared.status.success());
    assert!(
        applied.status.success(),
        "{}",
        String::from_utf8_lossy(&applied.stderr)
    );
    assert!(!edited.join("quote.txt").exists());
    assert_eq!(
        fs::read_to_string(edited.join("notes/quotation.txt")).unwrap(),
        quote
    );
    assert_eq!(
        fs::read_to_string(edited.join("src/main.rs")).unwrap(),
        source
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rejects_apply_to_a_file() {
    let root = std::env::temp_dir().join(format!("semdiff-apply-file-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let file = root.join("quote.txt");
    let patch = root.join("out.yml");
    fs::write(&file, "Brevity is the soul of wit.\n").unwrap();
    fs::write(&patch, "header:\n  semdiff: \"0.1.0\"\n").unwrap();

    let file = file.to_str().unwrap();
    let patch = patch.to_str().unwrap();
    let output = run(&["--apply", file, patch]);
    let stderr = String::from_utf8(output.stderr).expect("stderr was not utf-8");

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stderr, format!("not a directory: {file}\n"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rejects_a_missing_semdif_file() {
    let root = std::env::temp_dir().join(format!("semdiff-apply-missing-{}", std::process::id()));
    let source = root.join("source");
    fs::create_dir_all(&source).unwrap();
    let patch = root.join("missing.yml");

    let source = source.to_str().unwrap();
    let patch_arg = patch.to_str().unwrap();
    let output = run(&["--apply", source, patch_arg]);
    let stderr = String::from_utf8(output.stderr).expect("stderr was not utf-8");

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stderr, format!("not a file: {}\n", patch.display()));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rejects_a_change_that_is_not_a_rename() {
    let root = std::env::temp_dir().join(format!("semdiff-apply-type-{}", std::process::id()));
    let source = root.join("source");
    fs::create_dir_all(&source).unwrap();
    let patch = root.join("out.yml");
    fs::write(
        &patch,
        "\
header:
  semdiff: \"0.1.0\"
  timestamp: \"2020-01-01T00:00:00Z\"
changes:
  - scope: \".\"
    type: line-comment-added
    details:
      line: 1
      text: \"/// note\"
",
    )
    .unwrap();

    let source = source.to_str().unwrap();
    let patch = patch.to_str().unwrap();
    let output = run(&["--apply", source, patch]);
    let stderr = String::from_utf8(output.stderr).expect("stderr was not utf-8");

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stderr, "unknown change type line-comment-added\n");

    let _ = fs::remove_dir_all(root);
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
    let document = fs::read_to_string(output_path).expect("failed to read semdiff file");

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
    let document = fs::read_to_string(output_path).expect("failed to read semdiff file");

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
