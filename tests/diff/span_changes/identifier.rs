//! Tests of identifying and classifying a renamed name.

use crate::filesystem::Tree;

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
