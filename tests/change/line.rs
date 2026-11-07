//! The record of a line-level change, including how such records combine and how
//! apply carries one out.

use crate::filesystem::Tree;
use std::path::Path;

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
        &semdiff::StdFileSystem::new(),
        sample.join("v1").to_str().unwrap(),
        sample.join("v2").to_str().unwrap(),
    )
    .unwrap();

    assert!(document.contains(
        "  - scope: \"src/main.rs\"\n    type: line-removed\n    details:\n      old: 43\n      lines: -1\n      new: 44\n      text: \"// This single line is deleted.\"\n      isComment: true\n"
    ));
    assert!(document.contains(
        "  - scope: \"src/main.rs\"\n    type: line-removed\n    details:\n      old: 49\n      lines: -2\n      new: 49\n      text: \"// This deleted line is first.\\n// This deleted line is second.\"\n      isComment: true\n"
    ));
    assert!(document.contains(
        "  - scope: \"src/main.rs\"\n    type: line-added\n    details:\n      old: 18\n      lines: +1\n      new: 18\n      text: \"// Print Fibonacci numbers.\"\n      isComment: true\n"
    ));
    assert!(document.contains(
        "  - scope: \"src/main.rs\"\n    type: line-added\n    details:\n      old: 53\n      lines: +2\n      new: 51\n      text: \"// This added line is first.\\n// This added line is second.\"\n      isComment: true\n"
    ));
    assert!(document.contains(
        "  - scope: \"src\"\n    type: identifier-renamed\n    details:\n      oldname: \"fibonacci\"\n      newname: \"fib\"\n"
    ));
    assert!(
        !document.contains("scope: \"src/main.rs:1-38\""),
        "{document}"
    );
    assert!(!document.contains("scope: \"src/report.rs:"), "{document}");
    assert!(document.contains(
        "  - scope: \"src/main.rs:46\"\n    type: identifier-renamed\n    details:\n      new: 46\n      oldname: \"zero\"\n      newname: \"one\"\n"
    ));
    assert!(document.contains(
        "  - scope: \"src/main.rs:55-56\"\n    type: identifier-renamed\n    details:\n      oldname: \"Old\"\n      newname: \"New\"\n"
    ));
    assert!(
        !document.contains("scope: \"src/main.rs:25\""),
        "{document}"
    );
    assert!(
        !document.contains("scope: \"src/main.rs:35\""),
        "{document}"
    );
    assert!(
        !document.contains("scope: \"src/main.rs:56\""),
        "{document}"
    );
    assert!(!document.contains("old: 50\n"), "{document}");
    assert!(!document.contains("new: 52\n"), "{document}");
}

#[test]
fn applies_a_line_addition_marked_as_a_comment() {
    let tree = Tree::new();
    tree.write("edited/src/main.rs", "fn main() {}\n");

    tree.apply(
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
      isComment: true
",
    )
    .unwrap();

    assert_eq!(tree.read("edited/src/main.rs"), b"// note\nfn main() {}\n");
}

#[test]
fn applies_a_line_addition_marked_as_whitespace() {
    let tree = Tree::new();
    tree.write("edited/src/main.rs", "fn main() {}\n");
    tree.write("edited/src/lib.rs", "fn lib() {}\n");

    tree.apply(
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
      isWhitespace: true
  - scope: \"src/lib.rs\"
    type: line-added
    details:
      old: 1
      lines: +1
      new: 1
      text: \"// note\"
      isComment: true
      isWhitespace: true
",
    )
    .unwrap();

    assert_eq!(tree.read("edited/src/main.rs"), b"    \nfn main() {}\n");
    assert_eq!(tree.read("edited/src/lib.rs"), b"// note\nfn lib() {}\n");
}

#[test]
fn a_comment_does_not_merge_with_an_adjacent_code_edit() {
    let tree = Tree::new();
    let before = "middle\n// old comment\nlet value = 1;\n// gone\nlet removed = 1;\ntail\n";
    let after = "// one\n// two\nmiddle\n// new comment here\nlet total = 2;\ntail\n";
    tree.write("left/src/merge.rs", before);
    tree.write("right/src/merge.rs", after);
    tree.write("edited/src/merge.rs", before);

    let document = tree.compare("left", "right");

    assert!(document.contains(
        "  - scope: \"src/merge.rs\"\n    type: line-added\n    details:\n      old: 1\n      lines: +2\n      new: 1\n      text: \"// one\\n// two\"\n      isComment: true\n"
    ));
    assert!(document.contains(
        "  - scope: \"src/merge.rs\"\n    type: line-modified\n    details:\n      old: 2\n      lines: +0\n      new: 4\n      text: \"// new comment here\"\n      isComment: true\n"
    ));
    assert!(document.contains(
        "  - scope: \"src/merge.rs\"\n    type: line-modified\n    details:\n      old: 3\n      lines: +0\n      new: 5\n      text: \"let total = 2;\"\n"
    ));
    assert!(!document.contains("let total = 2;\"\n      isComment:"));
    assert!(document.contains(
        "  - scope: \"src/merge.rs\"\n    type: line-removed\n    details:\n      old: 4\n      lines: -1\n      new: 6\n      text: \"// gone\"\n      isComment: true\n"
    ));
    assert!(document.contains(
        "  - scope: \"src/merge.rs\"\n    type: line-removed\n    details:\n      old: 5\n      lines: -1\n      new: 6\n      text: \"let removed = 1;\"\n"
    ));
    assert!(!document.contains("let removed = 1;\"\n      isComment:"));

    tree.apply("edited", &document).unwrap();
    assert_eq!(tree.read("edited/src/merge.rs"), after.as_bytes());
}
