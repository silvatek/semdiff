//! Tests of reading or writing a semdif, including a document the command rejects.

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
fn rejects_an_import_move_whose_line_is_zero() {
    let tree = Tree::new();
    tree.write("edited/src/main.rs", "use old::path::Item;\n");
    let message = tree
        .apply(
            "edited",
            "\
header:
  semdiff: \"0.1.0\"
  timestamp: \"2020-01-01T00:00:00Z\"
changes:
  - scope: \"src/main.rs:0\"
    type: import-moved
    details:
      new: 1
      oldname: \"old::path::Item\"
      newname: \"new::place::Item\"
",
        )
        .unwrap_err();

    assert!(message.contains("path:line"), "{message}");
    assert_eq!(tree.read("edited/src/main.rs"), b"use old::path::Item;\n");
}

#[test]
fn rejects_an_import_move_that_is_missing_its_old_name() {
    let tree = Tree::new();
    tree.write("edited/src/main.rs", "use old::path::Item;\n");
    let message = tree
        .apply(
            "edited",
            "\
header:
  semdiff: \"0.1.0\"
  timestamp: \"2020-01-01T00:00:00Z\"
changes:
  - scope: \"src/main.rs:1\"
    type: import-moved
    details:
      new: 1
      newname: \"new::place::Item\"
",
        )
        .unwrap_err();

    assert!(message.contains("missing oldname"), "{message}");
    assert_eq!(tree.read("edited/src/main.rs"), b"use old::path::Item;\n");
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

/// The summary section, which sits between the header and the changes.
fn summary_of(document: &str) -> &str {
    let after_header = document
        .split_once("binary-encoding: \"base64\"\n")
        .expect("header")
        .1;
    after_header
        .split_once("changes:\n")
        .expect("changes")
        .0
}

#[test]
fn writes_a_summary_between_the_header_and_the_changes() {
    let tree = Tree::new();
    let before = "old line\n";
    let after = "He said \"hello\"\nNext line\n";
    tree.write("left/same.txt", "unchanged\n");
    tree.write("right/same.txt", "unchanged\n");
    tree.write("left/note.txt", before);
    tree.write("right/note.txt", after);
    tree.write("edited/note.txt", before);
    tree.write("left/drop.txt", "keep\ngone\n");
    tree.write("right/drop.txt", "keep\n");
    tree.write("edited/drop.txt", "keep\ngone\n");
    tree.write("left/data.bin", b"old");
    tree.write("right/data.bin", b"\x00\xff");
    tree.write("edited/data.bin", b"old");

    let document = tree.compare("left", "right");

    assert!(document.contains("type: line-added\n"), "{document}");
    assert!(document.contains("lines: +1\n"), "{document}");
    assert!(document.contains("type: line-removed\n"), "{document}");
    assert!(document.contains("lines: -1\n"), "{document}");
    assert!(document.contains("type: line-modified\n"), "{document}");
    assert!(document.contains("scope: \"data.bin\""), "{document}");
    assert!(!document.contains("same.txt"), "{document}");
    assert_eq!(
        summary_of(&document),
        "\
summary:
  changes: 7
  files: 3
  lines-added: 1
  lines-removed: 1
"
    );

    tree.apply("edited", &document).unwrap();
    assert_eq!(tree.read("edited/note.txt"), after.as_bytes());
    assert_eq!(tree.read("edited/drop.txt"), b"keep\n");
    assert_eq!(tree.read("edited/data.bin"), b"\x00\xff");
}

#[test]
fn counts_each_changed_file_and_leaves_a_directory_out() {
    let tree = Tree::new();
    let quote = "Brevity is the soul of wit.\n";
    tree.write("left/quote.txt", quote);
    tree.write("right/quotation.txt", quote);
    tree.write("left/notes.txt", "left only\n");
    tree.write("right/other.txt", "right only\n");
    tree.dir("right/vacant");

    let document = tree.compare("left", "right");

    assert!(document.contains("type: file-renamed\n"), "{document}");
    assert!(document.contains("type: file-added\n"), "{document}");
    assert!(document.contains("type: file-deleted\n"), "{document}");
    assert!(document.contains("type: directory-added\n"), "{document}");
    assert_eq!(
        summary_of(&document),
        "\
summary:
  changes: 4
  files: 3
  lines-added: 0
  lines-removed: 0
"
    );
}

#[test]
fn counts_a_moved_file_once_when_its_line_is_modified() {
    let tree = Tree::new();
    tree.write("left/old/note.txt", "a\nb\nc\nd\n");
    tree.write("right/new/note.txt", "a\nb\nc\ny\n");

    let document = tree.compare("left", "right");

    assert!(document.contains("type: file-moved\n"), "{document}");
    assert!(document.contains("type: line-modified\n"), "{document}");
    assert!(document.contains("type: directory-deleted\n"), "{document}");
    assert_eq!(
        summary_of(&document),
        "\
summary:
  changes: 4
  files: 1
  lines-added: 0
  lines-removed: 0
"
    );
}

#[test]
fn sums_the_lines_a_named_block_record_stores() {
    let tree = Tree::new();
    tree.write("left/src/types.rs", "impl Foo {\n}\n");
    tree.write(
        "right/src/types.rs",
        "impl Foo {\n    fn added() {}\n}\n",
    );
    tree.write("left/types.py", "class Foo:\n    x = 1\n");
    tree.write(
        "right/types.py",
        "class Foo:\n    x = 1\n    def added(self):\n        return 1\n",
    );
    tree.write("left/pair.py", "x = 0\ndef one():\n    return 1\n");
    tree.write("right/pair.py", "x = 0\n");

    let document = tree.compare("left", "right");

    assert!(
        document.contains("scope: \"src/types.rs[Foo]\"\n    type: named-block-added\n"),
        "{document}"
    );
    assert!(
        document.contains("scope: \"types.py[Foo]\"\n    type: named-block-added\n"),
        "{document}"
    );
    assert!(
        document.contains("type: named-block-removed\n"),
        "{document}"
    );
    assert_eq!(
        summary_of(&document),
        "\
summary:
  changes: 6
  files: 3
  lines-added: 3
  lines-removed: 2
"
    );
}
