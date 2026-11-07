//! The record of a span-level change, including how such records combine and how
//! apply carries one out. A span is a changed sequence within a line.

use crate::filesystem::Tree;

#[test]
fn rejects_a_rename_when_the_old_name_does_not_occur_once() {
    let tree = Tree::new();
    tree.write("edited/src/main.rs", "foo(foo)\n");
    let message = tree
        .apply(
            "edited",
            "\
header:
  semdiff: \"0.1.0\"
  timestamp: \"2020-01-01T00:00:00Z\"
changes:
  - scope: \"src/main.rs:1\"
    type: identifier-renamed
    details:
      new: 1
      oldname: \"foo\"
      newname: \"bar\"
",
        )
        .unwrap_err();

    assert!(message.contains("does not occur once"), "{message}");
    assert_eq!(tree.read("edited/src/main.rs"), b"foo(foo)\n");
}

#[test]
fn merges_renames_of_one_name_across_files_in_one_directory() {
    let tree = Tree::new();
    tree.write("left/src/a.rs", "fn foo() {}\n");
    tree.write("right/src/a.rs", "fn bar() {}\n");
    tree.write("edited/src/a.rs", "fn foo() {}\n");
    tree.write("left/src/b.rs", "foo();\n");
    tree.write("right/src/b.rs", "bar();\n");
    tree.write("edited/src/b.rs", "foo();\n");
    tree.write("left/tests/c.rs", "fn foo() {}\n");
    tree.write("right/tests/c.rs", "fn foo() {}\n");
    tree.write("edited/tests/c.rs", "fn foo() {}\n");

    let document = tree.compare("left", "right");

    assert!(document.contains(
        "  - scope: \"src\"\n    type: identifier-renamed\n    details:\n      oldname: \"foo\"\n      newname: \"bar\"\n"
    ));
    assert!(!document.contains("scope: \"src/a.rs:"), "{document}");
    assert!(!document.contains("scope: \"src/b.rs:"), "{document}");
    assert!(!document.contains("scope: \"tests/c.rs\""), "{document}");

    tree.apply("edited", &document).unwrap();
    assert_eq!(tree.read("edited/src/a.rs"), b"fn bar() {}\n");
    assert_eq!(tree.read("edited/src/b.rs"), b"bar();\n");
    assert_eq!(tree.read("edited/tests/c.rs"), b"fn foo() {}\n");
}

#[test]
fn a_tree_rename_leaves_a_binary_file_to_its_own_record() {
    let tree = Tree::new();
    let binary = b"\x00\xff";
    tree.write("left/a.rs", "fn foo() {}\n");
    tree.write("right/a.rs", "fn bar() {}\n");
    tree.write("edited/a.rs", "fn foo() {}\n");
    tree.write("left/b.rs", "foo();\n");
    tree.write("right/b.rs", "bar();\n");
    tree.write("edited/b.rs", "foo();\n");
    tree.write("left/data.bin", b"old");
    tree.write("right/data.bin", binary);
    tree.write("edited/data.bin", b"old");

    let document = tree.compare("left", "right");

    assert!(document.contains(
        "  - scope: \".\"\n    type: identifier-renamed\n    details:\n      oldname: \"foo\"\n      newname: \"bar\"\n"
    ));
    assert!(document.contains("scope: \"data.bin\""));
    assert!(document.contains("encoding: \"base64\""));

    tree.apply("edited", &document).unwrap();
    assert_eq!(tree.read("edited/a.rs"), b"fn bar() {}\n");
    assert_eq!(tree.read("edited/b.rs"), b"bar();\n");
    assert_eq!(tree.read("edited/data.bin"), binary);
}

#[test]
fn keeps_file_renames_when_the_directory_still_has_the_name() {
    let tree = Tree::new();
    tree.write("left/src/a.rs", "fn foo() {}\n");
    tree.write("right/src/a.rs", "fn bar() {}\n");
    tree.write("edited/src/a.rs", "fn foo() {}\n");
    tree.write("left/src/b.rs", "foo();\n");
    tree.write("right/src/b.rs", "bar();\n");
    tree.write("edited/src/b.rs", "foo();\n");
    tree.write("left/src/c.rs", "fn foo() {}\n");
    tree.write("right/src/c.rs", "fn foo() {}\n");
    tree.write("edited/src/c.rs", "fn foo() {}\n");

    let document = tree.compare("left", "right");

    assert!(document.contains(
        "  - scope: \"src/a.rs:1\"\n    type: identifier-renamed\n    details:\n      new: 1\n      oldname: \"foo\"\n      newname: \"bar\"\n"
    ));
    assert!(document.contains(
        "  - scope: \"src/b.rs:1\"\n    type: identifier-renamed\n    details:\n      new: 1\n      oldname: \"foo\"\n      newname: \"bar\"\n"
    ));
    assert!(!document.contains("scope: \"src\""), "{document}");

    tree.apply("edited", &document).unwrap();
    assert_eq!(tree.read("edited/src/a.rs"), b"fn bar() {}\n");
    assert_eq!(tree.read("edited/src/b.rs"), b"bar();\n");
    assert_eq!(tree.read("edited/src/c.rs"), b"fn foo() {}\n");
}

#[test]
fn merges_renames_of_one_name_when_the_gap_does_not_keep_that_name() {
    let tree = Tree::new();
    let before = "foo one\nkeep\nfoo two\nstill foo\nfoo three\n";
    let after = "bar one\nkeep\nbar two\nstill foo\nbar three\n";
    tree.write("left/src/main.rs", before);
    tree.write("right/src/main.rs", after);
    tree.write("edited/src/main.rs", before);

    let document = tree.compare("left", "right");

    assert!(document.contains(
        "  - scope: \"src/main.rs:1-3\"\n    type: identifier-renamed\n    details:\n      oldname: \"foo\"\n      newname: \"bar\"\n"
    ));
    assert!(document.contains(
        "  - scope: \"src/main.rs:5\"\n    type: identifier-renamed\n    details:\n      new: 5\n      oldname: \"foo\"\n      newname: \"bar\"\n"
    ));
    assert!(!document.contains("scope: \"src/main.rs:1\""), "{document}");
    assert!(!document.contains("scope: \"src/main.rs:3\""), "{document}");

    tree.apply("edited", &document).unwrap();
    assert_eq!(tree.read("edited/src/main.rs"), after.as_bytes());
}
