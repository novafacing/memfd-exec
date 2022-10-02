use std::{env::var, fs::read, io::Write, process::Command, thread::spawn};

fn build_test_static() {
    let mut clang = Command::new("clang")
        .arg("-x")
        .arg("c")
        .arg("-static")
        .arg("-o")
        .arg(var("OUT_DIR").unwrap() + "test_static.bin")
        .arg("-")
        .stdin(std::process::Stdio::piped())
        .spawn()
        .expect("Failed to run clang");

    let mut clang_stdin = clang.stdin.take().expect("Failed to open stdin");

    spawn(move || {
        clang_stdin
            .write_all(read("tests/test_static.c").unwrap().as_slice())
            .unwrap();
    });

    clang.wait_with_output().expect("Failed to run clang");
}

fn build_test_dynamic() {
    let mut clang = Command::new("clang")
        .arg("-x")
        .arg("c")
        .arg("-o")
        .arg(var("OUT_DIR").unwrap() + "test_dynamic.bin")
        .arg("-")
        .stdin(std::process::Stdio::piped())
        .spawn()
        .expect("Failed to run clang");

    let mut clang_stdin = clang.stdin.take().expect("Failed to open stdin");

    spawn(move || {
        clang_stdin
            .write_all(read("tests/test_static.c").unwrap().as_slice())
            .unwrap();
    });

    clang.wait_with_output().expect("Failed to run clang");
}

fn main() {
    /* Build executables for testing */
    build_test_dynamic();
    build_test_static();
}
