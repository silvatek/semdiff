use super::tree::Tree;

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
fn replays_a_modification_a_removal_and_an_addition() {
    let tree = Tree::new();
    let before = "same\nold\ngone\ntail\n";
    let after = "same\nnew\ntail\nextra\n";
    tree.write("left/edit.txt", before);
    tree.write("right/edit.txt", after);
    tree.write("edited/edit.txt", before);

    let document = tree.compare("left", "right");

    assert_eq!(
        document.split_once("changes:\n").expect("changes list").1,
        "  - scope: \"edit.txt\"\n    type: file-changed\n    details:\n      old: \"38a0e71cdc417583a6f82094581552227c96aaa4dbb897edece3f69a71d31464\"\n      new: \"21f14a6d40bb2fcf5c54176224f1ae9cfb50dad1e6561d62c2cadff3bd848ca2\"\n  - scope: \"edit.txt\"\n    type: line-modified\n    details:\n      old: 2\n      lines: +0\n      new: 2\n      text: \"new\"\n  - scope: \"edit.txt\"\n    type: line-removed\n    details:\n      old: 3\n      lines: -1\n      new: 3\n      text: \"gone\"\n  - scope: \"edit.txt\"\n    type: line-added\n    details:\n      old: 5\n      lines: +1\n      new: 4\n      text: \"extra\"\n"
    );

    tree.apply("edited", &document).unwrap();
    assert_eq!(tree.read("edited/edit.txt"), after.as_bytes());
}

#[test]
fn merges_adjacent_removed_lines_into_one_span() {
    let tree = Tree::new();
    tree.write("left/span.txt", "keep\none\ntwo\nthree\ntail\n");
    tree.write("right/span.txt", "keep\ntail\n");
    tree.write("edited/span.txt", "keep\none\ntwo\nthree\ntail\n");
    tree.write("left/gap.txt", "keep\none\nmid\ntwo\ntail\n");
    tree.write("right/gap.txt", "keep\nmid\ntail\n");
    tree.write("edited/gap.txt", "keep\none\nmid\ntwo\ntail\n");

    let document = tree.compare("left", "right");

    assert!(document.contains(
        "  - scope: \"span.txt\"\n    type: line-removed\n    details:\n      old: 2\n      lines: -3\n      new: 2\n      text: \"one\\ntwo\\nthree\"\n"
    ));
    assert!(document.contains(
        "  - scope: \"gap.txt\"\n    type: line-removed\n    details:\n      old: 2\n      lines: -1\n      new: 2\n      text: \"one\"\n"
    ));
    assert!(document.contains(
        "  - scope: \"gap.txt\"\n    type: line-removed\n    details:\n      old: 4\n      lines: -1\n      new: 3\n      text: \"two\"\n"
    ));

    tree.apply("edited", &document).unwrap();
    assert_eq!(tree.read("edited/span.txt"), b"keep\ntail\n");
    assert_eq!(tree.read("edited/gap.txt"), b"keep\nmid\ntail\n");
}

#[test]
fn merges_adjacent_additions_and_modifications() {
    let tree = Tree::new();
    tree.write("left/added.txt", "keep\ntail\n");
    tree.write("right/added.txt", "keep\none\ntwo\nthree\ntail\n");
    tree.write("edited/added.txt", "keep\ntail\n");
    tree.write("left/changed.txt", "keep\nold1\nold2\ntail\n");
    tree.write("right/changed.txt", "keep\nnew1\nnew2\ntail\n");
    tree.write("edited/changed.txt", "keep\nold1\nold2\ntail\n");
    tree.write("left/split-added.txt", "keep\nmid\ntail\n");
    tree.write("right/split-added.txt", "keep\none\nmid\ntwo\ntail\n");
    tree.write("edited/split-added.txt", "keep\nmid\ntail\n");
    tree.write("left/split-changed.txt", "keep\nold1\nmid\nold2\ntail\n");
    tree.write("right/split-changed.txt", "keep\nnew1\nmid\nnew2\ntail\n");
    tree.write("edited/split-changed.txt", "keep\nold1\nmid\nold2\ntail\n");

    let document = tree.compare("left", "right");

    assert!(document.contains(
        "  - scope: \"added.txt\"\n    type: line-added\n    details:\n      old: 2\n      lines: +3\n      new: 2\n      text: \"one\\ntwo\\nthree\"\n"
    ));
    assert!(document.contains(
        "  - scope: \"changed.txt\"\n    type: line-modified\n    details:\n      old: 2\n      lines: +0\n      new: 2\n      text: \"new1\\nnew2\"\n"
    ));
    assert!(document.contains(
        "  - scope: \"split-added.txt\"\n    type: line-added\n    details:\n      old: 2\n      lines: +1\n      new: 2\n      text: \"one\"\n"
    ));
    assert!(document.contains(
        "  - scope: \"split-added.txt\"\n    type: line-added\n    details:\n      old: 3\n      lines: +1\n      new: 4\n      text: \"two\"\n"
    ));
    assert!(document.contains(
        "  - scope: \"split-changed.txt\"\n    type: line-modified\n    details:\n      old: 2\n      lines: +0\n      new: 2\n      text: \"new1\"\n"
    ));
    assert!(document.contains(
        "  - scope: \"split-changed.txt\"\n    type: line-modified\n    details:\n      old: 4\n      lines: +0\n      new: 4\n      text: \"new2\"\n"
    ));

    tree.apply("edited", &document).unwrap();
    assert_eq!(
        tree.read("edited/added.txt"),
        b"keep\none\ntwo\nthree\ntail\n"
    );
    assert_eq!(tree.read("edited/changed.txt"), b"keep\nnew1\nnew2\ntail\n");
    assert_eq!(
        tree.read("edited/split-added.txt"),
        b"keep\none\nmid\ntwo\ntail\n"
    );
    assert_eq!(
        tree.read("edited/split-changed.txt"),
        b"keep\nnew1\nmid\nnew2\ntail\n"
    );
}

#[test]
fn merges_the_adjacent_removals_in_the_rust_sample() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let sample = manifest.join("TestCode/rust/main");
    let document = semdiff::compare_sources(
        &semdiff::StdFileSystem,
        sample.join("v1").to_str().unwrap(),
        sample.join("v2").to_str().unwrap(),
    )
    .unwrap();

    assert!(document.contains(
        "  - scope: \"src/main.rs\"\n    type: line-removed\n    details:\n      old: 43\n      lines: -1\n      new: 44\n      text: \"// This single line is deleted.\"\n"
    ));
    assert!(document.contains(
        "  - scope: \"src/main.rs\"\n    type: line-removed\n    details:\n      old: 49\n      lines: -2\n      new: 49\n      text: \"// This deleted line is first.\\n// This deleted line is second.\"\n"
    ));
    assert!(document.contains(
        "  - scope: \"src/main.rs\"\n    type: line-added\n    details:\n      old: 18\n      lines: +1\n      new: 18\n      text: \"// Print Fibonacci numbers.\"\n"
    ));
    assert!(document.contains(
        "  - scope: \"src/main.rs\"\n    type: line-added\n    details:\n      old: 53\n      lines: +2\n      new: 51\n      text: \"// This added line is first.\\n// This added line is second.\"\n"
    ));
    assert!(document.contains(
        "  - scope: \"src/main.rs\"\n    type: identifier-renamed\n    details:\n      old: 1\n      new: 1\n      oldname: \"fibonacci\"\n      newname: \"fib\"\n"
    ));
    assert!(document.contains(
        "  - scope: \"src/main.rs\"\n    type: identifier-renamed\n    details:\n      old: 25\n      new: 26\n      oldname: \"fibonacci\"\n      newname: \"fib\"\n"
    ));
    assert!(document.contains(
        "  - scope: \"src/main.rs\"\n    type: identifier-renamed\n    details:\n      old: 31\n      new: 32\n      oldname: \"fibonacci\"\n      newname: \"fib\"\n"
    ));
    assert!(document.contains(
        "  - scope: \"src/main.rs\"\n    type: identifier-renamed\n    details:\n      old: 35\n      new: 36\n      oldname: \"fibonacci\"\n      newname: \"fib\"\n"
    ));
    assert!(document.contains(
        "  - scope: \"src/main.rs\"\n    type: identifier-renamed\n    details:\n      old: 36\n      new: 37\n      oldname: \"fibonacci\"\n      newname: \"fib\"\n"
    ));
    assert!(document.contains(
        "  - scope: \"src/main.rs\"\n    type: identifier-renamed\n    details:\n      old: 37\n      new: 38\n      oldname: \"fibonacci\"\n      newname: \"fib\"\n"
    ));
    assert!(document.contains(
        "  - scope: \"src/main.rs\"\n    type: identifier-renamed\n    details:\n      old: 38\n      new: 39\n      oldname: \"fibonacci\"\n      newname: \"fib\"\n"
    ));
    assert!(document.contains(
        "  - scope: \"src/main.rs\"\n    type: identifier-renamed\n    details:\n      old: 46\n      new: 46\n      oldname: \"zero\"\n      newname: \"one\"\n"
    ));
    assert!(document.contains(
        "  - scope: \"src/main.rs\"\n    type: identifier-renamed\n    details:\n      old: 55\n      new: 55\n      oldname: \"Old\"\n      newname: \"New\"\n"
    ));
    assert!(document.contains(
        "  - scope: \"src/main.rs\"\n    type: identifier-renamed\n    details:\n      old: 56\n      new: 56\n      oldname: \"Old\"\n      newname: \"New\"\n"
    ));
    assert!(!document.contains("old: 50\n"), "{document}");
    assert!(!document.contains("new: 52\n"), "{document}");
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
fn rejects_a_file_whose_old_hash_does_not_match() {
    let (tree, document) = changed_note();
    let document = document.replacen(
        "9e9894bfcaf8d63b9898e90db0fe05f5a0388d45bcf60a0a208ad31153072142",
        &"0".repeat(64),
        1,
    );
    let message = tree.apply("edited", &document).unwrap_err();
    assert!(message.contains("do not match the old hash"), "{message}");
    assert_eq!(tree.read("edited/note.txt"), b"old line\n");
}

#[test]
fn rejects_a_file_whose_new_hash_does_not_match() {
    let (tree, document) = changed_note();
    let document = document.replacen(
        "f77de5e87174686ce6fca0e3d03893b38dc6020febaa55578a8b20b289b5c362",
        &"f".repeat(64),
        1,
    );
    let message = tree.apply("edited", &document).unwrap_err();
    assert!(message.contains("do not match the new hash"), "{message}");
    assert_eq!(tree.read("edited/note.txt"), b"old line\n");
}

#[test]
fn records_an_identifier_rename_when_one_name_changes() {
    let tree = Tree::new();
    let before = "fn fibonacci(n: u32) {\n    let total = foo + bar;\n}\n";
    let after = "fn fib(n: u32) {\n    let total = fox + baz;\n}\n";
    let twice_before = "foo(foo)\n";
    let twice_after = "foo(bar)\n";
    let note_before = "hello\n";
    let note_after = "hallo\n";
    let python_before = "def fibonacci(n):\n    return n\n";
    let python_after = "def fib(n):\n    return n\n";
    tree.write("left/src/main.rs", before);
    tree.write("right/src/main.rs", after);
    tree.write("edited/src/main.rs", before);
    tree.write("left/src/twice.rs", twice_before);
    tree.write("right/src/twice.rs", twice_after);
    tree.write("edited/src/twice.rs", twice_before);
    tree.write("left/note.txt", note_before);
    tree.write("right/note.txt", note_after);
    tree.write("edited/note.txt", note_before);
    tree.write("left/calc.py", python_before);
    tree.write("right/calc.py", python_after);
    tree.write("edited/calc.py", python_before);

    let document = tree.compare("left", "right");

    assert!(document.contains(
        "  - scope: \"src/main.rs\"\n    type: identifier-renamed\n    details:\n      old: 1\n      new: 1\n      oldname: \"fibonacci\"\n      newname: \"fib\"\n"
    ));
    assert!(document.contains(
        "  - scope: \"src/main.rs\"\n    type: line-modified\n    details:\n      old: 2\n      lines: +0\n      new: 2\n      text: \"    let total = fox + baz;\"\n"
    ));
    assert!(document.contains(
        "  - scope: \"src/twice.rs\"\n    type: line-modified\n    details:\n      old: 1\n      lines: +0\n      new: 1\n      text: \"foo(bar)\"\n"
    ));
    assert!(!document.contains("scope: \"src/twice.rs\"\n    type: identifier-renamed"));
    assert!(document.contains(
        "  - scope: \"note.txt\"\n    type: line-modified\n    details:\n      old: 1\n      lines: +0\n      new: 1\n      text: \"hallo\"\n"
    ));
    assert!(document.contains(
        "  - scope: \"calc.py\"\n    type: identifier-renamed\n    details:\n      old: 1\n      new: 1\n      oldname: \"fibonacci\"\n      newname: \"fib\"\n"
    ));

    tree.apply("edited", &document).unwrap();
    assert_eq!(tree.read("edited/src/main.rs"), after.as_bytes());
    assert_eq!(tree.read("edited/src/twice.rs"), twice_after.as_bytes());
    assert_eq!(tree.read("edited/note.txt"), note_after.as_bytes());
    assert_eq!(tree.read("edited/calc.py"), python_after.as_bytes());
}

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
  - scope: \"src/main.rs\"
    type: identifier-renamed
    details:
      old: 1
      new: 1
      oldname: \"foo\"
      newname: \"bar\"
",
        )
        .unwrap_err();

    assert!(message.contains("does not occur once"), "{message}");
    assert_eq!(tree.read("edited/src/main.rs"), b"foo(foo)\n");
}

fn changed_note() -> (Tree, String) {
    let tree = Tree::new();
    tree.write("left/note.txt", "old line\n");
    tree.write("right/note.txt", "He said \"hello\"\nNext line\n");
    tree.write("edited/note.txt", "old line\n");
    let document = tree.compare("left", "right");
    (tree, document)
}
