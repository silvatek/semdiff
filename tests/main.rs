//! Tests of the process entry, including the helper that runs semdiff.

use std::fs;
use std::process::Command;

pub(crate) fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_semdiff"))
        .args(args)
        .output()
        .expect("failed to run semdiff")
}

pub(crate) fn run_with_stdin(args: &[&str], stdin: &[u8]) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_semdiff"))
        .args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("failed to run semdiff");
    std::io::Write::write_all(child.stdin.as_mut().expect("stdin"), stdin).expect("write stdin");
    child.wait_with_output().expect("failed to run semdiff")
}

#[test]
fn prints_paths_present_in_both_directories() {
    let root = std::env::temp_dir().join(format!("semdiff-pairs-{}", std::process::id()));
    let left = root.join("left");
    let right = root.join("right");
    fs::create_dir_all(left.join("src")).unwrap();
    fs::create_dir_all(right.join("src")).unwrap();
    fs::write(left.join("src/main.rs"), "fn fibonacci() {}\n").unwrap();
    fs::write(right.join("src/main.rs"), "fn fib() {}\n").unwrap();
    fs::write(left.join("only-left.rs"), "").unwrap();
    fs::create_dir_all(left.join("target")).unwrap();
    fs::write(left.join("target/skip.txt"), "").unwrap();
    fs::create_dir_all(right.join("target")).unwrap();
    fs::write(right.join("target/skip.txt"), "").unwrap();

    let output_path = root.join("out.yml");
    let left = left.to_str().unwrap();
    let right = right.to_str().unwrap();
    let output_path = output_path.to_str().unwrap();
    let output = run(&[left, right, output_path]);
    let stdout = String::from_utf8(output.stdout).expect("stdout was not utf-8");

    assert!(output.status.success());
    assert_eq!(
        stdout,
        format!(
            "Semdiff {}\nComparing {left} and {right}\nsrc/main.rs\ntarget/skip.txt\n",
            env!("CARGO_PKG_VERSION")
        )
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn compares_individual_files() {
    let root = std::env::temp_dir().join(format!("semdiff-files-{}", std::process::id()));
    let left_dir = root.join("left");
    let right_dir = root.join("right");
    fs::create_dir_all(&left_dir).unwrap();
    fs::create_dir_all(&right_dir).unwrap();
    fs::write(left_dir.join("quote.txt"), "Brevity is the soul of wit.\n").unwrap();
    fs::write(right_dir.join("quote.txt"), "Brevity is the soul of wit.\n").unwrap();

    let output_path = root.join("out.yml");
    let left = left_dir.join("quote.txt");
    let right = right_dir.join("quote.txt");
    let left = left.to_str().unwrap();
    let right = right.to_str().unwrap();
    let output_path = output_path.to_str().unwrap();
    let output = run(&[left, right, output_path]);
    let stdout = String::from_utf8(output.stdout).expect("stdout was not utf-8");
    let document = fs::read_to_string(output_path).expect("failed to read semdiff file");

    assert!(output.status.success());
    assert!(stdout.ends_with("quote.txt\n"), "{stdout}");
    assert!(!document.contains("changes:"));

    let _ = fs::remove_dir_all(root);
}
