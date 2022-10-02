use std::fmt::{Debug, Formatter, Result};
use std::str::from_utf8;

use crate::process::ExitStatus;

#[derive(PartialEq, Clone, Eq)]
pub struct Output {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

impl Debug for Output {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result {
        let stdout_utf8 = from_utf8(&self.stdout);
        let stdout_debug: &dyn Debug = match stdout_utf8 {
            Ok(ref str) => str,
            Err(_) => &self.stdout,
        };

        let stderr_utf8 = from_utf8(&self.stderr);
        let stderr_debug: &dyn Debug = match stderr_utf8 {
            Ok(ref str) => str,
            Err(_) => &self.stderr,
        };

        fmt.debug_struct("Output")
            .field("status", &self.status)
            .field("stdout", stdout_debug)
            .field("stderr", stderr_debug)
            .finish()
    }
}
