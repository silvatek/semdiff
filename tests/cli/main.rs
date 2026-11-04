mod apply;
mod rename;
mod semdif;

use std::fs;
use std::process::Command;

fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_semdiff"))
        .args(args)
        .output()
        .expect("failed to run semdiff")
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
            "Semdiff {}\nComparing {left} and {right}\nsrc/main.rs\n",
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

#[test]
fn requires_an_output_path() {
    let output = run(&[]);
    let stderr = String::from_utf8(output.stderr).expect("stderr was not utf-8");

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        stderr,
        "Usage: semdiff <left> <right> <output>\n       semdiff --apply <semdif> <directory>\n"
    );
}

#[test]
fn rejects_a_missing_source() {
    let output = run(&["missing-left", "missing-right", "out.yml"]);
    let stderr = String::from_utf8(output.stderr).expect("stderr was not utf-8");

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stderr, "not a file or directory: missing-left\n");
}

#[test]
fn rejects_a_file_compared_with_a_directory() {
    let root = std::env::temp_dir().join(format!("semdiff-mixed-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let file = root.join("quote.txt");
    fs::write(&file, "Brevity is the soul of wit.\n").unwrap();

    let file = file.to_str().unwrap();
    let directory = root.to_str().unwrap();
    let output = run(&[file, directory, "out.yml"]);
    let stderr = String::from_utf8(output.stderr).expect("stderr was not utf-8");

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        stderr,
        "sources must both be files or both be directories\n"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rejects_apply_to_a_file() {
    let root = std::env::temp_dir().join(format!("semdiff-apply-file-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let file = root.join("quote.txt");
    let semdif = root.join("out.yml");
    fs::write(&file, "Brevity is the soul of wit.\n").unwrap();
    fs::write(&semdif, "header:\n  semdiff: \"0.1.0\"\n").unwrap();

    let file = file.to_str().unwrap();
    let semdif = semdif.to_str().unwrap();
    let output = run(&["--apply", semdif, file]);
    let stderr = String::from_utf8(output.stderr).expect("stderr was not utf-8");

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stderr, format!("not a directory: {file}\n"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rejects_a_missing_semdif_file() {
    let root = std::env::temp_dir().join(format!("semdiff-apply-missing-{}", std::process::id()));
    let source = root.join("source");
    fs::create_dir_all(&source).unwrap();
    let semdif = root.join("missing.yml");

    let source = source.to_str().unwrap();
    let semdif_arg = semdif.to_str().unwrap();
    let output = run(&["--apply", semdif_arg, source]);
    let stderr = String::from_utf8(output.stderr).expect("stderr was not utf-8");

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stderr, format!("not a file: {}\n", semdif.display()));

    let _ = fs::remove_dir_all(root);
}
