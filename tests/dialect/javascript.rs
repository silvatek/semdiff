//! Tests of how the JavaScript sample is classified.

use std::path::Path;

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
        "  - scope: \"src/main.js:95\"\n    type: constant-changed\n    details:\n      new: 96\n      oldvalue: \"10\"\n      newvalue: \"20\"\n      occurrence: 1\n"
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
    assert!(javascript.contains(
        "  - scope: \"src/main.js\"\n    type: named-block-added\n    details:\n      old: 98\n      lines: +5\n      new: 99\n      text: \"function spaced(n) {\\n    const value = n;\\n\\n    return value + 1;\\n}\"\n      category: \"function\"\n      name: \"spaced\"\n"
    ), "{javascript}");
    assert!(javascript.contains(
        "  - scope: \"src/main.js\"\n    type: named-block-added\n    details:\n      old: 99\n      lines: +5\n      new: 105\n      text: \"function noted(n) {\\n    const value = n;\\n    // note\\n    return value + 1;\\n}\"\n      category: \"function\"\n      name: \"noted\"\n"
    ), "{javascript}");
    assert!(javascript.contains(
        "  - scope: \"src/main.js\"\n    type: named-block-added\n    details:\n      old: 100\n      lines: +6\n      new: 111\n      text: \"function outer() {\\n    function inner() {\\n        return 1;\\n    }\\n    return 0;\\n}\"\n      category: \"function\"\n      name: \"outer\"\n"
    ), "{javascript}");
    assert!(javascript.contains(
        "  - scope: \"src/main.js\"\n    type: named-block-added\n    details:\n      old: 101\n      lines: +1\n      new: 118\n      text: \"function one() { return 1; }\"\n      category: \"function\"\n      name: \"one\"\n  - scope: \"src/main.js\"\n    type: line-added\n    details:\n      old: 101\n      lines: +1\n      new: 119\n      text: \"\"\n      isWhitespace: true\n  - scope: \"src/main.js\"\n    type: named-block-added\n    details:\n      old: 101\n      lines: +1\n      new: 120\n      text: \"function two() { return 2; }\"\n      category: \"function\"\n      name: \"two\"\n"
    ), "{javascript}");
    assert!(javascript.contains(
        "  - scope: \"src/main.js\"\n    type: named-block-removed\n    details:\n      old: 102\n      lines: -5\n      new: 122\n      text: \"function gapped(n) {\\n    const value = n;\\n\\n    return value - 1;\\n}\"\n      category: \"function\"\n      name: \"gapped\"\n"
    ), "{javascript}");
    assert!(!javascript.contains("name: \"inner\""), "{javascript}");
}
