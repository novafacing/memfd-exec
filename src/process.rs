//! This basically implements Process from:
//! <https://github.com/rust-lang/rust/blob/master/library/std/src/sys/unix/process/process_unix.rs>

use libc::c_int;
use std::fmt::{Debug, Formatter, Result as FmtResult};
use std::io::{Error, Result};

use libc::pid_t;

use crate::cvt::{cvt, cvt_r};

pub struct Process {
    pid: pid_t,
    status: Option<ExitStatus>,
}

impl Process {
    pub unsafe fn new(pid: pid_t) -> Self {
        // Safety: If `pidfd` is nonnegative, we assume it's valid and otherwise unowned.
        Process { pid, status: None }
    }

    pub fn id(&self) -> u32 {
        self.pid as u32
    }

    pub fn kill(&mut self) -> Result<()> {
        // If we've already waited on this process then the pid can be recycled
        // and used for another process, and we probably shouldn't be killing
        // random processes, so just return an error.
        if self.status.is_some() {
            Err(Error::new(
                std::io::ErrorKind::InvalidInput,
                "invalid argument: can't kill an exited process",
            ))
        } else {
            cvt(unsafe { libc::kill(self.pid, libc::SIGKILL) }).map(drop)
        }
    }

    pub fn wait(&mut self) -> Result<ExitStatus> {
        if let Some(status) = self.status {
            return Ok(status);
        }
        let mut status = 0 as c_int;
        cvt_r(|| unsafe { libc::waitpid(self.pid, &mut status, 0) })?;
        self.status = Some(ExitStatus::new(status));
        Ok(ExitStatus::new(status))
    }

    pub fn try_wait(&mut self) -> Result<Option<ExitStatus>> {
        if let Some(status) = self.status {
            return Ok(Some(status));
        }
        let mut status = 0 as c_int;
        let pid = cvt(unsafe { libc::waitpid(self.pid, &mut status, libc::WNOHANG) })?;
        if pid == 0 {
            Ok(None)
        } else {
            self.status = Some(ExitStatus::new(status));
            Ok(Some(ExitStatus::new(status)))
        }
    }
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub struct ExitStatus(c_int);

impl Debug for ExitStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_tuple("unix_wait_status").field(&self.0).finish()
    }
}

impl ExitStatus {
    pub fn new(status: c_int) -> ExitStatus {
        ExitStatus(status)
    }

    fn exited(&self) -> bool {
        libc::WIFEXITED(self.0)
    }

    pub fn exit_ok(&self) -> Result<()> {
        // This assumes that WIFEXITED(status) && WEXITSTATUS==0 corresponds to status==0.  This is
        // true on all actual versions of Unix, is widely assumed, and is specified in SuS
        // https://pubs.opengroup.org/onlinepubs/9699919799/functions/wait.html .  If it is not
        // true for a platform pretending to be Unix, the tests (our doctests, and also
        // procsss_unix/tests.rs) will spot it.  `ExitStatusError::code` assumes this too.
        match c_int::try_from(self.0) {
            /* was nonzero */
            Ok(failure) => Err(Error::new(
                std::io::ErrorKind::Other,
                format!("process exited with status {}", failure),
            )),
            /* was zero, couldn't convert */ Err(_) => Ok(()),
        }
    }

    pub fn code(&self) -> Option<i32> {
        self.exited().then(|| libc::WEXITSTATUS(self.0))
    }

    pub fn signal(&self) -> Option<i32> {
        libc::WIFSIGNALED(self.0).then(|| libc::WTERMSIG(self.0))
    }

    pub fn core_dumped(&self) -> bool {
        libc::WIFSIGNALED(self.0) && libc::WCOREDUMP(self.0)
    }

    pub fn stopped_signal(&self) -> Option<i32> {
        libc::WIFSTOPPED(self.0).then(|| libc::WSTOPSIG(self.0))
    }

    pub fn continued(&self) -> bool {
        libc::WIFCONTINUED(self.0)
    }

    pub fn into_raw(&self) -> c_int {
        self.0
    }
}

/// Converts a raw `c_int` to a type-safe `ExitStatus` by wrapping it without copying.
impl From<c_int> for ExitStatus {
    fn from(a: c_int) -> ExitStatus {
        ExitStatus(a)
    }
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub struct ExitStatusError(c_int);

impl Into<ExitStatus> for ExitStatusError {
    fn into(self) -> ExitStatus {
        ExitStatus(self.0.into())
    }
}

impl Debug for ExitStatusError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_tuple("unix_wait_status").field(&self.0).finish()
    }
}

impl ExitStatusError {}
