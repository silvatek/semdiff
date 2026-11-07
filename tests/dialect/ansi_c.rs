//! Tests of how the ANSI C sample is classified.

use std::path::Path;

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
        "  - scope: \"src/main.c:91\"\n    type: constant-changed\n    details:\n      new: 92\n      oldvalue: \"10\"\n      newvalue: \"20\"\n      occurrence: 1\n"
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
