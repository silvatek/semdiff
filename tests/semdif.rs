//! Code that reads or writes semdif files, including serialising and deserialising
//! change records.

use crate::filesystem::Tree;
use crate::main::{run, run_with_stdin};
use std::fs;

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
    assert_eq!(lines.next(), Some("  hash: \"sha256\""));
    assert_eq!(lines.next(), Some("  text-encoding: \"utf-8\""));
    assert_eq!(lines.next(), Some("  binary-encoding: \"base64\""));
    assert!(lines.next().is_none());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn a_dash_writes_the_semdif_to_stdout_and_reads_it_from_stdin() {
    let root = std::env::temp_dir().join(format!("semdiff-stdio-{}", std::process::id()));
    let left = root.join("left");
    let right = root.join("right");
    let edited = root.join("edited");
    fs::create_dir_all(&left).unwrap();
    fs::create_dir_all(&right).unwrap();
    fs::create_dir_all(&edited).unwrap();
    fs::write(left.join("note.txt"), "old\n").unwrap();
    fs::write(right.join("note.txt"), "new\n").unwrap();
    fs::write(edited.join("note.txt"), "old\n").unwrap();

    let left = left.to_str().unwrap();
    let right = right.to_str().unwrap();
    let output = run(&[left, right, "-"]);
    let stdout = String::from_utf8(output.stdout).expect("stdout was not utf-8");
    let stderr = String::from_utf8(output.stderr).expect("stderr was not utf-8");

    assert!(output.status.success(), "{stderr}");
    assert!(stdout.starts_with("header:\n"), "{stdout}");
    assert!(stdout.contains("text: \"new\"\n"), "{stdout}");
    assert!(!stdout.contains("Semdiff "), "{stdout}");
    assert_eq!(
        stderr,
        format!(
            "Semdiff {}\nComparing {left} and {right}\nnote.txt\n",
            env!("CARGO_PKG_VERSION")
        )
    );

    let edited = edited.to_str().unwrap();
    let applied = run_with_stdin(&["--apply", "-", edited], stdout.as_bytes());
    let apply_stderr = String::from_utf8(applied.stderr).expect("stderr was not utf-8");
    assert!(applied.status.success(), "{apply_stderr}");
    assert_eq!(fs::read(root.join("edited/note.txt")).unwrap(), b"new\n");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rejects_a_change_that_is_not_a_rename() {
    let tree = Tree::new();
    tree.dir("source");
    let message = tree
        .apply(
            "source",
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
        .unwrap_err();

    assert_eq!(message, "unknown change type line-comment-added");
}

#[test]
fn rejects_a_line_removal_whose_line_count_is_positive() {
    let tree = Tree::new();
    tree.write("edited/note.txt", "keep\ngone\n");
    let message = tree
        .apply(
            "edited",
            "\
header:
  semdiff: \"0.1.0\"
  timestamp: \"2020-01-01T00:00:00Z\"
changes:
  - scope: \"note.txt\"
    type: line-removed
    details:
      old: 2
      lines: 2
      new: 2
      text: \"gone\"
",
        )
        .unwrap_err();

    assert!(message.contains("invalid lines 2"), "{message}");
    assert_eq!(tree.read("edited/note.txt"), b"keep\ngone\n");
}

#[test]
fn rejects_an_identifier_rename_whose_line_is_zero() {
    let tree = Tree::new();
    tree.write("edited/src/main.rs", "fn fibonacci() {}\n");
    let message = tree
        .apply(
            "edited",
            "\
header:
  semdiff: \"0.1.0\"
  timestamp: \"2020-01-01T00:00:00Z\"
changes:
  - scope: \"src/main.rs:0\"
    type: identifier-renamed
    details:
      new: 1
      oldname: \"fibonacci\"\n      newname: \"fib\"
",
        )
        .unwrap_err();

    assert!(message.contains("path:line"), "{message}");
    assert_eq!(tree.read("edited/src/main.rs"), b"fn fibonacci() {}\n");
}

#[test]
fn rejects_a_line_change_with_an_invalid_comment_flag() {
    let tree = Tree::new();
    tree.write("edited/src/main.rs", "fn main() {}\n");

    let message = tree
        .apply(
            "edited",
            "\
header:
  semdiff: \"0.1.0\"
  timestamp: \"2020-01-01T00:00:00Z\"
changes:
  - scope: \"src/main.rs\"
    type: line-added
    details:
      old: 1
      lines: +1
      new: 1
      text: \"// note\"
      isComment: yes
",
        )
        .unwrap_err();

    assert!(message.contains("invalid isComment yes"), "{message}");
    assert_eq!(tree.read("edited/src/main.rs"), b"fn main() {}\n");
}

#[test]
fn rejects_a_line_change_with_an_invalid_whitespace_flag() {
    let tree = Tree::new();
    tree.write("edited/src/main.rs", "fn main() {}\n");

    let message = tree
        .apply(
            "edited",
            "\
header:
  semdiff: \"0.1.0\"
  timestamp: \"2020-01-01T00:00:00Z\"
changes:
  - scope: \"src/main.rs\"
    type: line-added
    details:
      old: 1
      lines: +1
      new: 1
      text: \"    \"
      isWhitespace: yes
",
        )
        .unwrap_err();

    assert!(message.contains("invalid isWhitespace yes"), "{message}");
    assert_eq!(tree.read("edited/src/main.rs"), b"fn main() {}\n");
}
