# memfd_exec ![crates.io](https://img.shields.io/crates/v/memfd-exec.svg)

This is a very simple crate that allows execution of in-memory only programs. Simply
put, if you have the contents of a Linux executable in a `Vec<u8>`, you can use
`memfd_exec` to execute the program without it ever touching your hard disk. Use
cases for this may include:

* Bundling a static executable with another program (for example, my motivation to
  create this package is that I want to ship a statically built QEMU with
  [cantrace](https://github.com/novafacing/cannoli))
* Sending executables over the network and running them, to reduce footprint and increase
  throughput
* Really hacky stuff that I haven't thought of, if you have a cool use case, feel free
  to make a PR to the README or add an example in [examples](examples)

## Using

Just include `memfd-exec = "VERSION"` in your `Cargo.toml` file.

## Features

* Feature-parity API with `process::Command`, the only difference is we don't execute
  anything from disk.
* Only two dependencies

## Example

The motivating example for this project is to bundle an executable along with a rust
program and be able to run the executable straight from memory instead of going
through the tedious and slow process of writing the executable file to disk and then
invoking it as a command.

This example creates an executable with a bundled [program](tests/test_static.c) that
opens a socket, reads a bit of input, and then prints out the input. Of course, the
logical extension of the idea would be to use a static
[netcat](https://github.com/openbsd/src/blob/master/usr.bin/nc/netcat.c) build or some
such thing.

```rust

use memfd_exec::{MemFdExecutable, Stdio};

const EXECUTABLE_FILE: &[u8] = include_bytes!("tets/test_static");

fn main() {
    const PORT = 1234;
    // We create an in-memory executable with an argv[0] "test" and an executable file
    // that we embedded in our rust binary
    let exe = MemFdExecutable::new("test", EXECUTABLE_FILE.to_vec())
        // We pass one arg, the port number to listen on
        .arg(format!("{}", PORT))
        // We tell it to use a pipe for stdout (stdin and stderr will default to Stdio::inherit())
        .stdout(Stdio::piped())
        // We spawn the child process as a forked child process
        .spawn()
        .expect("Failed to create process!");

    // Wait until the process finishes and print its output
    let output = exe.wait_with_output().unwrap();
    println!("Got output: {:?}", output.stdout);
}
```
