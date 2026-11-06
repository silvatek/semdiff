use super::run;

use std::fs;

#[test]
fn requires_an_output_path() {
    let output = run(&[]);
    let stderr = String::from_utf8(output.stderr).expect("stderr was not utf-8");

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        stderr,
        "Usage: semdiff <left> <right> <output>\n       semdiff --apply [--ignore-hash] <semdif> <directory>\n"
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
