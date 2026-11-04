use std::fs;
use std::path::Path;

use super::run;

#[test]
fn records_one_symbol_rename_across_rust_files() {
    let root = std::env::temp_dir().join(format!("semdiff-symbol-{}", std::process::id()));
    let left = root.join("left");
    let right = root.join("right");
    fs::create_dir_all(left.join("src")).unwrap();
    fs::create_dir_all(right.join("src")).unwrap();
    fs::write(left.join("src/a.rs"), "fn fibonacci() { fibonacci(); }\n").unwrap();
    fs::write(left.join("src/b.rs"), "fn use_it() { fibonacci(); }\n").unwrap();
    fs::write(right.join("src/a.rs"), "fn fib() { fib(); }\n").unwrap();
    fs::write(
        right.join("src/b.rs"),
        "fn use_it() { fib(); } // fibonacci\n",
    )
    .unwrap();

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
            "  - scope: \".\"\n",
            "    type: symbol-renamed\n",
            "    details:\n",
            "      from: \"fibonacci\"\n",
            "      to: \"fib\"\n",
        )
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn leaves_a_partial_symbol_rename_unrecorded() {
    let root = std::env::temp_dir().join(format!("semdiff-symbol-partial-{}", std::process::id()));
    let left = root.join("left");
    let right = root.join("right");
    fs::create_dir_all(left.join("src")).unwrap();
    fs::create_dir_all(right.join("src")).unwrap();
    fs::write(left.join("src/a.rs"), "fn fibonacci() {}\n").unwrap();
    fs::write(left.join("src/b.rs"), "fn use_it() { fibonacci(); }\n").unwrap();
    fs::write(right.join("src/a.rs"), "fn fib() {}\n").unwrap();
    fs::write(right.join("src/b.rs"), "fn use_it() { fibonacci(); }\n").unwrap();

    let output_path = root.join("out.yml");
    let output = run(&[
        left.to_str().unwrap(),
        right.to_str().unwrap(),
        output_path.to_str().unwrap(),
    ]);
    let document = fs::read_to_string(&output_path).expect("failed to read semdif file");

    assert!(output.status.success());
    assert!(!document.contains("symbol-renamed"), "{document}");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn records_the_rust_sample_symbol_rename() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let v1 = manifest.join("TestCode/rust/main/v1");
    let v2 = manifest.join("TestCode/rust/main/v2");
    let root = std::env::temp_dir().join(format!("semdiff-symbol-sample-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let diff = root.join("diff.yml");
    let output = run(&[
        v1.to_str().unwrap(),
        v2.to_str().unwrap(),
        diff.to_str().unwrap(),
    ]);
    let document = fs::read_to_string(&diff).expect("failed to read semdif file");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(document.matches("type: symbol-renamed").count(), 1);
    assert!(
        document.contains(concat!(
            "  - scope: \".\"\n",
            "    type: symbol-renamed\n",
            "    details:\n",
            "      from: \"fibonacci\"\n",
            "      to: \"fib\"\n",
        )),
        "{document}"
    );

    let _ = fs::remove_dir_all(root);
}
