use super::run;

use std::fs;

#[test]
fn prints_help_when_asked() {
    let output = run(&["--help"]);
    let stdout = String::from_utf8(output.stdout).expect("stdout was not utf-8");
    let stderr = String::from_utf8(output.stderr).expect("stderr was not utf-8");

    assert!(output.status.success());
    assert!(stderr.is_empty());
    assert!(stdout.starts_with(
        "\
Compare two sources and write a semdif. --apply replays a semdif onto a directory.
A semdif is a list of differences, with semantic context where available.
"
    ));
    assert!(stdout.contains("\nArguments\n"));
    assert!(stdout.contains("\nLimitations\n"));
}

#[test]
fn help_takes_precedence_over_the_other_arguments() {
    let output = run(&["--apply", "--help", "missing"]);
    let stdout = String::from_utf8(output.stdout).expect("stdout was not utf-8");

    assert!(output.status.success());
    assert!(stdout.contains("\nArguments\n"));
}

#[test]
fn requires_an_output_path() {
    let output = run(&[]);
    let stderr = String::from_utf8(output.stderr).expect("stderr was not utf-8");

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        stderr,
        "\
Compare two sources and write a semdif. --apply replays a semdif onto a directory.
A semdif is a list of differences, with semantic context where available.

Usage: semdiff [--ignore=<file>] <left> <right> <output>
       semdiff --apply [--ignore=<file>] [--ignore-hash] <semdif> <directory>
       semdiff --help

An output path of - writes the semdif to standard output, and the listing to standard error.
A semdif path of - reads the semdif from standard input.
--help writes a full description to standard output.
"
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
fn rejects_apply_to_a_file() {
    let root = std::env::temp_dir().join(format!("semdiff-apply-file-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let file = root.join("quote.txt");
    let semdif = root.join("out.yml");
    fs::write(&file, "Brevity is the soul of wit.\n").unwrap();
    fs::write(&semdif, "header:\n  semdiff: \"0.1.0\"\n").unwrap();

    let file = file.to_str().unwrap();
    let semdif = semdif.to_str().unwrap();
    let output = run(&["--apply", semdif, file]);
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
    let semdif = root.join("missing.yml");

    let source = source.to_str().unwrap();
    let semdif_arg = semdif.to_str().unwrap();
    let output = run(&["--apply", semdif_arg, source]);
    let stderr = String::from_utf8(output.stderr).expect("stderr was not utf-8");

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stderr, format!("not a file: {}\n", semdif.display()));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rejects_ignore_hash_without_apply() {
    let output = run(&["--ignore-hash", "left", "right", "out.yml"]);
    let stderr = String::from_utf8(output.stderr).expect("stderr was not utf-8");

    assert_eq!(output.status.code(), Some(2));
    assert!(stderr.contains("--ignore-hash"), "{stderr}");
}

#[test]
fn ignore_hash_still_applies_when_a_tree_hash_does_not_match() {
    let root = std::env::temp_dir().join(format!("semdiff-ignore-hash-{}", std::process::id()));
    let source = root.join("source");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("notes.txt"), "left only\n").unwrap();
    fs::write(source.join("quote.txt"), "Brevity is the soul of wit.\n").unwrap();
    let semdif = root.join("out.yml");
    fs::write(
        &semdif,
        "\
header:
  semdiff: \"0.1.0\"
  timestamp: \"2020-01-01T00:00:00Z\"
changes:
  - scope: \".\"
    type: file-deleted
    details:
      path: \"notes.txt\"
      hash: \"0000000000000000000000000000000000000000000000000000000000000000\"
  - scope: \".\"
    type: file-renamed
    details:
      from: \"quote.txt\"
      to: \"quotation.txt\"
      hash: \"0000000000000000000000000000000000000000000000000000000000000000\"
  - scope: \".\"
    type: file-added
    details:
      path: \"other.txt\"
      hash: \"0000000000000000000000000000000000000000000000000000000000000000\"
      contents: \"right only\\n\"
",
    )
    .unwrap();

    let semdif = semdif.to_str().unwrap();
    let source_arg = source.to_str().unwrap();
    let rejected = run(&["--apply", semdif, source_arg]);
    let stderr = String::from_utf8(rejected.stderr).expect("stderr was not utf-8");
    assert_eq!(rejected.status.code(), Some(1));
    assert!(stderr.contains("do not match the hash"), "{stderr}");
    assert_eq!(fs::read(source.join("notes.txt")).unwrap(), b"left only\n");
    assert!(source.join("quote.txt").is_file());
    assert!(!source.join("other.txt").exists());

    let applied = run(&["--apply", "--ignore-hash", semdif, source_arg]);
    let stderr = String::from_utf8(applied.stderr).expect("stderr was not utf-8");
    assert!(applied.status.success(), "{stderr}");
    assert!(!source.join("notes.txt").exists());
    assert!(!source.join("quote.txt").exists());
    assert_eq!(
        fs::read(source.join("quotation.txt")).unwrap(),
        b"Brevity is the soul of wit.\n"
    );
    assert_eq!(fs::read(source.join("other.txt")).unwrap(), b"right only\n");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn an_ignore_file_hides_those_paths_from_compare_and_apply() {
    let root = std::env::temp_dir().join(format!("semdiff-ignore-file-{}", std::process::id()));
    let left = root.join("left");
    let right = root.join("right");
    let edited = root.join("edited");
    fs::create_dir_all(left.join("src/target")).unwrap();
    fs::create_dir_all(right.join("src/target")).unwrap();
    fs::create_dir_all(edited.join("src/target")).unwrap();
    fs::create_dir_all(left.join("target")).unwrap();
    fs::create_dir_all(right.join("target")).unwrap();
    fs::create_dir_all(edited.join("target")).unwrap();
    fs::write(left.join("src/main.rs"), "old\n").unwrap();
    fs::write(right.join("src/main.rs"), "new\n").unwrap();
    fs::write(edited.join("src/main.rs"), "old\n").unwrap();
    fs::write(left.join("src/target/lib.rs"), "old\n").unwrap();
    fs::write(right.join("src/target/lib.rs"), "new\n").unwrap();
    fs::write(edited.join("src/target/lib.rs"), "old\n").unwrap();
    fs::write(left.join("target/skip.txt"), "old\n").unwrap();
    fs::write(right.join("target/skip.txt"), "new\n").unwrap();
    fs::write(edited.join("target/skip.txt"), "old\n").unwrap();
    fs::write(left.join("build.txt"), "old\n").unwrap();
    fs::write(right.join("build.txt"), "new\n").unwrap();
    fs::write(edited.join("build.txt"), "old\n").unwrap();
    let ignore = root.join("ignore.txt");
    fs::write(&ignore, "target\n\nbuild.txt\n").unwrap();

    let output_path = root.join("out.yml");
    let left_arg = left.to_str().unwrap();
    let right_arg = right.to_str().unwrap();
    let output_arg = output_path.to_str().unwrap();
    let ignore_arg = format!("--ignore={}", ignore.display());
    let compared = run(&[&ignore_arg, left_arg, right_arg, output_arg]);
    let stderr = String::from_utf8(compared.stderr).expect("stderr was not utf-8");
    let stdout = String::from_utf8(compared.stdout).expect("stdout was not utf-8");
    assert!(compared.status.success(), "{stderr}");
    let document = fs::read_to_string(&output_path).unwrap();
    assert!(!document.contains("target/skip.txt"), "{document}");
    assert!(!document.contains("build.txt"), "{document}");
    assert!(document.contains("src/main.rs"), "{document}");
    assert!(document.contains("src/target/lib.rs"), "{document}");
    assert!(!stdout.contains("target/skip.txt"), "{stdout}");
    assert!(stdout.contains("src/target/lib.rs"), "{stdout}");

    let edited_arg = edited.to_str().unwrap();
    let applied = run(&["--apply", &ignore_arg, output_arg, edited_arg]);
    let stderr = String::from_utf8(applied.stderr).expect("stderr was not utf-8");
    assert!(applied.status.success(), "{stderr}");
    assert_eq!(fs::read(edited.join("src/main.rs")).unwrap(), b"new\n");
    assert_eq!(
        fs::read(edited.join("src/target/lib.rs")).unwrap(),
        b"new\n"
    );
    assert_eq!(fs::read(edited.join("target/skip.txt")).unwrap(), b"old\n");
    assert_eq!(fs::read(edited.join("build.txt")).unwrap(), b"old\n");

    let missing = run(&["--ignore=missing-ignore", left_arg, right_arg, output_arg]);
    let stderr = String::from_utf8(missing.stderr).expect("stderr was not utf-8");
    assert_eq!(missing.status.code(), Some(1));
    assert_eq!(stderr, "not a file: missing-ignore\n");

    let repeated = run(&[&ignore_arg, &ignore_arg, left_arg, right_arg, output_arg]);
    assert_eq!(repeated.status.code(), Some(2));

    let empty = run(&["--ignore=", left_arg, right_arg, output_arg]);
    assert_eq!(empty.status.code(), Some(2));

    let _ = fs::remove_dir_all(root);
}
