use super::run;

use std::fs;

#[test]
fn writes_a_file_renamed_change() {
    let root = std::env::temp_dir().join(format!("semdiff-rename-{}", std::process::id()));
    let left = root.join("left");
    let right = root.join("right");
    fs::create_dir_all(&left).unwrap();
    fs::create_dir_all(&right).unwrap();
    let quote = "Brevity is the soul of wit.\n";
    fs::write(left.join("quote.txt"), quote).unwrap();
    fs::write(right.join("quotation.txt"), quote).unwrap();
    fs::write(left.join("notes.txt"), "left only\n").unwrap();
    fs::write(right.join("other.txt"), "right only\n").unwrap();

    let output_path = root.join("out.yml");
    let left = left.to_str().unwrap();
    let right = right.to_str().unwrap();
    let output_path = output_path.to_str().unwrap();
    let output = run(&[left, right, output_path]);
    let document = fs::read_to_string(output_path).expect("failed to read semdiff file");

    assert!(output.status.success());
    let changes = document.split_once("changes:\n").expect("changes list").1;
    assert_eq!(
        changes,
        "  - scope: \".\"\n    type: file-renamed\n    details:\n      from: \"quote.txt\"\n      to: \"quotation.txt\"\n"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn hashes_past_the_first_buffer() {
    let root = std::env::temp_dir().join(format!("semdiff-hash-{}", std::process::id()));
    let left = root.join("left");
    let right = root.join("right");
    fs::create_dir_all(&left).unwrap();
    fs::create_dir_all(&right).unwrap();

    let mut same = vec![1_u8; 9_000];
    same[8_500] = 7;
    let shared_prefix = vec![2_u8; 9_000];
    let mut shared_prefix_changed = shared_prefix.clone();
    shared_prefix_changed[8_500] = 4;
    fs::write(left.join("same.bin"), &same).unwrap();
    fs::write(right.join("same-renamed.bin"), &same).unwrap();
    fs::write(left.join("left.bin"), &shared_prefix).unwrap();
    fs::write(right.join("right.bin"), &shared_prefix_changed).unwrap();

    let output_path = root.join("out.yml");
    let left = left.to_str().unwrap();
    let right = right.to_str().unwrap();
    let output_path = output_path.to_str().unwrap();
    let output = run(&[left, right, output_path]);
    let document = fs::read_to_string(output_path).expect("failed to read semdiff file");

    assert!(output.status.success());
    let changes = document.split_once("changes:\n").expect("changes list").1;
    assert_eq!(
        changes,
        "  - scope: \".\"\n    type: file-renamed\n    details:\n      from: \"same.bin\"\n      to: \"same-renamed.bin\"\n"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn detects_a_rename_between_individual_files() {
    let root = std::env::temp_dir().join(format!("semdiff-file-rename-{}", std::process::id()));
    let left_dir = root.join("left");
    let right_dir = root.join("right");
    fs::create_dir_all(&left_dir).unwrap();
    fs::create_dir_all(&right_dir).unwrap();
    let quote = "Brevity is the soul of wit.\n";
    fs::write(left_dir.join("quote.txt"), quote).unwrap();
    fs::write(right_dir.join("quotation.txt"), quote).unwrap();

    let output_path = root.join("out.yml");
    let left = left_dir.join("quote.txt");
    let right = right_dir.join("quotation.txt");
    let left = left.to_str().unwrap();
    let right = right.to_str().unwrap();
    let output_path = output_path.to_str().unwrap();
    let output = run(&[left, right, output_path]);
    let document = fs::read_to_string(output_path).expect("failed to read semdiff file");

    assert!(output.status.success());
    let changes = document.split_once("changes:\n").expect("changes list").1;
    assert_eq!(
        changes,
        "  - scope: \".\"\n    type: file-renamed\n    details:\n      from: \"quote.txt\"\n      to: \"quotation.txt\"\n"
    );

    let _ = fs::remove_dir_all(root);
}
