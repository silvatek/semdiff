//! Tests of recording an import move and merging those records.

use crate::filesystem::Tree;

#[test]
fn records_an_import_move_when_the_import_name_changes() {
    let tree = Tree::new();
    let rust_before = "\
use old::path::Item;
use old::path::Kept;
fn keep() {}
pub use old::path::Item;
fn keep() {}
use old::path::Item; // keep
fn keep() {}
use old::path::Item;
";
    let rust_after = "\
use new::place::Item;
use new::path::Kept;
fn keep() {}
pub use new::place::Item;
fn keep() {}
use new::place::Item; // changed
fn keep() {}
use  new::place::Item;
";
    let python_before = "\
import old.pkg.name
from old.pkg import name
from old.pkg import name
";
    let python_after = "\
import new.mod.name
from new.mod import name
import old.pkg
";
    let javascript_before = "import { read } from \"./old/path.js\";\n";
    let javascript_after = "import { read } from \"./new/place.js\";\n";
    let c_before = "\
#include \"old/path.h\"
#include <stdio.h>
#include \"http://old/a.h\"
";
    let c_after = "\
#include \"new/place.h\"
#include <stdlib.h>
#include \"http://new/b.h\"
";
    let note_before = "use old::path::Item;\n";
    let note_after = "use new::place::Item;\n";
    tree.write("left/src/lib.rs", rust_before);
    tree.write("right/src/lib.rs", rust_after);
    tree.write("edited/src/lib.rs", rust_before);
    tree.write("left/pkg/app.py", python_before);
    tree.write("right/pkg/app.py", python_after);
    tree.write("edited/pkg/app.py", python_before);
    tree.write("left/src/app.js", javascript_before);
    tree.write("right/src/app.js", javascript_after);
    tree.write("edited/src/app.js", javascript_before);
    tree.write("left/src/main.c", c_before);
    tree.write("right/src/main.c", c_after);
    tree.write("edited/src/main.c", c_before);
    tree.write("left/notes.txt", note_before);
    tree.write("right/notes.txt", note_after);
    tree.write("edited/notes.txt", note_before);

    let document = tree.compare("left", "right");

    assert!(document.contains(
        "  - scope: \"src/lib.rs:1\"\n    type: import-moved\n    details:\n      new: 1\n      oldname: \"old::path::Item\"\n      newname: \"new::place::Item\"\n"
    ), "{document}");
    assert!(document.contains(
        "  - scope: \"src/lib.rs:2\"\n    type: identifier-renamed\n    details:\n      new: 2\n      oldname: \"old\"\n      newname: \"new\"\n"
    ), "{document}");
    assert!(document.contains(
        "  - scope: \"src/lib.rs\"\n    type: line-modified\n    details:\n      old: 4\n      lines: +0\n      new: 4\n      text: \"pub use new::place::Item;\"\n"
    ), "{document}");
    assert!(document.contains(
        "  - scope: \"src/lib.rs\"\n    type: line-modified\n    details:\n      old: 6\n      lines: +0\n      new: 6\n      text: \"use new::place::Item; // changed\"\n"
    ), "{document}");
    assert!(document.contains(
        "  - scope: \"src/lib.rs\"\n    type: line-modified\n    details:\n      old: 8\n      lines: +0\n      new: 8\n      text: \"use  new::place::Item;\"\n"
    ), "{document}");
    assert!(document.contains(
        "  - scope: \"pkg/app.py:1\"\n    type: import-moved\n    details:\n      new: 1\n      oldname: \"old.pkg.name\"\n      newname: \"new.mod.name\"\n"
    ), "{document}");
    assert!(document.contains(
        "  - scope: \"pkg/app.py:2\"\n    type: import-moved\n    details:\n      new: 2\n      oldname: \"old.pkg import name\"\n      newname: \"new.mod import name\"\n"
    ), "{document}");
    assert!(document.contains(
        "  - scope: \"pkg/app.py\"\n    type: line-modified\n    details:\n      old: 3\n      lines: +0\n      new: 3\n      text: \"import old.pkg\"\n"
    ), "{document}");
    assert!(document.contains(
        "  - scope: \"src/app.js:1\"\n    type: import-moved\n    details:\n      new: 1\n      oldname: \"{ read } from \\\"./old/path.js\\\"\"\n      newname: \"{ read } from \\\"./new/place.js\\\"\"\n"
    ), "{document}");
    assert!(document.contains(
        "  - scope: \"src/main.c:1\"\n    type: import-moved\n    details:\n      new: 1\n      oldname: \"\\\"old/path.h\\\"\"\n      newname: \"\\\"new/place.h\\\"\"\n"
    ), "{document}");
    assert!(document.contains(
        "  - scope: \"src/main.c:2\"\n    type: identifier-renamed\n    details:\n      new: 2\n      oldname: \"stdio\"\n      newname: \"stdlib\"\n"
    ), "{document}");
    assert!(document.contains(
        "  - scope: \"src/main.c:3\"\n    type: import-moved\n    details:\n      new: 3\n      oldname: \"\\\"http://old/a.h\\\"\"\n      newname: \"\\\"http://new/b.h\\\"\"\n"
    ), "{document}");
    assert!(document.contains(
        "  - scope: \"notes.txt\"\n    type: line-modified\n    details:\n      old: 1\n      lines: +0\n      new: 1\n      text: \"use new::place::Item;\"\n"
    ), "{document}");
    assert!(
        !document
            .contains("type: import-moved\n    details:\n      new: 2\n      oldname: \"stdio\""),
        "{document}"
    );

    tree.apply("edited", &document).unwrap();
    assert_eq!(tree.read("edited/src/lib.rs"), rust_after.as_bytes());
    assert_eq!(tree.read("edited/pkg/app.py"), python_after.as_bytes());
    assert_eq!(tree.read("edited/src/app.js"), javascript_after.as_bytes());
    assert_eq!(tree.read("edited/src/main.c"), c_after.as_bytes());
    assert_eq!(tree.read("edited/notes.txt"), note_after.as_bytes());
}

#[test]
fn merges_import_moves_of_one_import_when_the_gap_does_not_import_it() {
    let tree = Tree::new();
    let before = "\
use old::path::Item;
let keep = 1;
use old::path::Item;
";
    let after = "\
use new::place::Item;
let keep = 1;
use new::place::Item;
";
    tree.write("left/src/main.rs", before);
    tree.write("right/src/main.rs", after);
    tree.write("edited/src/main.rs", before);

    let document = tree.compare("left", "right");

    assert!(document.contains(
        "  - scope: \"src/main.rs:1-3\"\n    type: import-moved\n    details:\n      oldname: \"old::path::Item\"\n      newname: \"new::place::Item\"\n"
    ), "{document}");
    assert!(!document.contains("scope: \"src/main.rs:1\""), "{document}");
    assert!(!document.contains("scope: \"src/main.rs:3\""), "{document}");

    tree.apply("edited", &document).unwrap();
    assert_eq!(tree.read("edited/src/main.rs"), after.as_bytes());
}

#[test]
fn keeps_import_moves_apart_when_an_import_of_that_name_stays() {
    let tree = Tree::new();
    let before = "\
use old::path::Item; // one
use old::path::Item; // two
use old::path::Item; // three
";
    let after = "\
use new::place::Item; // one
use old::path::Item; // two
use new::place::Item; // three
";
    tree.write("left/src/main.rs", before);
    tree.write("right/src/main.rs", after);
    tree.write("edited/src/main.rs", before);

    let document = tree.compare("left", "right");

    assert!(document.contains(
        "  - scope: \"src/main.rs:1\"\n    type: import-moved\n    details:\n      new: 1\n      oldname: \"old::path::Item\"\n      newname: \"new::place::Item\"\n"
    ), "{document}");
    assert!(document.contains(
        "  - scope: \"src/main.rs:3\"\n    type: import-moved\n    details:\n      new: 3\n      oldname: \"old::path::Item\"\n      newname: \"new::place::Item\"\n"
    ), "{document}");
    assert!(
        !document.contains("scope: \"src/main.rs:1-3\""),
        "{document}"
    );

    tree.apply("edited", &document).unwrap();
    assert_eq!(tree.read("edited/src/main.rs"), after.as_bytes());
}

#[test]
fn merges_import_moves_of_one_import_across_files_in_one_directory() {
    let tree = Tree::new();
    tree.write("left/src/a.rs", "use old::path::Item;\n");
    tree.write("right/src/a.rs", "use new::place::Item;\n");
    tree.write("edited/src/a.rs", "use old::path::Item;\n");
    tree.write("left/src/b.rs", "use old::path::Item;\n");
    tree.write("right/src/b.rs", "use new::place::Item;\n");
    tree.write("edited/src/b.rs", "use old::path::Item;\n");
    tree.write("left/tests/c.rs", "use old::path::Item;\n");
    tree.write("right/tests/c.rs", "use old::path::Item;\n");
    tree.write("edited/tests/c.rs", "use old::path::Item;\n");

    let document = tree.compare("left", "right");

    assert!(document.contains(
        "  - scope: \"src\"\n    type: import-moved\n    details:\n      oldname: \"old::path::Item\"\n      newname: \"new::place::Item\"\n"
    ), "{document}");
    assert!(!document.contains("scope: \"src/a.rs:"), "{document}");
    assert!(!document.contains("scope: \"src/b.rs:"), "{document}");

    tree.apply("edited", &document).unwrap();
    assert_eq!(tree.read("edited/src/a.rs"), b"use new::place::Item;\n");
    assert_eq!(tree.read("edited/src/b.rs"), b"use new::place::Item;\n");
    assert_eq!(tree.read("edited/tests/c.rs"), b"use old::path::Item;\n");
}

#[test]
fn keeps_file_import_moves_when_the_directory_still_imports_that_name() {
    let tree = Tree::new();
    tree.write("left/src/a.rs", "use old::path::Item;\n");
    tree.write("right/src/a.rs", "use new::place::Item;\n");
    tree.write("edited/src/a.rs", "use old::path::Item;\n");
    tree.write("left/src/b.rs", "use old::path::Item;\n");
    tree.write("right/src/b.rs", "use new::place::Item;\n");
    tree.write("edited/src/b.rs", "use old::path::Item;\n");
    tree.write("left/src/c.rs", "use old::path::Item;\n");
    tree.write("right/src/c.rs", "use old::path::Item;\n");
    tree.write("edited/src/c.rs", "use old::path::Item;\n");

    let document = tree.compare("left", "right");

    assert!(document.contains(
        "  - scope: \"src/a.rs:1\"\n    type: import-moved\n    details:\n      new: 1\n      oldname: \"old::path::Item\"\n      newname: \"new::place::Item\"\n"
    ), "{document}");
    assert!(document.contains(
        "  - scope: \"src/b.rs:1\"\n    type: import-moved\n    details:\n      new: 1\n      oldname: \"old::path::Item\"\n      newname: \"new::place::Item\"\n"
    ), "{document}");
    assert!(!document.contains("scope: \"src\""), "{document}");

    tree.apply("edited", &document).unwrap();
    assert_eq!(tree.read("edited/src/a.rs"), b"use new::place::Item;\n");
    assert_eq!(tree.read("edited/src/b.rs"), b"use new::place::Item;\n");
    assert_eq!(tree.read("edited/src/c.rs"), b"use old::path::Item;\n");
}
