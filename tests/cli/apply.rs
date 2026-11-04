use super::run;

use std::fs;

#[test]
fn applies_a_rename_inside_a_directory() {
    let root = std::env::temp_dir().join(format!("semdiff-apply-{}", std::process::id()));
    let left = root.join("left");
    let right = root.join("right");
    let edited = root.join("edited");
    fs::create_dir_all(left.join("src")).unwrap();
    fs::create_dir_all(right.join("src")).unwrap();
    fs::create_dir_all(right.join("notes")).unwrap();
    fs::create_dir_all(edited.join("src")).unwrap();
    let quote = "Brevity is the soul of wit.\n";
    let source = "fn fibonacci() {}\n";
    fs::write(left.join("quote.txt"), quote).unwrap();
    fs::write(right.join("notes/quotation.txt"), quote).unwrap();
    fs::write(left.join("src/main.rs"), source).unwrap();
    fs::write(right.join("src/main.rs"), source).unwrap();
    fs::write(edited.join("quote.txt"), quote).unwrap();
    fs::write(edited.join("src/main.rs"), source).unwrap();

    let output_path = root.join("out.yml");
    let left = left.to_str().unwrap();
    let right = right.to_str().unwrap();
    let edited_path = edited.to_str().unwrap();
    let output_path = output_path.to_str().unwrap();
    let compared = run(&[left, right, output_path]);
    let applied = run(&["--apply", output_path, edited_path]);

    assert!(compared.status.success());
    assert!(
        applied.status.success(),
        "{}",
        String::from_utf8_lossy(&applied.stderr)
    );
    assert!(!edited.join("quote.txt").exists());
    assert_eq!(
        fs::read_to_string(edited.join("notes/quotation.txt")).unwrap(),
        quote
    );
    assert_eq!(
        fs::read_to_string(edited.join("src/main.rs")).unwrap(),
        source
    );

    let _ = fs::remove_dir_all(root);
}
