//! Identifying and classifying a changed constant.

use crate::filesystem::Tree;

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
fn records_a_boolean_constant_change() {
    let tree = Tree::new();
    let rust_before = "fn main() {\n    let n = true;\n    let s = \"true\";\n    // true\n}\n";
    let rust_after = "fn main() {\n    let n = false;\n    let s = \"false\";\n    // false\n}\n";
    let python_before = "flag = True\n";
    let python_after = "flag = False\n";
    let c_before = "int flag = true;\n";
    let c_after = "int flag = false;\n";
    tree.write("left/src/main.rs", rust_before);
    tree.write("right/src/main.rs", rust_after);
    tree.write("edited/src/main.rs", rust_before);
    tree.write("left/calc.py", python_before);
    tree.write("right/calc.py", python_after);
    tree.write("edited/calc.py", python_before);
    tree.write("left/src/main.c", c_before);
    tree.write("right/src/main.c", c_after);
    tree.write("edited/src/main.c", c_before);

    let document = tree.compare("left", "right");

    assert!(document.contains(
        "  - scope: \"src/main.rs:2\"\n    type: constant-changed\n    details:\n      new: 2\n      oldvalue: \"true\"\n      newvalue: \"false\"\n      occurrence: 1\n"
    ));
    assert!(document.contains(
        "  - scope: \"src/main.rs:3-4\"\n    type: identifier-renamed\n    details:\n      oldname: \"true\"\n      newname: \"false\"\n"
    ));
    assert!(document.contains(
        "  - scope: \"calc.py:1\"\n    type: constant-changed\n    details:\n      new: 1\n      oldvalue: \"True\"\n      newvalue: \"False\"\n      occurrence: 1\n"
    ));
    assert!(document.contains(
        "  - scope: \"src/main.c:1\"\n    type: identifier-renamed\n    details:\n      new: 1\n      oldname: \"true\"\n      newname: \"false\"\n"
    ));

    tree.apply("edited", &document).unwrap();
    assert_eq!(tree.read("edited/src/main.rs"), rust_after.as_bytes());
    assert_eq!(tree.read("edited/calc.py"), python_after.as_bytes());
    assert_eq!(tree.read("edited/src/main.c"), c_after.as_bytes());
}
