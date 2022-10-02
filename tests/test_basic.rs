//! Test the `ls` command from the local system

use std::{fs::read, io::Write, process::Command, thread::spawn};

use tempfile::NamedTempFile;

use memfd_exec::{MemFdExecutable, Stdio};

#[test]
fn test_ls() {
    let ls_contents = read("/bin/ls").unwrap();
    let ls = MemFdExecutable::new("ls", ls_contents)
        .arg(".")
        .spawn()
        .unwrap();
}

#[test]
fn test_cat_simple() {
    let cat_contents = read("/bin/cat").unwrap();
    let _cat = MemFdExecutable::new("cat", cat_contents)
        .arg("Cargo.toml")
        .spawn()
        .unwrap();
}

#[test]
fn test_cat_stdin() {
    let cat_contents = read("/bin/cat").unwrap();
    let mut cat = MemFdExecutable::new("cat", cat_contents.clone())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    let mut stdin = cat.stdin.take().expect("Failed to open stdin");
    spawn(move || {
        stdin.write(b"Hello, world!").unwrap();
    });

    let output = cat.wait_with_output().unwrap();

    assert!(
        output.stdout.len() == b"Hello, world!".len(),
        "Output was too short (wanted at least {} bytes, got {})",
        cat_contents.len(),
        output.stdout.len()
    );
}

#[test]
fn test_static_included() {
    let tfp = NamedTempFile::new().unwrap();
    let tfp_path = tfp.into_temp_path();
    let mut clang = Command::new("clang")
        .arg("-x")
        .arg("c")
        .arg("-o")
        .arg(tfp_path.as_os_str())
        .arg("-")
        .stdin(std::process::Stdio::piped())
        .spawn()
        .expect("Failed to run clang");

    let mut clang_stdin = clang.stdin.take().expect("Failed to open stdin");

    spawn(move || {
        clang_stdin
            .write_all(include_bytes!("test_static.c"))
            .unwrap();
    });
}
