//! This library provides essentially a single functionality to execute an in-memory executable
//! as if it were a process::Command run normally from the filesystem
#![feature(exact_size_is_empty)]
#![feature(exit_status_error)]
#![feature(raw_os_nonzero)]
#![feature(read_buf)]
#![feature(can_vector)]
#![feature(never_type)]

mod anon_pipe;
mod child;
mod command_env;
mod cvt;
mod executable;
mod file_desc;
mod output;
mod process;
mod stdio;

pub use executable::MemFdExecutable;
pub use stdio::Stdio;
