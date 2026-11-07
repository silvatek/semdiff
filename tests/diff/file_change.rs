//! Tests of identifying and classifying changes at the file level.

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

#[test]
fn records_a_file_moved_when_one_name_keeps_most_lines_in_order() {
    let tree = Tree::new();
    let before = "a\nb\nc\nd\n";
    let after = "a\nb\nc\ny\n";
    tree.write("left/old/note.txt", before);
    tree.write("right/new/note.txt", after);
    tree.write("edited/old/note.txt", before);

    let document = tree.compare("left", "right");

    let changes = document.split_once("changes:\n").expect("changes list").1;
    assert_eq!(
        changes,
        "  - scope: \".\"\n    type: file-moved\n    details:\n      from: \"old/note.txt\"\n      to: \"new/note.txt\"\n      hash: \"cf2c7f63055d2e84af6e3f01ac1bb7fce598d20cf213fab2b56b8e8047b46ced\"\n  - scope: \"new/note.txt\"\n    type: file-changed\n    details:\n      old: \"cf2c7f63055d2e84af6e3f01ac1bb7fce598d20cf213fab2b56b8e8047b46ced\"\n      new: \"db9c74e32785df4e1904ce1dc35bbc5e18d3c8375e95fc8185e9e6d497c618c3\"\n  - scope: \"new/note.txt\"\n    type: line-modified\n    details:\n      old: 4\n      lines: +0\n      new: 4\n      text: \"y\"\n  - scope: \".\"\n    type: directory-deleted\n    details:\n      path: \"old\"\n"
    );

    tree.apply("edited", &document).unwrap();
    assert!(!tree.exists("edited/old"));
    assert_eq!(tree.read("edited/new/note.txt"), after.as_bytes());
}

#[test]
fn an_unchanged_file_with_the_same_name_stays_a_rename() {
    let tree = Tree::new();
    let note = "a\nb\nc\nd\n";
    tree.write("left/old/note.txt", note);
    tree.write("right/new/note.txt", note);

    let document = tree.compare("left", "right");

    assert!(document.contains("type: file-renamed\n"), "{document}");
    assert!(!document.contains("file-moved"), "{document}");
}

#[test]
fn half_the_lines_or_a_reordering_stays_a_deletion_and_an_addition() {
    let half = Tree::new();
    half.write("left/old/note.txt", "a\nb\nc\nd\n");
    half.write("right/new/note.txt", "a\nb\nx\ny\n");
    let half_document = half.compare("left", "right");
    assert!(
        half_document.contains("type: file-deleted\n"),
        "{half_document}"
    );
    assert!(
        half_document.contains("type: file-added\n"),
        "{half_document}"
    );
    assert!(!half_document.contains("file-moved"), "{half_document}");

    let reversed = Tree::new();
    reversed.write("left/old/note.txt", "a\nb\nc\nd\n");
    reversed.write("right/new/note.txt", "d\nc\nb\na\n");
    let reversed_document = reversed.compare("left", "right");
    assert!(
        reversed_document.contains("type: file-deleted\n"),
        "{reversed_document}"
    );
    assert!(
        reversed_document.contains("type: file-added\n"),
        "{reversed_document}"
    );
    assert!(
        !reversed_document.contains("file-moved"),
        "{reversed_document}"
    );
}

#[test]
fn several_files_with_one_name_or_a_different_name_are_not_a_move() {
    let many = Tree::new();
    let note = "a\nb\nc\nd\n";
    let edited = "a\nb\nc\ny\n";
    many.write("left/one/note.txt", note);
    many.write("left/two/note.txt", note);
    many.write("right/three/note.txt", edited);
    let many_document = many.compare("left", "right");
    assert!(
        many_document.contains("path: \"one/note.txt\""),
        "{many_document}"
    );
    assert!(
        many_document.contains("path: \"two/note.txt\""),
        "{many_document}"
    );
    assert!(
        many_document.contains("path: \"three/note.txt\""),
        "{many_document}"
    );
    assert!(!many_document.contains("file-moved"), "{many_document}");

    let renamed = Tree::new();
    renamed.write("left/old/note.txt", note);
    renamed.write("right/new/other.txt", edited);
    let renamed_document = renamed.compare("left", "right");
    assert!(
        renamed_document.contains("path: \"old/note.txt\""),
        "{renamed_document}"
    );
    assert!(
        renamed_document.contains("path: \"new/other.txt\""),
        "{renamed_document}"
    );
    assert!(
        !renamed_document.contains("file-moved"),
        "{renamed_document}"
    );
}

#[test]
fn a_binary_file_with_the_same_name_is_not_a_move() {
    let tree = Tree::new();
    tree.write("left/old/data.bin", b"\x00\x01\x02\xff");
    tree.write("right/new/data.bin", b"\x00\x01\x02\xfe");

    let document = tree.compare("left", "right");

    assert!(document.contains("type: file-deleted\n"), "{document}");
    assert!(document.contains("type: file-added\n"), "{document}");
    assert!(document.contains("encoding: \"base64\""), "{document}");
    assert!(!document.contains("file-moved"), "{document}");
}

#[test]
fn rejects_a_move_whose_old_hash_does_not_match() {
    let tree = Tree::new();
    tree.write("source/old/note.txt", "a\nb\nc\nd\n");
    let message = tree
        .apply(
            "source",
            "\
header:
  semdiff: \"0.1.0\"
  timestamp: \"2020-01-01T00:00:00Z\"
changes:
  - scope: \".\"
    type: file-moved
    details:
      from: \"old/note.txt\"
      to: \"new/note.txt\"
      hash: \"0000000000000000000000000000000000000000000000000000000000000000\"
",
        )
        .unwrap_err();

    assert!(message.contains("do not match the hash"), "{message}");
    assert_eq!(tree.read("source/old/note.txt"), b"a\nb\nc\nd\n");
    assert!(!tree.exists("source/new/note.txt"));
}

#[test]
fn rejects_a_moved_files_new_hash_when_the_edit_is_written() {
    let tree = Tree::new();
    tree.write("source/old/note.txt", "a\nb\nc\nd\n");
    let message = tree
        .apply(
            "source",
            "\
header:
  semdiff: \"0.1.0\"
  timestamp: \"2020-01-01T00:00:00Z\"
changes:
  - scope: \".\"
    type: file-moved
    details:
      from: \"old/note.txt\"
      to: \"new/note.txt\"
      hash: \"cf2c7f63055d2e84af6e3f01ac1bb7fce598d20cf213fab2b56b8e8047b46ced\"
  - scope: \"new/note.txt\"
    type: file-changed
    details:
      old: \"cf2c7f63055d2e84af6e3f01ac1bb7fce598d20cf213fab2b56b8e8047b46ced\"
      new: \"0000000000000000000000000000000000000000000000000000000000000000\"
  - scope: \"new/note.txt\"
    type: line-modified
    details:
      old: 4
      lines: +0
      new: 4
      text: \"y\"
",
        )
        .unwrap_err();

    assert!(message.contains("do not match the new hash"), "{message}");
    assert!(!tree.exists("source/old/note.txt"));
    assert_eq!(tree.read("source/new/note.txt"), b"a\nb\nc\nd\n");
}

#[test]
fn rejects_a_move_that_omits_its_hash() {
    let tree = Tree::new();
    tree.write("source/old/note.txt", "a\nb\nc\nd\n");
    let message = tree
        .apply(
            "source",
            "\
header:
  semdiff: \"0.1.0\"
  timestamp: \"2020-01-01T00:00:00Z\"
changes:
  - scope: \".\"
    type: file-moved
    details:
      from: \"old/note.txt\"
      to: \"new/note.txt\"
",
        )
        .unwrap_err();

    assert_eq!(message, "file-moved is missing hash");
    assert_eq!(tree.read("source/old/note.txt"), b"a\nb\nc\nd\n");
    assert!(!tree.exists("source/new/note.txt"));
}

#[test]
fn records_a_method_removed_from_a_moved_file() {
    let tree = Tree::new();
    let before = "impl Foo {\n    fn dropped() {}\n}\n";
    let after = "impl Foo {\n}\n";
    tree.write("left/old/types.rs", before);
    tree.write("right/new/types.rs", after);
    tree.write("edited/old/types.rs", before);

    let document = tree.compare("left", "right");

    assert!(document.contains(
        "  - scope: \"new/types.rs[Foo]\"\n    type: named-block-removed\n    details:\n      old: 2\n      lines: -1\n      new: 2\n      text: \"    fn dropped() {}\"\n      category: \"method\"\n      name: \"dropped\"\n"
    ), "{document}");
    assert!(!document.contains("scope: \"old/types.rs"), "{document}");

    tree.apply("edited", &document).unwrap();
    assert!(!tree.exists("edited/old"));
    assert_eq!(tree.read("edited/new/types.rs"), after.as_bytes());
}

#[test]
fn joins_identifier_renames_across_moved_files() {
    let tree = Tree::new();
    let before = "fn foo() {}\nfn kept() {}\nfn also() {}\n";
    let after = "fn bar() {}\nfn kept() {}\nfn also() {}\n";
    tree.write("left/old/a.rs", before);
    tree.write("left/old/b.rs", before);
    tree.write("right/new/a.rs", after);
    tree.write("right/other/b.rs", after);
    tree.write("edited/old/a.rs", before);
    tree.write("edited/old/b.rs", before);

    let document = tree.compare("left", "right");

    assert!(document.contains(
        "  - scope: \".\"\n    type: identifier-renamed\n    details:\n      oldname: \"foo\"\n      newname: \"bar\"\n"
    ), "{document}");
    assert!(!document.contains("scope: \"new/a.rs:"), "{document}");
    assert!(!document.contains("scope: \"other/b.rs:"), "{document}");
    assert!(!document.contains("scope: \"old/"), "{document}");

    tree.apply("edited", &document).unwrap();
    assert_eq!(tree.read("edited/new/a.rs"), after.as_bytes());
    assert_eq!(tree.read("edited/other/b.rs"), after.as_bytes());
    assert!(!tree.exists("edited/old"));
}

#[test]
fn records_a_whole_file_when_a_move_only_changes_the_final_newline() {
    let tree = Tree::new();
    tree.write("left/old/note.txt", "a\nb\nc\nd");
    tree.write("right/new/note.txt", "a\nb\nc\nd\n");
    tree.write("edited/old/note.txt", "a\nb\nc\nd");

    let document = tree.compare("left", "right");

    assert!(document.contains("type: file-moved\n"), "{document}");
    assert!(
        document.contains("scope: \"new/note.txt\"\n    type: file-changed\n    details:\n      contents: \"a\\nb\\nc\\nd\\n\"\n"),
        "{document}"
    );
    assert!(!document.contains("line-modified"), "{document}");

    tree.apply("edited", &document).unwrap();
    assert_eq!(tree.read("edited/new/note.txt"), b"a\nb\nc\nd\n");
    assert!(!tree.exists("edited/old"));
}
