//! This library provides essentially a single functionality to execute an in-memory executable
//! as if it were a process::Command run normally from the filesystem
#![feature(exact_size_is_empty)]

mod command_env;
mod cvt;
mod file_desc;

use std::{
    ffi::{c_char, CString},
    process::Stdio,
};

use nix::{
    sys::memfd::{memfd_create, MemFdCreateFlag},
    unistd::{fexecve, pipe2},
};

use command_env::CommandEnv;

struct Executable {
    program: Vec<u8>,
    args: Vec<CString>,
    argv: Argv,
    env: CommandEnv,
    cwd: Option<CString>,
    stdin: Option<Stdio>,
    stdout: Option<Stdio>,
    stderr: Option<Stdio>,
}

struct Argv(Vec<*const c_char>);

unsafe impl Send for Argv {}
unsafe impl Sync for Argv {}

struct StdioPipes {
    pub stdin: Option<AnonPipe>,
    pub stdout: Option<AnonPipe>,
    pub stderr: Option<AnonPipe>,
}

impl Executable {
    pub fn new(prog: &[u8]) -> Self {
        Self { prog }
    }
}
