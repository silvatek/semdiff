use std::fs;
use std::process::Command;

fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_semdiff"))
        .args(args)
        .output()
        .expect("failed to run semdiff")
}

#[test]
fn prints_name_and_version_when_comparing_directories() {
    let root = std::env::temp_dir().join(format!("semdiff-cli-{}", std::process::id()));
    let left = root.join("left");
    let right = root.join("right");
    fs::create_dir_all(&left).unwrap();
    fs::create_dir_all(&right).unwrap();

    let left = left.to_str().unwrap();
    let right = right.to_str().unwrap();
    let output = run(&[left, right]);
    let stdout = String::from_utf8(output.stdout).expect("stdout was not utf-8");

    assert!(output.status.success());
    assert_eq!(
        stdout,
        format!(
            "Semdiff {}\nComparing {left} and {right}\n",
            env!("CARGO_PKG_VERSION")
        )
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn requires_two_directories() {
    let output = run(&[]);
    let stderr = String::from_utf8(output.stderr).expect("stderr was not utf-8");

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(stderr, "Usage: semdiff <left> <right>\n");
}

#[test]
fn rejects_a_missing_directory() {
    let output = run(&["missing-left", "missing-right"]);
    let stderr = String::from_utf8(output.stderr).expect("stderr was not utf-8");

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stderr, "not a directory: missing-left\n");
}
