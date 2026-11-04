use std::fs;

use super::run;

#[test]
fn records_an_added_rust_line_comment() {
    let root = std::env::temp_dir().join(format!("semdiff-comment-{}", std::process::id()));
    let left = root.join("left");
    let right = root.join("right");
    fs::create_dir_all(left.join("src")).unwrap();
    fs::create_dir_all(right.join("src")).unwrap();
    fs::write(left.join("src/main.rs"), "fn fibonacci() {}\n/// Shared.\n").unwrap();
    fs::write(
        right.join("src/main.rs"),
        "/// Returns the nth Fibonacci number.\n// Keep this.\nfn fibonacci() {} // trailing\n/// Shared.\n    /// Inner detail.\n",
    )
    .unwrap();
    fs::write(left.join("notes.txt"), "").unwrap();
    fs::write(right.join("notes.txt"), "/// not rust\n").unwrap();

    let output_path = root.join("out.yml");
    let left = left.to_str().unwrap();
    let right = right.to_str().unwrap();
    let output_path = output_path.to_str().unwrap();
    let output = run(&[left, right, output_path]);
    let document = fs::read_to_string(output_path).expect("failed to read semdif file");

    assert!(output.status.success());
    let changes = document.split_once("changes:\n").expect("changes list").1;
    assert_eq!(
        changes,
        concat!(
            "  - scope: \"src/main.rs\"\n",
            "    type: line-comment-added\n",
            "    details:\n",
            "      line: 1\n",
            "      text: \"/// Returns the nth Fibonacci number.\"\n",
            "  - scope: \"src/main.rs\"\n",
            "    type: line-comment-added\n",
            "    details:\n",
            "      line: 2\n",
            "      text: \"// Keep this.\"\n",
            "  - scope: \"src/main.rs\"\n",
            "    type: line-comment-added\n",
            "    details:\n",
            "      line: 5\n",
            "      text: \"    /// Inner detail.\"\n",
        )
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn records_an_added_python_line_comment() {
    let root = std::env::temp_dir().join(format!("semdiff-py-comment-{}", std::process::id()));
    let left = root.join("left");
    let right = root.join("right");
    fs::create_dir_all(&left).unwrap();
    fs::create_dir_all(&right).unwrap();
    fs::write(
        left.join("main.py"),
        "def fibonacci():\n    pass\n# Shared.\n",
    )
    .unwrap();
    fs::write(
        right.join("main.py"),
        "# Returns the nth Fibonacci number.\n# Keep this.\ndef fibonacci():\n    pass  # trailing\n# Shared.\n    # Inner detail.\n",
    )
    .unwrap();
    fs::write(left.join("notes.txt"), "").unwrap();
    fs::write(right.join("notes.txt"), "# not python\n").unwrap();

    let output_path = root.join("out.yml");
    let left = left.to_str().unwrap();
    let right = right.to_str().unwrap();
    let output_path = output_path.to_str().unwrap();
    let output = run(&[left, right, output_path]);
    let document = fs::read_to_string(output_path).expect("failed to read semdif file");

    assert!(output.status.success());
    let changes = document.split_once("changes:\n").expect("changes list").1;
    assert_eq!(
        changes,
        concat!(
            "  - scope: \"main.py\"\n",
            "    type: line-comment-added\n",
            "    details:\n",
            "      line: 1\n",
            "      text: \"# Returns the nth Fibonacci number.\"\n",
            "  - scope: \"main.py\"\n",
            "    type: line-comment-added\n",
            "    details:\n",
            "      line: 2\n",
            "      text: \"# Keep this.\"\n",
            "  - scope: \"main.py\"\n",
            "    type: line-comment-added\n",
            "    details:\n",
            "      line: 6\n",
            "      text: \"    # Inner detail.\"\n",
        )
    );

    let _ = fs::remove_dir_all(root);
}
