//! Test the `ls` command from the local system

use std::{
    fs::read,
    io::Write,
    net::{SocketAddr, TcpStream},
    thread::{sleep, spawn},
    time::Duration,
};

use memfd_exec::{MemFdExecutable, Stdio};

#[test]
fn test_ls() {
    let ls_contents = read("/bin/ls").unwrap();
    let _ls = MemFdExecutable::new("ls", ls_contents)
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
        b"Hello, world!".len(),
        output.stdout.len()
    );
}

const TEST_STATIC_EXE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "test_static.bin"));
const TEST_DYN_EXE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "test_dynamic.bin"));

#[test]
fn test_static_included() {
    const PORT: u32 = 5432;
    let test_static = MemFdExecutable::new("test_static.bin", TEST_STATIC_EXE.to_vec())
        .arg(format!("{}", PORT))
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    let output_thread = spawn(move || {
        let output = test_static.wait_with_output().unwrap();
        assert!(
            output.stdout.len() == b"Hello, world!\n\n".len(),
            "Output was too short (wanted at least {} bytes, got {})",
            b"Hello, world!".len(),
            output.stdout.len()
        );
    });

    let sock: SocketAddr = format!("127.0.0.1:{}", PORT).parse().unwrap();

    for _ in 0..10 {
        if let Ok(mut stream) = TcpStream::connect(&sock) {
            stream.write(b"Hello, world!\n\n").unwrap();
            drop(stream);
            break;
        }
        sleep(Duration::from_millis(100));
    }
    output_thread.join().unwrap();
}

#[test]
fn test_dynamic_included() {
    const PORT: u32 = 2345;
    let test_static = MemFdExecutable::new("test_dynamic.bin", TEST_DYN_EXE.to_vec())
        .arg(format!("{}", PORT))
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    let output_thread = spawn(move || {
        let output = test_static.wait_with_output().unwrap();
        assert!(
            output.stdout.len() == b"Hello, world!\n\n".len(),
            "Output was too short (wanted at least {} bytes, got {})",
            b"Hello, world!".len(),
            output.stdout.len()
        );
    });

    let sock: SocketAddr = format!("127.0.0.1:{}", PORT).parse().unwrap();

    for _ in 0..10 {
        if let Ok(mut stream) = TcpStream::connect(&sock) {
            stream.write(b"Hello, world!\n\n").unwrap();
            drop(stream);
            break;
        }
        sleep(Duration::from_millis(100));
    }
    output_thread.join().unwrap();
}
