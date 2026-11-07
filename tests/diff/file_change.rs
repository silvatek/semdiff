//! Identifying and classifying changes at the file level.

use crate::filesystem::Tree;

#[test]
fn writes_a_file_renamed_change() {
    let tree = Tree::new();
    let quote = "Brevity is the soul of wit.\n";
    tree.write("left/quote.txt", quote);
    tree.write("right/quotation.txt", quote);
    tree.write("left/notes.txt", "left only\n");
    tree.write("right/other.txt", "right only\n");

    let document = tree.compare("left", "right");

    let changes = document.split_once("changes:\n").expect("changes list").1;
    assert_eq!(
        changes,
        "  - scope: \".\"\n    type: file-deleted\n    details:\n      path: \"notes.txt\"\n      hash: \"7895f1404c181ccc997e89e37160d7b0b3861277cb31e918c73938d6dcf9a4fa\"\n  - scope: \".\"\n    type: file-added\n    details:\n      path: \"other.txt\"\n      hash: \"38018bc42d71ecc429f52ac65cefc8447db479ddb67eae66deb8d9fe270a497a\"\n      contents: \"right only\\n\"\n  - scope: \".\"\n    type: file-renamed\n    details:\n      from: \"quote.txt\"\n      to: \"quotation.txt\"\n      hash: \"d38c93c9206417950b787b96ff1d376cdb83fb6df5c46a58fbbeeaa4621c3c54\"\n"
    );
}

#[test]
fn hashes_past_the_first_buffer() {
    let tree = Tree::new();
    let mut same = vec![1_u8; 9_000];
    same[8_500] = 7;
    let shared_prefix = vec![2_u8; 9_000];
    let mut shared_prefix_changed = shared_prefix.clone();
    shared_prefix_changed[8_500] = 4;
    tree.write("left/same.bin", &same);
    tree.write("right/same-renamed.bin", &same);
    tree.write("left/left.bin", &shared_prefix);
    tree.write("right/right.bin", &shared_prefix_changed);

    let document = tree.compare("left", "right");

    let changes = document.split_once("changes:\n").expect("changes list").1;
    assert!(changes.contains(
        "  - scope: \".\"\n    type: file-deleted\n    details:\n      path: \"left.bin\"\n      hash: \"1b493ec1fbe9d38ea2a56abf0ed423562af579f59c109d552e045caa98336262\"\n"
    ));
    assert!(changes.contains(
        "  - scope: \".\"\n    type: file-renamed\n    details:\n      from: \"same.bin\"\n      to: \"same-renamed.bin\"\n      hash: \"b62232a52eb1d7300a109f54fdd5a0e6a3307eac382ce4f48a4503f1cbd2d7f3\"\n"
    ));
    let contents_line = document
        .lines()
        .find(|line| line.contains("\\u0002"))
        .expect("added file contents");
    assert!(contents_line.contains("\\u0002"));
    assert!(!contents_line
        .chars()
        .any(|character| (character as u32) < 0x20));
}

#[test]
fn adds_a_file_and_restores_contents_that_yaml_cannot_hold_literally() {
    let tree = Tree::new();
    let kept = "same\n";
    let tricky = "He said \"hello\"\tand wrote C:\\temp.\r\nNext: line\n  spaced\0\u{2028}";
    let binary = b"\x00\xff\"\\\n: ";
    tree.write("left/keep.txt", kept);
    tree.write("right/keep.txt", kept);
    tree.write("edited/keep.txt", kept);
    tree.write("right/tricky.txt", tricky);
    tree.write("right/empty.txt", "");
    tree.write("right/nested/data.bin", binary);

    let document = tree.compare("left", "right");

    let tricky_line = document
        .lines()
        .find(|line| line.contains("hello"))
        .expect("tricky contents");
    assert!(tricky_line.contains("\\\""));
    assert!(tricky_line.contains("\\\\"));
    assert!(tricky_line.contains("\\n"));
    assert!(tricky_line.contains("\\r"));
    assert!(tricky_line.contains("\\t"));
    assert!(tricky_line.contains("\\u0000"));
    assert!(tricky_line.contains("\\u2028"));
    assert!(!tricky_line.chars().any(|character| {
        character == '\n'
            || character == '\r'
            || character == '\t'
            || character == '\0'
            || character == '\u{2028}'
    }));
    assert!(document.contains("path: \"empty.txt\""));
    assert!(document.contains("contents: \"\""));
    assert!(document.contains("encoding: \"base64\""));
    assert!(document.contains("path: \"nested/data.bin\""));
    assert!(!document.as_bytes().contains(&0xff));

    tree.apply("edited", &document).unwrap();
    assert_eq!(tree.read("edited/keep.txt"), kept.as_bytes());
    assert_eq!(tree.read("edited/tricky.txt"), tricky.as_bytes());
    assert_eq!(tree.read("edited/empty.txt"), b"");
    assert_eq!(tree.read("edited/nested/data.bin"), binary);
}

#[test]
fn detects_a_rename_between_individual_files() {
    let tree = Tree::new();
    let quote = "Brevity is the soul of wit.\n";
    tree.write("left/quote.txt", quote);
    tree.write("right/quotation.txt", quote);

    let document = tree.compare("left/quote.txt", "right/quotation.txt");

    let changes = document.split_once("changes:\n").expect("changes list").1;
    assert_eq!(
        changes,
        "  - scope: \".\"\n    type: file-renamed\n    details:\n      from: \"quote.txt\"\n      to: \"quotation.txt\"\n      hash: \"d38c93c9206417950b787b96ff1d376cdb83fb6df5c46a58fbbeeaa4621c3c54\"\n"
    );
}

#[test]
fn rejects_a_deletion_whose_hash_does_not_match() {
    let tree = Tree::new();
    tree.write("source/notes.txt", "left only\n");
    let message = tree
        .apply(
            "source",
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
",
        )
        .unwrap_err();

    assert!(message.contains("do not match the hash"), "{message}");
    assert_eq!(tree.read("source/notes.txt"), b"left only\n");
}

#[test]
fn rejects_an_addition_whose_hash_does_not_match() {
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
    type: file-added
    details:
      path: \"other.txt\"
      hash: \"0000000000000000000000000000000000000000000000000000000000000000\"
      contents: \"right only\\n\"
",
        )
        .unwrap_err();

    assert!(message.contains("do not match the hash"), "{message}");
    assert!(!tree.exists("source/other.txt"));
}

#[test]
fn rejects_a_rename_whose_hash_does_not_match() {
    let tree = Tree::new();
    tree.write("source/quote.txt", "Brevity is the soul of wit.\n");
    let message = tree
        .apply(
            "source",
            "\
header:
  semdiff: \"0.1.0\"
  timestamp: \"2020-01-01T00:00:00Z\"
changes:
  - scope: \".\"
    type: file-renamed
    details:
      from: \"quote.txt\"
      to: \"quotation.txt\"
      hash: \"0000000000000000000000000000000000000000000000000000000000000000\"
",
        )
        .unwrap_err();

    assert!(message.contains("do not match the hash"), "{message}");
    assert_eq!(
        tree.read("source/quote.txt"),
        b"Brevity is the soul of wit.\n"
    );
    assert!(!tree.exists("source/quotation.txt"));
}

#[test]
fn rejects_a_tree_change_that_omits_its_hash() {
    let tree = Tree::new();
    tree.write("source/notes.txt", "left only\n");
    let message = tree
        .apply(
            "source",
            "\
header:
  semdiff: \"0.1.0\"
  timestamp: \"2020-01-01T00:00:00Z\"
changes:
  - scope: \".\"
    type: file-deleted
    details:
      path: \"notes.txt\"
",
        )
        .unwrap_err();

    assert_eq!(message, "file-deleted is missing hash");
    assert_eq!(tree.read("source/notes.txt"), b"left only\n");
}

#[test]
fn records_a_whole_file_when_only_the_final_newline_differs() {
    let tree = Tree::new();
    tree.write("left/note.txt", "hello");
    tree.write("right/note.txt", "hello\n");
    tree.write("edited/note.txt", "hello");

    let document = tree.compare("left", "right");

    assert!(document.contains(
        "  - scope: \"note.txt\"\n    type: file-changed\n    details:\n      old: \"2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824\"\n      new: \"5891b5b522d5df086d0ff0b110fbd9d21bb4fc7163af34d08286a2e846f6be03\"\n  - scope: \"note.txt\"\n    type: file-changed\n    details:\n      contents: \"hello\\n\"\n"
    ));

    tree.apply("edited", &document).unwrap();
    assert_eq!(tree.read("edited/note.txt"), b"hello\n");
}
