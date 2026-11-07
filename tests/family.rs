//! Reading named blocks in source, including where one opens, closes, and
//! continues.

use crate::filesystem::Tree;
use std::path::Path;

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
        "  - scope: \"src/main.rs:106\"\n    type: constant-changed\n    details:\n      new: 107\n      oldvalue: \"10\"\n      newvalue: \"20\"\n      occurrence: 1\n"
    ), "{rust}");
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
        "  - scope: \"src/main.py:93\"\n    type: constant-changed\n    details:\n      new: 94\n      oldvalue: \"10\"\n      newvalue: \"20\"\n      occurrence: 1\n"
    ), "{python}");
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
