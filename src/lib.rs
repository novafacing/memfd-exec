//! This is a very simple crate that allows execution of in-memory only programs. Simply
//! put, if you have the contents of a Linux executable in a `Vec<u8>`, you can use
//! `memfd_exec` to execute the program without it ever touching your hard disk. Use
//! cases for this may include:
//!
//! * Bundling a static executable with another program (for example, my motivation to
//!   create this package is that I want to ship a statically built QEMU with
//!   [cantrace](https://github.com/novafacing/cannoli))
//! * Sending executables over the network and running them, to reduce footprint and increase
//!   throughput
//!
//! # Example
//!
//! The following example will download and run a qemu-x86_64 executable from the internet,
//! without ever writing the executable to disk.
//!
//! ```
//! use memfd_exec::{MemFdExecutable, Stdio};
//! use reqwest::blocking::get;
//!
//! const URL: &str = "https://novafacing.github.io/assets/qemu-x86_64";
//! let resp = get(URL).unwrap();
//!
//! // The `MemFdExecutable` struct is at near feature-parity with `std::process::Command`,
//! // so you can use it in the same way. The only difference is that you must provide the
//! // executable contents as a `Vec<u8>` as well as telling it the argv[0] to use.
//! let qemu = MemFdExecutable::new("qemu-x86_64", resp.bytes().unwrap().as_ref())
//!     // We'll just get the version here, but you can do anything you want with the
//!     // args.
//!     .arg("-version")
//!     // We'll capture the stdout of the process, so we need to set up a pipe.
//!     .stdout(Stdio::piped())
//!     // Spawn the process as a forked child
//!     .spawn()
//!     .unwrap();
//!
//! // Get the output and status code of the process (this will block until the process
//! // exits)
//! let output = qemu.wait_with_output().unwrap();
//! assert!(output.status.into_raw() == 0);
//! // Print out the version we got!
//! println!("{}", String::from_utf8_lossy(&output.stdout));
//! ```
//!
// #![feature(exact_size_is_empty)]
// #![feature(exit_status_error)]
// #![feature(raw_os_nonzero)]
// #![feature(read_buf)]
// #![feature(can_vector)]
// #![feature(never_type)]

mod anon_pipe;
mod child;
mod command_env;
mod cvt;
mod executable;
mod file_desc;
mod output;
mod process;
mod stdio;

pub use child::{Child, ChildStderr, ChildStdin, ChildStdout};
pub use executable::MemFdExecutable;
pub use output::Output;
pub use stdio::Stdio;
