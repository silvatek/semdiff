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
fn records_named_blocks_in_the_samples() {
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
        "  - scope: \"src/main.rs\"\n    type: named-block-added\n    details:\n      old: 73\n      lines: +3\n      new: 74\n      text: \"fn shifted(n: u32) -> u64 {\\n    n + 1\\n}\"\n      category: \"function\"\n      name: \"shifted\"\n"
    ), "{rust}");
    assert!(rust.contains(
        "  - scope: \"src/main.rs\"\n    type: named-block-added\n    details:\n      old: 75\n      lines: +3\n      new: 79\n      text: \"struct Step {\\n    by: u32,\\n}\"\n      category: \"structure\"\n      name: \"Step\"\n"
    ), "{rust}");
    assert!(rust.contains(
        "  - scope: \"src/main.rs[Count]\"\n    type: named-block-added\n    details:\n      old: 82\n      lines: +3\n      new: 89\n      text: \"    fn step(&self) -> u32 {\\n        self.by\\n    }\"\n      category: \"method\"\n      name: \"step\"\n"
    ), "{rust}");
    assert!(python.contains(
        "  - scope: \"src/main.py\"\n    type: named-block-added\n    details:\n      old: 71\n      lines: +2\n      new: 72\n      text: \"def shifted(n):\\n    return n + 1\"\n      category: \"function\"\n      name: \"shifted\"\n"
    ), "{python}");
    assert!(python.contains(
        "  - scope: \"src/main.py\"\n    type: named-block-added\n    details:\n      old: 73\n      lines: +2\n      new: 76\n      text: \"class Step:\\n    by = 1\"\n      category: \"class\"\n      name: \"Step\"\n"
    ), "{python}");
    assert!(python.contains(
        "  - scope: \"src/main.py[Count]\"\n    type: named-block-added\n    details:\n      old: 77\n      lines: +2\n      new: 82\n      text: \"    def step(self):\\n        return self.by\"\n      category: \"method\"\n      name: \"step\"\n"
    ), "{python}");
    assert!(rust.contains(
        "  - scope: \"src/main.rs\"\n    type: named-block-removed\n    details:\n      old: 85\n      lines: -3\n      new: 95\n      text: \"fn dropped(n: u32) -> u64 {\\n    n - 1\\n}\"\n      category: \"function\"\n      name: \"dropped\"\n"
    ), "{rust}");
    assert!(rust.contains(
        "  - scope: \"src/main.rs\"\n    type: named-block-removed\n    details:\n      old: 90\n      lines: -3\n      new: 97\n      text: \"struct Gone {\\n    gone: u32,\\n}\"\n      category: \"structure\"\n      name: \"Gone\"\n"
    ), "{rust}");
    assert!(rust.contains(
        "  - scope: \"src/main.rs[Total]\"\n    type: named-block-removed\n    details:\n      old: 100\n      lines: -3\n      new: 104\n      text: \"    fn undo(&self) -> u32 {\\n        self.left\\n    }\"\n      category: \"method\"\n      name: \"undo\"\n"
    ), "{rust}");
    assert!(python.contains(
        "  - scope: \"src/main.py\"\n    type: named-block-removed\n    details:\n      old: 79\n      lines: -2\n      new: 86\n      text: \"def dropped(n):\\n    return n - 1\"\n      category: \"function\"\n      name: \"dropped\"\n"
    ), "{python}");
    assert!(python.contains(
        "  - scope: \"src/main.py\"\n    type: named-block-removed\n    details:\n      old: 83\n      lines: -2\n      new: 88\n      text: \"class Gone:\\n    gone = 2\"\n      category: \"class\"\n      name: \"Gone\"\n"
    ), "{python}");
    assert!(python.contains(
        "  - scope: \"src/main.py[Total]\"\n    type: named-block-removed\n    details:\n      old: 89\n      lines: -2\n      new: 92\n      text: \"    def undo(self):\\n        return self.left\"\n      category: \"method\"\n      name: \"undo\"\n"
    ), "{python}");
}

#[test]
fn classifies_the_javascript_sample() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let javascript = semdiff::compare_sources(
        &semdiff::StdFileSystem::new(),
        manifest
            .join("TestCode/javascript/main/v1")
            .to_str()
            .unwrap(),
        manifest
            .join("TestCode/javascript/main/v2")
            .to_str()
            .unwrap(),
    )
    .unwrap();

    assert!(javascript.contains(
        "  - scope: \"src\"\n    type: identifier-renamed\n    details:\n      oldname: \"fibonacci\"\n      newname: \"fib\"\n"
    ), "{javascript}");
    assert!(javascript.contains(
        "  - scope: \"src/main.js\"\n    type: line-added\n    details:\n      old: 18\n      lines: +1\n      new: 18\n      text: \"// Print Fibonacci numbers.\"\n      isComment: true\n"
    ), "{javascript}");
    assert!(javascript.contains(
        "  - scope: \"src/main.js:38\"\n    type: identifier-renamed\n    details:\n      new: 38\n      oldname: \"zero\"\n      newname: \"one\"\n"
    ), "{javascript}");
    assert!(javascript.contains(
        "  - scope: \"src/main.js:47-48\"\n    type: identifier-renamed\n    details:\n      oldname: \"Old\"\n      newname: \"New\"\n"
    ), "{javascript}");
    assert!(javascript.contains(
        "  - scope: \"src/main.js\"\n    type: line-modified\n    details:\n      old: 57\n      lines: +0\n      new: 57\n      text: \"    let value = 1;\"\n      isWhitespace: true\n"
    ), "{javascript}");
    assert!(javascript.contains(
        "  - scope: \"src/main.js\"\n    type: line-modified\n    details:\n      old: 61\n      lines: +0\n      new: 61\n      text: \"const trailing = 1; \"\n      isWhitespace: true\n"
    ), "{javascript}");
    assert!(javascript.contains(
        "  - scope: \"src/main.js\"\n    type: line-modified\n    details:\n      old: 64\n      lines: +0\n      new: 64\n      text: \"    // margin\"\n      isComment: true\n      isWhitespace: true\n"
    ), "{javascript}");
    assert!(javascript.contains(
        "  - scope: \"src/main.js\"\n    type: line-added\n    details:\n      old: 67\n      lines: +1\n      new: 67\n      text: \"\"\n      isWhitespace: true\n"
    ), "{javascript}");
    assert!(javascript.contains(
        "  - scope: \"src/main.js\"\n    type: named-block-added\n    details:\n      old: 68\n      lines: +3\n      new: 69\n      text: \"function shifted(n) {\\n    return n + 1;\\n}\"\n      category: \"function\"\n      name: \"shifted\"\n"
    ), "{javascript}");
    assert!(javascript.contains(
        "  - scope: \"src/main.js\"\n    type: named-block-added\n    details:\n      old: 70\n      lines: +3\n      new: 74\n      text: \"class Step {\\n    by = 1;\\n}\"\n      category: \"class\"\n      name: \"Step\"\n"
    ), "{javascript}");
    assert!(javascript.contains(
        "  - scope: \"src/main.js[Count]\"\n    type: named-block-added\n    details:\n      old: 74\n      lines: +3\n      new: 81\n      text: \"    step() {\\n        return this.by;\\n    }\"\n      category: \"method\"\n      name: \"step\"\n"
    ), "{javascript}");
    assert!(javascript.contains(
        "  - scope: \"src/main.js\"\n    type: named-block-removed\n    details:\n      old: 77\n      lines: -3\n      new: 87\n      text: \"function dropped(n) {\\n    return n - 1;\\n}\"\n      category: \"function\"\n      name: \"dropped\"\n"
    ), "{javascript}");
    assert!(javascript.contains(
        "  - scope: \"src/main.js\"\n    type: named-block-removed\n    details:\n      old: 82\n      lines: -3\n      new: 89\n      text: \"class Gone {\\n    gone = 2;\\n}\"\n      category: \"class\"\n      name: \"Gone\"\n"
    ), "{javascript}");
    assert!(javascript.contains(
        "  - scope: \"src/main.js[Total]\"\n    type: named-block-removed\n    details:\n      old: 89\n      lines: -3\n      new: 93\n      text: \"    undo() {\\n        return this.left;\\n    }\"\n      category: \"method\"\n      name: \"undo\"\n"
    ), "{javascript}");
}

#[test]
fn classifies_the_c_sample() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let c = semdiff::compare_sources(
        &semdiff::StdFileSystem::new(),
        manifest.join("TestCode/c/main/v1").to_str().unwrap(),
        manifest.join("TestCode/c/main/v2").to_str().unwrap(),
    )
    .unwrap();

    assert!(c.contains(
        "  - scope: \"src\"\n    type: identifier-renamed\n    details:\n      oldname: \"fibonacci\"\n      newname: \"fib\"\n"
    ), "{c}");
    assert!(c.contains(
        "  - scope: \"src/main.c\"\n    type: line-added\n    details:\n      old: 28\n      lines: +1\n      new: 28\n      text: \"// Print Fibonacci numbers.\"\n      isComment: true\n"
    ), "{c}");
    assert!(c.contains(
        "  - scope: \"src/main.c:50\"\n    type: identifier-renamed\n    details:\n      new: 50\n      oldname: \"zero\"\n      newname: \"one\"\n"
    ), "{c}");
    assert!(c.contains(
        "  - scope: \"src/main.c:59-60\"\n    type: identifier-renamed\n    details:\n      oldname: \"Old\"\n      newname: \"New\"\n"
    ), "{c}");
    assert!(c.contains(
        "  - scope: \"src/main.c\"\n    type: line-modified\n    details:\n      old: 66\n      lines: +0\n      new: 66\n      text: \"    int value = 1;\"\n      isWhitespace: true\n"
    ), "{c}");
    assert!(c.contains(
        "  - scope: \"src/main.c\"\n    type: line-modified\n    details:\n      old: 70\n      lines: +0\n      new: 70\n      text: \"int trailing = 1; \"\n      isWhitespace: true\n"
    ), "{c}");
    assert!(c.contains(
        "  - scope: \"src/main.c\"\n    type: line-modified\n    details:\n      old: 73\n      lines: +0\n      new: 73\n      text: \"    // margin\"\n      isComment: true\n      isWhitespace: true\n"
    ), "{c}");
    assert!(c.contains(
        "  - scope: \"src/main.c\"\n    type: line-added\n    details:\n      old: 76\n      lines: +1\n      new: 76\n      text: \"\"\n      isWhitespace: true\n"
    ), "{c}");
    assert!(c.contains(
        "  - scope: \"src/main.c\"\n    type: named-block-added\n    details:\n      old: 77\n      lines: +3\n      new: 78\n      text: \"int shifted(int n) {\\n    return n + 1;\\n}\"\n      category: \"function\"\n      name: \"shifted\"\n"
    ), "{c}");
    assert!(c.contains(
        "  - scope: \"src/main.c\"\n    type: line-added\n    details:\n      old: 79\n      lines: +3\n      new: 83\n      text: \"struct Step {\\n    int by;\\n};\"\n"
    ), "{c}");
    assert!(c.contains(
        "  - scope: \"src/main.c\"\n    type: named-block-removed\n    details:\n      old: 81\n      lines: -3\n      new: 88\n      text: \"int dropped(int n) {\\n    return n - 1;\\n}\"\n      category: \"function\"\n      name: \"dropped\"\n"
    ), "{c}");
    assert!(c.contains(
        "  - scope: \"src/main.c\"\n    type: line-removed\n    details:\n      old: 86\n      lines: -3\n      new: 90\n      text: \"struct Gone {\\n    int gone;\\n};\"\n"
    ), "{c}");
    assert!(!c.contains("name: \"Step\""), "{c}");
    assert!(!c.contains("name: \"Gone\""), "{c}");
}

#[test]
fn an_addition_that_is_not_one_block_stays_a_line_addition() {
    let tree = Tree::new();
    tree.write("left/src/pair.rs", "fn kept() {}\n");
    tree.write(
        "right/src/pair.rs",
        "fn kept() {}\nfn one() {}\nfn two() {}\n",
    );
    tree.write("left/src/split.rs", "fn kept() {}\n");
    tree.write(
        "right/src/split.rs",
        "fn kept() {}\nfn added() {\n    let value = 1;\n\n    let more = 2;\n}\n",
    );
    tree.write("left/calc.py", "x = 0\n");
    tree.write(
        "right/calc.py",
        "x = 0\ndef added():\n    return 1\n\n    return 2\n",
    );
    tree.write("left/pair.py", "x = 0\n");
    tree.write(
        "right/pair.py",
        "x = 0\ndef one():\n    return 1\n\ndef two():\n    return 2\n",
    );

    let document = tree.compare("left", "right");

    assert!(
        document.contains(
            "scope: \"src/pair.rs\"\n    type: line-added\n    details:\n      old: 2\n      lines: +2\n      new: 2\n      text: \"fn one() {}\\nfn two() {}\"\n"
        ),
        "{document}"
    );
    assert!(
        !document.contains("scope: \"src/pair.rs\"\n    type: named-block-added"),
        "{document}"
    );
    assert!(!document.contains("name: \"added\""), "{document}");
    assert!(
        document.contains("scope: \"src/split.rs\"\n    type: line-added"),
        "{document}"
    );
    assert!(
        !document.contains("scope: \"src/split.rs\"\n    type: named-block-added"),
        "{document}"
    );
    assert!(
        document.contains("scope: \"calc.py\"\n    type: line-added"),
        "{document}"
    );
    assert!(
        !document.contains("scope: \"calc.py\"\n    type: named-block-added"),
        "{document}"
    );
    assert!(
        document.contains(
            "scope: \"pair.py\"\n    type: named-block-added\n    details:\n      old: 2\n      lines: +2\n      new: 2\n      text: \"def one():\\n    return 1\"\n      category: \"function\"\n      name: \"one\"\n"
        ),
        "{document}"
    );
    assert!(
        document.contains(
            "scope: \"pair.py\"\n    type: named-block-added\n    details:\n      old: 2\n      lines: +2\n      new: 5\n      text: \"def two():\\n    return 2\"\n      category: \"function\"\n      name: \"two\"\n"
        ),
        "{document}"
    );
}

#[test]
fn records_a_method_added_inside_a_type() {
    let tree = Tree::new();
    tree.write("left/src/types.rs", "impl Foo {\n}\n");
    tree.write("right/src/types.rs", "impl Foo {\n    fn added() {}\n}\n");
    tree.write(
        "left/src/nested.rs",
        "fn outer() {\n    let value = 1;\n}\n",
    );
    tree.write(
        "right/src/nested.rs",
        "fn outer() {\n    fn added() {}\n    let value = 1;\n}\n",
    );
    tree.write("left/types.py", "class Foo:\n    x = 1\n");
    tree.write(
        "right/types.py",
        "class Foo:\n    x = 1\n    def added(self):\n        return 1\n",
    );
    tree.write("left/nested.py", "def outer():\n    return 0\n");
    tree.write(
        "right/nested.py",
        "def outer():\n    def added():\n        return 1\n    return 0\n",
    );

    let document = tree.compare("left", "right");

    assert!(
        document.contains(
            "scope: \"src/types.rs[Foo]\"\n    type: named-block-added\n    details:\n      old: 2\n      lines: +1\n      new: 2\n      text: \"    fn added() {}\"\n      category: \"method\"\n      name: \"added\"\n"
        ),
        "{document}"
    );
    assert!(
        document.contains(
            "scope: \"src/nested.rs\"\n    type: named-block-added\n    details:\n      old: 2\n      lines: +1\n      new: 2\n      text: \"    fn added() {}\"\n      category: \"function\"\n      name: \"added\"\n"
        ),
        "{document}"
    );
    assert!(
        document.contains(
            "scope: \"types.py[Foo]\"\n    type: named-block-added\n    details:\n      old: 3\n      lines: +2\n      new: 3\n      text: \"    def added(self):\\n        return 1\"\n      category: \"method\"\n      name: \"added\"\n"
        ),
        "{document}"
    );
    assert!(
        document.contains(
            "scope: \"nested.py\"\n    type: named-block-added\n    details:\n      old: 2\n      lines: +2\n      new: 2\n      text: \"    def added():\\n        return 1\"\n      category: \"function\"\n      name: \"added\"\n"
        ),
        "{document}"
    );
}

#[test]
fn records_an_added_rust_function_as_a_named_block() {
    let tree = Tree::new();
    tree.write("left/src/add.rs", "fn kept() {}\n");
    tree.write(
        "right/src/add.rs",
        "fn kept() {}\nfn added() {\n    let value = 1;\n}\n",
    );
    tree.write("edited/src/add.rs", "fn kept() {}\n");
    tree.write("left/src/note.rs", "fn kept() {}\n");
    tree.write("right/src/note.rs", "fn kept() {}\nlet value = 1;\n");
    tree.write("edited/src/note.rs", "fn kept() {}\n");
    tree.write("left/calc.py", "def kept():\n    return 1\n");
    tree.write(
        "right/calc.py",
        "def kept():\n    return 1\ndef added():\n    return 2\n",
    );
    tree.write("edited/calc.py", "def kept():\n    return 1\n");

    let document = tree.compare("left", "right");

    assert!(document.contains(
        "  - scope: \"src/add.rs\"\n    type: named-block-added\n    details:\n      old: 2\n      lines: +3\n      new: 2\n      text: \"fn added() {\\n    let value = 1;\\n}\"\n      category: \"function\"\n      name: \"added\"\n"
    ), "{document}");
    assert!(
        !document.contains("scope: \"src/add.rs\"\n    type: line-added"),
        "{document}"
    );
    assert!(document.contains(
        "  - scope: \"src/note.rs\"\n    type: line-added\n    details:\n      old: 2\n      lines: +1\n      new: 2\n      text: \"let value = 1;\"\n"
    ), "{document}");
    assert!(document.contains(
        "  - scope: \"calc.py\"\n    type: named-block-added\n    details:\n      old: 3\n      lines: +2\n      new: 3\n      text: \"def added():\\n    return 2\"\n      category: \"function\"\n      name: \"added\"\n"
    ), "{document}");
    assert!(
        !document.contains("scope: \"calc.py\"\n    type: line-added"),
        "{document}"
    );

    tree.apply("edited", &document).unwrap();
    assert_eq!(
        tree.read("edited/src/add.rs"),
        b"fn kept() {}\nfn added() {\n    let value = 1;\n}\n"
    );
    assert_eq!(
        tree.read("edited/src/note.rs"),
        b"fn kept() {}\nlet value = 1;\n"
    );
    assert_eq!(
        tree.read("edited/calc.py"),
        b"def kept():\n    return 1\ndef added():\n    return 2\n"
    );
}

#[test]
fn records_a_removed_named_block() {
    let tree = Tree::new();
    tree.write(
        "left/src/drop.rs",
        "fn kept() {}\nfn dropped() {\n    let value = 1;\n}\n",
    );
    tree.write("right/src/drop.rs", "fn kept() {}\n");
    tree.write(
        "edited/src/drop.rs",
        "fn kept() {}\nfn dropped() {\n    let value = 1;\n}\n",
    );
    tree.write("left/src/note.rs", "fn kept() {}\nlet value = 1;\n");
    tree.write("right/src/note.rs", "fn kept() {}\n");
    tree.write("edited/src/note.rs", "fn kept() {}\nlet value = 1;\n");
    tree.write(
        "left/calc.py",
        "def kept():\n    return 1\ndef dropped():\n    return 2\n",
    );
    tree.write("right/calc.py", "def kept():\n    return 1\n");
    tree.write(
        "edited/calc.py",
        "def kept():\n    return 1\ndef dropped():\n    return 2\n",
    );
    tree.write("left/src/types.rs", "impl Foo {\n    fn dropped() {}\n}\n");
    tree.write("right/src/types.rs", "impl Foo {\n}\n");
    tree.write(
        "edited/src/types.rs",
        "impl Foo {\n    fn dropped() {}\n}\n",
    );
    tree.write(
        "left/types.py",
        "class Foo:\n    x = 1\n    def dropped(self):\n        return 1\n",
    );
    tree.write("right/types.py", "class Foo:\n    x = 1\n");
    tree.write(
        "edited/types.py",
        "class Foo:\n    x = 1\n    def dropped(self):\n        return 1\n",
    );

    let document = tree.compare("left", "right");

    assert!(document.contains(
        "  - scope: \"src/drop.rs\"\n    type: named-block-removed\n    details:\n      old: 2\n      lines: -3\n      new: 2\n      text: \"fn dropped() {\\n    let value = 1;\\n}\"\n      category: \"function\"\n      name: \"dropped\"\n"
    ), "{document}");
    assert!(
        !document.contains("scope: \"src/drop.rs\"\n    type: line-removed"),
        "{document}"
    );
    assert!(document.contains(
        "  - scope: \"src/note.rs\"\n    type: line-removed\n    details:\n      old: 2\n      lines: -1\n      new: 2\n      text: \"let value = 1;\"\n"
    ), "{document}");
    assert!(document.contains(
        "  - scope: \"calc.py\"\n    type: named-block-removed\n    details:\n      old: 3\n      lines: -2\n      new: 3\n      text: \"def dropped():\\n    return 2\"\n      category: \"function\"\n      name: \"dropped\"\n"
    ), "{document}");
    assert!(document.contains(
        "  - scope: \"src/types.rs[Foo]\"\n    type: named-block-removed\n    details:\n      old: 2\n      lines: -1\n      new: 2\n      text: \"    fn dropped() {}\"\n      category: \"method\"\n      name: \"dropped\"\n"
    ), "{document}");
    assert!(document.contains(
        "  - scope: \"types.py[Foo]\"\n    type: named-block-removed\n    details:\n      old: 3\n      lines: -2\n      new: 3\n      text: \"    def dropped(self):\\n        return 1\"\n      category: \"method\"\n      name: \"dropped\"\n"
    ), "{document}");

    tree.apply("edited", &document).unwrap();
    assert_eq!(tree.read("edited/src/drop.rs"), b"fn kept() {}\n");
    assert_eq!(tree.read("edited/src/note.rs"), b"fn kept() {}\n");
    assert_eq!(tree.read("edited/calc.py"), b"def kept():\n    return 1\n");
    assert_eq!(tree.read("edited/src/types.rs"), b"impl Foo {\n}\n");
    assert_eq!(tree.read("edited/types.py"), b"class Foo:\n    x = 1\n");
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
        "  - scope: \".\"\n    type: identifier-renamed\n    details:\n      oldname: \"fibonacci\"\n      newname: \"fib\"\n"
    ));
    assert!(!document.contains("scope: \"src/main.rs:1\""), "{document}");
    assert!(!document.contains("scope: \"calc.py:1\""), "{document}");
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
fn records_an_integer_constant_change() {
    let tree = Tree::new();
    let before = "fn main() {\n    let n = 1;\n    let s = \"1\";\n    // 1\n}\n";
    let after = "fn main() {\n    let n = 42;\n    let s = \"2\";\n    // 2\n}\n";
    tree.write("left/src/main.rs", before);
    tree.write("right/src/main.rs", after);
    tree.write("edited/src/main.rs", before);

    let document = tree.compare("left", "right");

    assert!(document.contains(
        "  - scope: \"src/main.rs:2\"\n    type: constant-changed\n    details:\n      new: 2\n      oldvalue: \"1\"\n      newvalue: \"42\"\n      occurrence: 1\n"
    ));
    assert!(document.contains(
        "  - scope: \"src/main.rs\"\n    type: line-modified\n    details:\n      old: 3\n      lines: +0\n      new: 3\n      text: \"    let s = \\\"2\\\";\"\n"
    ));
    assert!(document.contains(
        "  - scope: \"src/main.rs\"\n    type: line-modified\n    details:\n      old: 4\n      lines: +0\n      new: 4\n      text: \"    // 2\"\n      isComment: true\n"
    ));

    tree.apply("edited", &document).unwrap();
    assert_eq!(tree.read("edited/src/main.rs"), after.as_bytes());
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

/// Whether `record` is present and its next detail is `isWhitespace`.
fn follows_with_whitespace(document: &str, record: &str) -> bool {
    let Some((_, rest)) = document.split_once(record) else {
        panic!("missing record in:\n{document}");
    };
    rest.starts_with("      isWhitespace:")
}

fn changed_note() -> (Tree, String) {
    let tree = Tree::new();
    tree.write("left/note.txt", "old line\n");
    tree.write("right/note.txt", "He said \"hello\"\nNext line\n");
    tree.write("edited/note.txt", "old line\n");
    let document = tree.compare("left", "right");
    (tree, document)
}
