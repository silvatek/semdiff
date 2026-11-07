//! Tests of identifying and classifying changes at the line level.

use crate::filesystem::Tree;
use std::path::Path;

#[test]
fn records_line_changes_for_a_pair_whose_contents_differ() {
    let tree = Tree::new();
    let same = "unchanged\n";
    let before = "old line\n";
    let after = "He said \"hello\"\nNext line\n";
    let removed = "keep\ngone\n";
    let kept = "keep\n";
    let binary = b"\x00\xff";
    tree.write("left/same.txt", same);
    tree.write("right/same.txt", same);
    tree.write("edited/same.txt", same);
    tree.write("left/note.txt", before);
    tree.write("right/note.txt", after);
    tree.write("edited/note.txt", before);
    tree.write("left/drop.txt", removed);
    tree.write("right/drop.txt", kept);
    tree.write("edited/drop.txt", removed);
    tree.write("left/data.bin", b"old");
    tree.write("right/data.bin", binary);
    tree.write("edited/data.bin", b"old");

    let document = tree.compare("left", "right");

    assert!(!document.contains("same.txt"), "{document}");
    assert!(document.contains(
        "  - scope: \"drop.txt\"\n    type: file-changed\n    details:\n      old: \"3282251a9b2bf054b863c3712910d292ff2f0c57d0fb16bb43c50157b20cce56\"\n      new: \"f660a7996deacfbc7560e4240054a8ad82eb02fe25a95064257e07084bcacb85\"\n  - scope: \"drop.txt\"\n    type: line-removed\n    details:\n      old: 2\n      lines: -1\n      new: 2\n      text: \"gone\"\n"
    ));
    assert!(document.contains(
        "  - scope: \"note.txt\"\n    type: file-changed\n    details:\n      old: \"9e9894bfcaf8d63b9898e90db0fe05f5a0388d45bcf60a0a208ad31153072142\"\n      new: \"f77de5e87174686ce6fca0e3d03893b38dc6020febaa55578a8b20b289b5c362\"\n"
    ));
    assert!(document.contains(
        "  - scope: \"data.bin\"\n    type: file-changed\n    details:\n      old: \"cba06b5736faf67e54b07b561eae94395e774c517a7d910a54369e1263ccfbd4\"\n      new: \"06eb7d6a69ee19e5fbdf749018d3d2abfa04bcbd1365db312eb86dc7169389b8\"\n"
    ));
    assert!(document.contains(
        "  - scope: \"note.txt\"\n    type: line-modified\n    details:\n      old: 1\n      lines: +0\n      new: 1\n      text: \"He said \\\"hello\\\"\"\n  - scope: \"note.txt\"\n    type: line-added\n    details:\n      old: 2\n      lines: +1\n      new: 2\n      text: \"Next line\"\n"
    ));
    assert!(document.contains("scope: \"data.bin\""));
    assert!(document.contains("encoding: \"base64\""));
    assert!(document.contains("type: file-changed"));
    assert!(!document.as_bytes().contains(&0xff));

    tree.apply("edited", &document).unwrap();
    assert_eq!(tree.read("edited/same.txt"), same.as_bytes());
    assert_eq!(tree.read("edited/note.txt"), after.as_bytes());
    assert_eq!(tree.read("edited/drop.txt"), kept.as_bytes());
    assert_eq!(tree.read("edited/data.bin"), binary);
}

#[test]
fn flags_whitespace_in_the_samples() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let rust = semdiff::compare_sources(
        &semdiff::StdFileSystem::new(),
        manifest.join("TestCode/rust/main/v1").to_str().unwrap(),
        manifest.join("TestCode/rust/main/v2").to_str().unwrap(),
    )
    .unwrap();
    let python = semdiff::compare_sources(
        &semdiff::StdFileSystem::new(),
        manifest.join("TestCode/python/main/v1").to_str().unwrap(),
        manifest.join("TestCode/python/main/v2").to_str().unwrap(),
    )
    .unwrap();

    assert!(rust.contains(
        "  - scope: \"src/main.rs\"\n    type: line-modified\n    details:\n      old: 63\n      lines: +0\n      new: 63\n      text: \"    let value = 1;\"\n      isWhitespace: true\n"
    ), "{rust}");
    assert!(rust.contains(
        "  - scope: \"src/main.rs\"\n    type: line-modified\n    details:\n      old: 68\n      lines: +0\n      new: 68\n      text: \"    let trailing = 1; \"\n      isWhitespace: true\n"
    ), "{rust}");
    assert!(rust.contains(
        "  - scope: \"src/main.rs\"\n    type: line-added\n    details:\n      old: 72\n      lines: +1\n      new: 72\n      text: \"\"\n      isWhitespace: true\n"
    ), "{rust}");
    assert!(
        !follows_with_whitespace(
            &python,
            "  - scope: \"src/main.py\"\n    type: line-modified\n    details:\n      old: 61\n      lines: +0\n      new: 61\n      text: \"        x = 1\"\n"
        ),
        "{python}"
    );
    assert!(python.contains(
        "  - scope: \"src/main.py\"\n    type: line-modified\n    details:\n      old: 64\n      lines: +0\n      new: 64\n      text: \"trailing = 1 \"\n      isWhitespace: true\n"
    ), "{python}");
    assert!(python.contains(
        "  - scope: \"src/main.py\"\n    type: line-modified\n    details:\n      old: 67\n      lines: +0\n      new: 67\n      text: \"    # margin\"\n      isComment: true\n      isWhitespace: true\n"
    ), "{python}");
    assert!(python.contains(
        "  - scope: \"src/main.py\"\n    type: line-added\n    details:\n      old: 70\n      lines: +1\n      new: 70\n      text: \"\"\n      isWhitespace: true\n"
    ), "{python}");
}

#[test]
fn flags_a_blank_line_for_a_known_dialect() {
    let tree = Tree::new();
    let blank_before = "fn main() {\n    \n    let value = 1;\n}\n";
    let blank_after = "fn main() {\n\t\n    let value = 1;\n}\n";
    let gap_before = "fn main() {\n}\n";
    let gap_after = "fn main() {\n\n    \n}\n";
    let add_before = "fn main() {\n}\n";
    let add_after = "fn main() {\n\n    let value = 1;\n}\n";
    let fill_before = "fn main() {\n    \n}\n";
    let fill_after = "fn main() {\n    let value = 1;\n}\n";
    let indent_before = "fn main() {\nlet value = 1;\n}\n";
    let indent_after = "fn main() {\n    let value = 1;\n}\n";
    let python_before = "def main():\n\n    \nx = 1\n";
    let python_after = "def main():\nx = 1\n";
    let python_indent_before = "def main():\n    x = 1\n";
    let python_indent_after = "def main():\n        x = 1\n";
    let note_before = "hello\n";
    let note_after = "hello\n\n";
    tree.write("left/src/blank.rs", blank_before);
    tree.write("right/src/blank.rs", blank_after);
    tree.write("edited/src/blank.rs", blank_before);
    tree.write("left/src/gap.rs", gap_before);
    tree.write("right/src/gap.rs", gap_after);
    tree.write("edited/src/gap.rs", gap_before);
    tree.write("left/src/add.rs", add_before);
    tree.write("right/src/add.rs", add_after);
    tree.write("edited/src/add.rs", add_before);
    tree.write("left/src/fill.rs", fill_before);
    tree.write("right/src/fill.rs", fill_after);
    tree.write("edited/src/fill.rs", fill_before);
    tree.write("left/src/indent.rs", indent_before);
    tree.write("right/src/indent.rs", indent_after);
    tree.write("edited/src/indent.rs", indent_before);
    tree.write("left/calc.py", python_before);
    tree.write("right/calc.py", python_after);
    tree.write("edited/calc.py", python_before);
    tree.write("left/indent.py", python_indent_before);
    tree.write("right/indent.py", python_indent_after);
    tree.write("edited/indent.py", python_indent_before);
    tree.write("left/notes.txt", note_before);
    tree.write("right/notes.txt", note_after);
    tree.write("edited/notes.txt", note_before);

    let document = tree.compare("left", "right");

    assert!(document.contains(
        "  - scope: \"src/blank.rs\"\n    type: line-modified\n    details:\n      old: 2\n      lines: +0\n      new: 2\n      text: \"\\t\"\n      isWhitespace: true\n"
    ), "{document}");
    assert!(document.contains(
        "  - scope: \"src/gap.rs\"\n    type: line-added\n    details:\n      old: 2\n      lines: +2\n      new: 2\n      text: \"\\n    \"\n      isWhitespace: true\n"
    ), "{document}");
    assert!(document.contains(
        "  - scope: \"src/add.rs\"\n    type: line-added\n    details:\n      old: 2\n      lines: +1\n      new: 2\n      text: \"\"\n      isWhitespace: true\n"
    ), "{document}");
    assert!(
        !follows_with_whitespace(
            &document,
            "  - scope: \"src/add.rs\"\n    type: line-added\n    details:\n      old: 2\n      lines: +1\n      new: 3\n      text: \"    let value = 1;\"\n"
        ),
        "{document}"
    );
    assert!(
        !follows_with_whitespace(
            &document,
            "  - scope: \"src/fill.rs\"\n    type: line-modified\n    details:\n      old: 2\n      lines: +0\n      new: 2\n      text: \"    let value = 1;\"\n"
        ),
        "{document}"
    );
    assert!(document.contains(
        "  - scope: \"src/indent.rs\"\n    type: line-modified\n    details:\n      old: 2\n      lines: +0\n      new: 2\n      text: \"    let value = 1;\"\n      isWhitespace: true\n"
    ), "{document}");
    assert!(document.contains(
        "  - scope: \"calc.py\"\n    type: line-removed\n    details:\n      old: 2\n      lines: -2\n      new: 2\n      text: \"\\n    \"\n      isWhitespace: true\n"
    ), "{document}");
    assert!(
        !follows_with_whitespace(
            &document,
            "  - scope: \"indent.py\"\n    type: line-modified\n    details:\n      old: 2\n      lines: +0\n      new: 2\n      text: \"        x = 1\"\n"
        ),
        "{document}"
    );
    assert!(document.contains(
        "  - scope: \"notes.txt\"\n    type: line-added\n    details:\n      old: 2\n      lines: +1\n      new: 2\n      text: \"\"\n"
    ), "{document}");
    assert!(!document.contains("scope: \"notes.txt\"\n    type: line-added\n    details:\n      old: 2\n      lines: +1\n      new: 2\n      text: \"\"\n      isWhitespace:"), "{document}");

    tree.apply("edited", &document).unwrap();
    assert_eq!(tree.read("edited/src/blank.rs"), blank_after.as_bytes());
    assert_eq!(tree.read("edited/src/gap.rs"), gap_after.as_bytes());
    assert_eq!(tree.read("edited/src/add.rs"), add_after.as_bytes());
    assert_eq!(tree.read("edited/src/fill.rs"), fill_after.as_bytes());
    assert_eq!(tree.read("edited/src/indent.rs"), indent_after.as_bytes());
    assert_eq!(tree.read("edited/calc.py"), python_after.as_bytes());
    assert_eq!(
        tree.read("edited/indent.py"),
        python_indent_after.as_bytes()
    );
    assert_eq!(tree.read("edited/notes.txt"), note_after.as_bytes());
}

#[test]
fn flags_leading_and_trailing_space_from_the_dialect() {
    let tree = Tree::new();
    let rust_indent_before = "fn main() {\nlet a = 1;\nlet b = 1;\n}\n";
    let rust_indent_after = "fn main() {\n    let a = 1;\n    let b = 1;\n}\n";
    let rust_mixed_before = "fn main() {\nlet a = 1;\nlet b = 1;\n}\n";
    let rust_mixed_after = "fn main() {\n    let a = 1;\nlet b = 2;\n}\n";
    let rust_trail_before = "fn main() {\nlet value = 1;\n}\n";
    let rust_trail_after = "fn main() {\nlet value = 1; \n}\n";
    let rust_comment_before = "fn main() {}\n// note\n";
    let rust_comment_after = "fn main() {}\n    // note\n";
    let python_trail_before = "x = 1\n";
    let python_trail_after = "x = 1 \n";
    let python_comment_before = "# note\nx = 1\n";
    let python_comment_after = "    # note\nx = 1\n";
    let inside_before = "let value = 1;\n";
    let inside_after = "let  value = 1;\n";
    let note_before = "hello\n";
    let note_after = "hello \n";
    tree.write("left/src/indent.rs", rust_indent_before);
    tree.write("right/src/indent.rs", rust_indent_after);
    tree.write("edited/src/indent.rs", rust_indent_before);
    tree.write("left/src/mixed.rs", rust_mixed_before);
    tree.write("right/src/mixed.rs", rust_mixed_after);
    tree.write("edited/src/mixed.rs", rust_mixed_before);
    tree.write("left/src/trail.rs", rust_trail_before);
    tree.write("right/src/trail.rs", rust_trail_after);
    tree.write("edited/src/trail.rs", rust_trail_before);
    tree.write("left/src/comment.rs", rust_comment_before);
    tree.write("right/src/comment.rs", rust_comment_after);
    tree.write("edited/src/comment.rs", rust_comment_before);
    tree.write("left/trail.py", python_trail_before);
    tree.write("right/trail.py", python_trail_after);
    tree.write("edited/trail.py", python_trail_before);
    tree.write("left/comment.py", python_comment_before);
    tree.write("right/comment.py", python_comment_after);
    tree.write("edited/comment.py", python_comment_before);
    tree.write("left/src/inside.rs", inside_before);
    tree.write("right/src/inside.rs", inside_after);
    tree.write("edited/src/inside.rs", inside_before);
    tree.write("left/notes.txt", note_before);
    tree.write("right/notes.txt", note_after);
    tree.write("edited/notes.txt", note_before);

    let document = tree.compare("left", "right");

    assert!(document.contains(
        "  - scope: \"src/indent.rs\"\n    type: line-modified\n    details:\n      old: 2\n      lines: +0\n      new: 2\n      text: \"    let a = 1;\\n    let b = 1;\"\n      isWhitespace: true\n"
    ), "{document}");
    assert!(document.contains(
        "  - scope: \"src/mixed.rs\"\n    type: line-modified\n    details:\n      old: 2\n      lines: +0\n      new: 2\n      text: \"    let a = 1;\"\n      isWhitespace: true\n"
    ), "{document}");
    assert!(document.contains(
        "  - scope: \"src/mixed.rs:3\"\n    type: constant-changed\n    details:\n      new: 3\n      oldvalue: \"1\"\n      newvalue: \"2\"\n      occurrence: 1\n"
    ), "{document}");
    assert!(document.contains(
        "  - scope: \"src/trail.rs\"\n    type: line-modified\n    details:\n      old: 2\n      lines: +0\n      new: 2\n      text: \"let value = 1; \"\n      isWhitespace: true\n"
    ), "{document}");
    assert!(document.contains(
        "  - scope: \"src/comment.rs\"\n    type: line-modified\n    details:\n      old: 2\n      lines: +0\n      new: 2\n      text: \"    // note\"\n      isComment: true\n      isWhitespace: true\n"
    ), "{document}");
    assert!(document.contains(
        "  - scope: \"trail.py\"\n    type: line-modified\n    details:\n      old: 1\n      lines: +0\n      new: 1\n      text: \"x = 1 \"\n      isWhitespace: true\n"
    ), "{document}");
    assert!(document.contains(
        "  - scope: \"comment.py\"\n    type: line-modified\n    details:\n      old: 1\n      lines: +0\n      new: 1\n      text: \"    # note\"\n      isComment: true\n      isWhitespace: true\n"
    ), "{document}");
    assert!(
        !follows_with_whitespace(
            &document,
            "  - scope: \"src/inside.rs\"\n    type: line-modified\n    details:\n      old: 1\n      lines: +0\n      new: 1\n      text: \"let  value = 1;\"\n"
        ),
        "{document}"
    );
    assert!(
        !follows_with_whitespace(
            &document,
            "  - scope: \"notes.txt\"\n    type: line-modified\n    details:\n      old: 1\n      lines: +0\n      new: 1\n      text: \"hello \"\n"
        ),
        "{document}"
    );

    tree.apply("edited", &document).unwrap();
    assert_eq!(
        tree.read("edited/src/indent.rs"),
        rust_indent_after.as_bytes()
    );
    assert_eq!(
        tree.read("edited/src/mixed.rs"),
        rust_mixed_after.as_bytes()
    );
    assert_eq!(
        tree.read("edited/src/trail.rs"),
        rust_trail_after.as_bytes()
    );
    assert_eq!(
        tree.read("edited/src/comment.rs"),
        rust_comment_after.as_bytes()
    );
    assert_eq!(tree.read("edited/trail.py"), python_trail_after.as_bytes());
    assert_eq!(
        tree.read("edited/comment.py"),
        python_comment_after.as_bytes()
    );
    assert_eq!(tree.read("edited/src/inside.rs"), inside_after.as_bytes());
    assert_eq!(tree.read("edited/notes.txt"), note_after.as_bytes());
}

#[test]
fn flags_a_line_that_opens_with_the_dialect_line_comment() {
    let tree = Tree::new();
    let rust_before = "fn main() {\n    let kept = 1;\n}\n// gone\n";
    let rust_after = "// header\nfn main() {\n    // note\n    let kept = 1;\n    /* block */\n}\n";
    let attr_before = "fn main() {}\n";
    let attr_after = "#[allow(dead_code)]\nfn main() {}\n";
    let python_before = "x = 1\n";
    let python_after = "# note\nx = 1\n";
    let note_before = "hello\n";
    let note_after = "// note\nhello\n";
    tree.write("left/src/main.rs", rust_before);
    tree.write("right/src/main.rs", rust_after);
    tree.write("edited/src/main.rs", rust_before);
    tree.write("left/src/attr.rs", attr_before);
    tree.write("right/src/attr.rs", attr_after);
    tree.write("edited/src/attr.rs", attr_before);
    tree.write("left/calc.py", python_before);
    tree.write("right/calc.py", python_after);
    tree.write("edited/calc.py", python_before);
    tree.write("left/notes.txt", note_before);
    tree.write("right/notes.txt", note_after);
    tree.write("edited/notes.txt", note_before);

    let document = tree.compare("left", "right");

    assert!(document.contains(
        "  - scope: \"src/main.rs\"\n    type: line-added\n    details:\n      old: 1\n      lines: +1\n      new: 1\n      text: \"// header\"\n      isComment: true\n"
    ));
    assert!(document.contains(
        "  - scope: \"src/main.rs\"\n    type: line-added\n    details:\n      old: 2\n      lines: +1\n      new: 3\n      text: \"    // note\"\n      isComment: true\n"
    ));
    assert!(document.contains(
        "  - scope: \"src/main.rs\"\n    type: line-removed\n    details:\n      old: 4\n      lines: -1\n      new: 7\n      text: \"// gone\"\n      isComment: true\n"
    ));
    assert!(document.contains(
        "  - scope: \"src/main.rs\"\n    type: line-added\n    details:\n      old: 3\n      lines: +1\n      new: 5\n      text: \"    /* block */\"\n"
    ));
    assert!(!document.contains("/* block */\"\n      isComment:"));
    assert!(document.contains(
        "  - scope: \"src/attr.rs\"\n    type: line-added\n    details:\n      old: 1\n      lines: +1\n      new: 1\n      text: \"#[allow(dead_code)]\"\n"
    ));
    assert!(!document.contains("#[allow(dead_code)]\"\n      isComment:"));
    assert!(document.contains(
        "  - scope: \"calc.py\"\n    type: line-added\n    details:\n      old: 1\n      lines: +1\n      new: 1\n      text: \"# note\"\n      isComment: true\n"
    ));
    assert!(document.contains(
        "  - scope: \"notes.txt\"\n    type: line-added\n    details:\n      old: 1\n      lines: +1\n      new: 1\n      text: \"// note\"\n"
    ));
    assert!(!document.contains("scope: \"notes.txt\"\n    type: line-added\n    details:\n      old: 1\n      lines: +1\n      new: 1\n      text: \"// note\"\n      isComment:"));

    tree.apply("edited", &document).unwrap();
    assert_eq!(tree.read("edited/src/main.rs"), rust_after.as_bytes());
    assert_eq!(tree.read("edited/src/attr.rs"), attr_after.as_bytes());
    assert_eq!(tree.read("edited/calc.py"), python_after.as_bytes());
    assert_eq!(tree.read("edited/notes.txt"), note_after.as_bytes());
}

#[test]
fn flags_a_modification_when_both_lines_are_comments() {
    let tree = Tree::new();
    let before = "// keep this\nunchanged\nlet value = 1;\n// zero\n";
    let after = "// keep this note\nunchanged\n// let value = 1;\n// one\n";
    tree.write("left/src/edit.rs", before);
    tree.write("right/src/edit.rs", after);
    tree.write("edited/src/edit.rs", before);

    let document = tree.compare("left", "right");

    assert!(document.contains(
        "  - scope: \"src/edit.rs\"\n    type: line-modified\n    details:\n      old: 1\n      lines: +0\n      new: 1\n      text: \"// keep this note\"\n      isComment: true\n"
    ));
    assert!(document.contains(
        "  - scope: \"src/edit.rs\"\n    type: line-modified\n    details:\n      old: 3\n      lines: +0\n      new: 3\n      text: \"// let value = 1;\"\n"
    ));
    assert!(!document.contains("let value = 1;\"\n      isComment:"));
    assert!(document.contains(
        "  - scope: \"src/edit.rs:4\"\n    type: identifier-renamed\n    details:\n      new: 4\n      oldname: \"zero\"\n      newname: \"one\"\n"
    ));
    assert!(!document.contains("oldname: \"zero\"\n      newname: \"one\"\n      isComment:"));

    tree.apply("edited", &document).unwrap();
    assert_eq!(tree.read("edited/src/edit.rs"), after.as_bytes());
}

/// Whether `record` is present and its next detail is `isWhitespace`.
fn follows_with_whitespace(document: &str, record: &str) -> bool {
    let Some((_, rest)) = document.split_once(record) else {
        panic!("missing record in:\n{document}");
    };
    rest.starts_with("      isWhitespace:")
}
