//! Test the `ls` command from the local system

use std::{
    fs::read,
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    path::PathBuf,
    process::{Command, Stdio as ProcessStdio},
    str,
    thread::{sleep, spawn},
    time::Duration,
};

use serial_test::serial;

use memfd_exec::{MemFdExecutable, Stdio};

const TEST_STATIC_CODE: &[u8] = include_bytes!("./test_static.c");
const CARGO_TARGET_TMPDIR: &str = env!("CARGO_TARGET_TMPDIR");

fn build_test_static() {
    let mut clang = Command::new("clang")
        .arg("-x")
        .arg("c")
        .arg("-static")
        .arg("-v")
        .arg("-o")
        .arg(PathBuf::from(CARGO_TARGET_TMPDIR).join("test_static.bin"))
        .arg("-")
        .stdin(ProcessStdio::piped())
        .stdout(ProcessStdio::piped())
        .stderr(ProcessStdio::piped())
        .spawn()
        .expect("Failed to run clang");

    let mut clang_stdin = clang.stdin.take().expect("Failed to open stdin");

    spawn(move || {
        clang_stdin
            .write_all(TEST_STATIC_CODE)
            .expect("Could not write to clang stdin");
    });

    let output = clang.wait_with_output().expect("Failed to run clang");

    assert!(
        output.status.success(),
        "Failed to compile static test:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn build_test_dynamic() {
    let mut clang = Command::new("clang")
        .arg("-x")
        .arg("c")
        .arg("-o")
        .arg(PathBuf::from(CARGO_TARGET_TMPDIR).join("test_dynamic.bin"))
        .arg("-")
        .stdin(ProcessStdio::piped())
        .stdout(ProcessStdio::piped())
        .stderr(ProcessStdio::piped())
        .spawn()
        .expect("Failed to run clang");

    let mut clang_stdin = clang.stdin.take().expect("Failed to open stdin");

    spawn(move || {
        clang_stdin
            .write_all(TEST_STATIC_CODE)
            .expect("Could not write to clang stdin");
    });

    let output = clang.wait_with_output().expect("Failed to run clang");

    assert!(
        output.status.success(),
        "Failed to compile static test:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn test_ls() {
    let ls_contents = read("/bin/ls").expect("Could not read /bin/ls");
    let _ls = MemFdExecutable::new("ls", &ls_contents)
        .arg(".")
        .spawn()
        .expect("Failed to run ls");
}

#[test]
fn test_cat_simple() {
    let cat_contents = read("/bin/cat").expect("Could not read /bin/cat");
    let _cat = MemFdExecutable::new("cat", &cat_contents)
        .arg("Cargo.toml")
        .spawn()
        .expect("Failed to run cat");
}

#[test]
fn test_cat_stdin() {
    let cat_contents = read("/bin/cat").expect("Could not read /bin/cat");
    let mut cat = MemFdExecutable::new("cat", &cat_contents)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to run cat");

    let mut stdin = cat.stdin.take().expect("Failed to open stdin");
    spawn(move || {
        stdin
            .write_all(b"Hello, world!")
            .expect("Failed to write to cat stdin");
    });

    let output = cat.wait_with_output().expect("Failed to run cat");

    assert!(
        output.stdout.len() == b"Hello, world!".len(),
        "Output was too short (wanted at least {} bytes, got {})",
        b"Hello, world!".len(),
        output.stdout.len()
    );
}

#[test]
#[serial]
fn test_static_included() {
    build_test_static();

    let test_static_exe = PathBuf::from(CARGO_TARGET_TMPDIR).join("test_static.bin");
    let test_static_exe_contents = read(test_static_exe).expect("Could not read static exe");

    let test_static = MemFdExecutable::new("test_static.bin", &test_static_exe_contents)
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to spawn test_static");

    let port = {
        let mut array = [0u8; 32];
        let len = test_static
            .stdout
            .as_ref()
            .unwrap()
            .read(&mut array)
            .unwrap();
        let port: u16 = str::from_utf8(&array[..len]).unwrap().parse().unwrap();
        port
    };

    let output_thread = spawn(move || {
        let output = test_static
            .wait_with_output()
            .expect("Failed to run test_static");
        assert!(
            output.stdout.len() == b"Hello, world!\n\n".len(),
            "Output was too short (wanted at least {} bytes, got {})",
            b"Hello, world!".len(),
            output.stdout.len()
        );
    });

    let sock: SocketAddr = format!("127.0.0.1:{}", port)
        .parse()
        .expect("Failed to parse socket address");

    let mut stream = TcpStream::connect(sock).unwrap();
    stream
        .write_all(b"Hello, world!\n\n")
        .expect("Failed to write to socket");
    drop(stream);

    output_thread.join().expect("Failed to join output thread");
}

#[test]
#[serial]
fn test_dynamic_included() {
    build_test_dynamic();

    let test_dynamic_exe = PathBuf::from(CARGO_TARGET_TMPDIR).join("test_dynamic.bin");
    let test_dynamic_exe_contents = read(test_dynamic_exe).expect("Could not read dynamic exe");

    let test_dynamic = MemFdExecutable::new("test_dynamic.bin", &test_dynamic_exe_contents)
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to spawn test_dynamic");

    let port = {
        let mut array = [0u8; 32];
        let len = test_dynamic
            .stdout
            .as_ref()
            .unwrap()
            .read(&mut array)
            .unwrap();
        let port: u16 = str::from_utf8(&array[..len]).unwrap().parse().unwrap();
        port
    };

    let output_thread = spawn(move || {
        let output = test_dynamic
            .wait_with_output()
            .expect("Failed to run test_dynamic");
        assert!(
            output.stdout.len() == b"Hello, world!\n\n".len(),
            "Output was too short (wanted at least {} bytes, got {})",
            b"Hello, world!".len(),
            output.stdout.len()
        );
    });

    let sock: SocketAddr = format!("127.0.0.1:{}", port)
        .parse()
        .expect("Failed to parse socket address");

    for _ in 0..10 {
        if let Ok(mut stream) = TcpStream::connect(sock) {
            stream
                .write_all(b"Hello, world!\n\n")
                .expect("Failed to write to socket");
            drop(stream);
            break;
        }
        sleep(Duration::from_millis(100));
    }
    output_thread.join().expect("Failed to join output thread");
}

// #[test]
// fn test_net() {
//     use memfd_exec::{MemFdExecutable, Stdio};
//     use reqwest::blocking::get;
//
//     const URL: &str = "https://novafacing.github.io/assets/qemu-x86_64";
//     let resp = get(URL).expect("Failed to download qemu");
//
//     // The `MemFdExecutable` struct is at near feature-parity with `std::process::Command`,
//     // so you can use it in the same way. The only difference is that you must provide the
//     // executable contents as a `Vec<u8>` as well as telling it the argv[0] to use.
//     let qemu = MemFdExecutable::new(
//         "qemu-x86_64",
//         resp.bytes()
//             .expect("Could not get bytes from qemu download")
//     )
//     // We'll just get the version here, but you can do anything you want with the
//     // args.
//     .arg("-version")
//     // We'll capture the stdout of the process, so we need to set up a pipe.
//     .stdout(Stdio::piped())
//     // Spawn the process as a forked child
//     .spawn()
//     .expect("Failed to spawn qemu");
//
//     // Get the output and status code of the process (this will block until the process
//     // exits)
//     let output = qemu.wait_with_output().expect("Failed to run qemu");
//     assert!(output.status.into_raw() == 0);
//     // Print out the version we got!
//     println!("{}", String::from_utf8_lossy(&output.stdout));
// }
