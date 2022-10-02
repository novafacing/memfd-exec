use std::ffi::CStr;
use std::fs::{File, OpenOptions};
use std::io::Result;
use std::os::raw::c_int;
use std::os::unix::prelude::{AsRawFd, FromRawFd};
use std::path::Path;

use crate::anon_pipe::{anon_pipe, AnonPipe};
use crate::file_desc::FileDesc;

const DEV_NULL: &str = "/dev/null\0";

pub struct StdioPipes {
    pub stdin: Option<AnonPipe>,
    pub stdout: Option<AnonPipe>,
    pub stderr: Option<AnonPipe>,
}

pub struct ChildPipes {
    pub stdin: ChildStdio,
    pub stdout: ChildStdio,
    pub stderr: ChildStdio,
}

pub enum ChildStdio {
    Inherit,
    Explicit(c_int),
    Owned(FileDesc),
}

pub enum Stdio {
    Inherit,
    Null,
    MakePipe,
    Fd(FileDesc),
}

impl Stdio {
    pub fn to_child_stdio(&self, readable: bool) -> Result<(ChildStdio, Option<AnonPipe>)> {
        match *self {
            Stdio::Inherit => Ok((ChildStdio::Inherit, None)),

            // Make sure that the source descriptors are not an stdio
            // descriptor, otherwise the order which we set the child's
            // descriptors may blow away a descriptor which we are hoping to
            // save. For example, suppose we want the child's stderr to be the
            // parent's stdout, and the child's stdout to be the parent's
            // stderr. No matter which we dup first, the second will get
            // overwritten prematurely.
            Stdio::Fd(ref fd) => {
                if fd.as_raw_fd() >= 0 && fd.as_raw_fd() <= libc::STDERR_FILENO {
                    Ok((ChildStdio::Owned(fd.duplicate()?), None))
                } else {
                    Ok((ChildStdio::Explicit(fd.as_raw_fd()), None))
                }
            }

            Stdio::MakePipe => {
                let (reader, writer) = anon_pipe()?;
                let (ours, theirs) = if readable {
                    (writer, reader)
                } else {
                    (reader, writer)
                };
                Ok((ChildStdio::Owned(theirs.into()), Some(ours)))
            }

            Stdio::Null => {
                let mut opts = OpenOptions::new();
                opts.read(readable);
                opts.write(!readable);
                let path = unsafe { CStr::from_ptr(DEV_NULL.as_ptr() as *const _) };
                let path = Path::new(path.to_str().unwrap());
                let fd = File::open(path)?.as_raw_fd();
                Ok((
                    ChildStdio::Owned(unsafe { FileDesc::from_raw_fd(fd) }),
                    None,
                ))
            }
        }
    }

    /// Create a pipe for this file descriptor and use it in the child process as
    /// the given file descriptor to facilitate input or output redirection. See
    /// `MemFdExecutable::stdin` for an example.
    pub fn piped() -> Stdio {
        Stdio::MakePipe
    }

    /// Use a null file descriptor, like /dev/null, to either provide no input or to
    /// discard output. See `MemFdExecutable::stdout` for an example.
    pub fn null() -> Stdio {
        Stdio::Null
    }

    /// Inherit the parent's file descriptor. this is the default behavior, but is
    /// generally not the desired behavior.
    pub fn inherit() -> Stdio {
        Stdio::Inherit
    }
}

impl From<AnonPipe> for Stdio {
    fn from(pipe: AnonPipe) -> Stdio {
        Stdio::Fd(pipe.into())
    }
}

impl ChildStdio {
    pub fn fd(&self) -> Option<c_int> {
        match *self {
            ChildStdio::Inherit => None,
            ChildStdio::Explicit(fd) => Some(fd),
            ChildStdio::Owned(ref fd) => Some(fd.as_raw_fd()),
        }
    }
}
