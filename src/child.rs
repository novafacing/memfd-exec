use std::fmt::{Debug, Formatter, Result as FmtResult};
use std::io::{Error, ErrorKind, IoSlice, IoSliceMut, Read, Result, Write};

use crate::anon_pipe::{read2, AnonPipe};
use crate::output::Output;
use crate::process::{ExitStatus, Process};
use crate::stdio::StdioPipes;

/// A child process created from a `MemFdExecutable` with handles to input and output streams
pub struct Child {
    handle: Process,
    /// The input stream to the child process
    pub stdin: Option<ChildStdin>,
    /// The output stream from the child process
    pub stdout: Option<ChildStdout>,
    /// The error stream from the child process
    pub stderr: Option<ChildStderr>,
}

impl Child {
    pub fn new(handle: Process, stdio: StdioPipes) -> Self {
        Self {
            handle,
            stdin: stdio.stdin.map(ChildStdin),
            stdout: stdio.stdout.map(ChildStdout),
            stderr: stdio.stderr.map(ChildStderr),
        }
    }

    /// Kill the child process
    pub fn kill(&mut self) -> Result<()> {
        self.handle.kill()
    }

    /// Return the id of the child process, probably a PID
    pub fn id(&self) -> u32 {
        self.handle.id()
    }

    /// Wait for the child process to exit, returning the exit status code
    pub fn wait(&mut self) -> Result<ExitStatus> {
        drop(self.stdin.take());
        self.handle.wait()
    }

    /// Try and wait for the child process to exit, returning the exit status code if it has
    pub fn try_wait(&mut self) -> Result<Option<ExitStatus>> {
        self.handle.try_wait()
    }

    /// Wait for the child process to exit, returning the exit status code and the output
    /// streams
    pub fn wait_with_output(mut self) -> Result<Output> {
        drop(self.stdin.take());

        let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
        match (self.stdout.take(), self.stderr.take()) {
            (None, None) => {}
            (Some(mut out), None) => {
                out.read_to_end(&mut stdout)?;
            }
            (None, Some(mut err)) => {
                err.read_to_end(&mut stderr)?;
            }
            (Some(out), Some(err)) => {
                read2(out.0, &mut stdout, err.0, &mut stderr)?;
            }
        }

        Ok(Output {
            status: self.wait()?,
            stdout,
            stderr,
        })
    }
}

/// A handle to a child process’s standard input (stdin).
pub struct ChildStdin(AnonPipe);

impl std::os::fd::AsRawFd for ChildStdin {
    #[inline]
    fn as_raw_fd(&self) -> std::os::fd::RawFd {
        self.0.as_raw_fd()
    }
}

impl Write for ChildStdin {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        (&*self).write(buf)
    }

    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> Result<usize> {
        (&*self).write_vectored(bufs)
    }

    fn flush(&mut self) -> Result<()> {
        (&*self).flush()
    }

    fn write_all(&mut self, buf: &[u8]) -> Result<()> {
        (&*self).write_all(buf)
    }
}

impl Write for &ChildStdin {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.0.write(buf)
    }

    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> Result<usize> {
        self.0.write_vectored(bufs)
    }

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }

    fn write_all(&mut self, mut buf: &[u8]) -> Result<()> {
        while !buf.is_empty() {
            match self.write(buf) {
                Ok(0) => {
                    return Err(Error::new(
                        ErrorKind::WriteZero,
                        "failed to write whole buffer",
                    ))
                }
                Ok(n) => buf = &buf[n..],
                Err(ref e) if e.kind() == ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
}

impl Debug for ChildStdin {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("ChildStdin").finish_non_exhaustive()
    }
}

/// A handle to a child process’s standard output (stdout).
pub struct ChildStdout(AnonPipe);

impl std::os::fd::AsRawFd for ChildStdout {
    #[inline]
    fn as_raw_fd(&self) -> std::os::fd::RawFd {
        self.0.as_raw_fd()
    }
}

impl Write for ChildStdout {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        (&*self).write(buf)
    }

    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> Result<usize> {
        (&*self).write_vectored(bufs)
    }

    fn flush(&mut self) -> Result<()> {
        (&*self).flush()
    }

    fn write_all(&mut self, buf: &[u8]) -> Result<()> {
        (&*self).write_all(buf)
    }
}

impl Write for &ChildStdout {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.0.write(buf)
    }

    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> Result<usize> {
        self.0.write_vectored(bufs)
    }

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }

    fn write_all(&mut self, mut buf: &[u8]) -> Result<()> {
        while !buf.is_empty() {
            match self.write(buf) {
                Ok(0) => {
                    return Err(Error::new(
                        ErrorKind::WriteZero,
                        "failed to write whole buffer",
                    ))
                }
                Ok(n) => buf = &buf[n..],
                Err(ref e) if e.kind() == ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
}

impl Read for ChildStdout {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        (&*self).read(buf)
    }

    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> Result<usize> {
        (&*self).read_vectored(bufs)
    }

    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> Result<usize> {
        (&*self).read_to_end(buf)
    }
}

impl Read for &ChildStdout {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.0.read(buf)
    }

    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> Result<usize> {
        self.0.read_vectored(bufs)
    }

    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> Result<usize> {
        self.0.read_to_end(buf)
    }
}

impl Debug for ChildStdout {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("ChildStdout").finish_non_exhaustive()
    }
}

/// A handle to a child process’s stderr.
pub struct ChildStderr(AnonPipe);

impl std::os::fd::AsRawFd for ChildStderr {
    #[inline]
    fn as_raw_fd(&self) -> std::os::fd::RawFd {
        self.0.as_raw_fd()
    }
}

impl Write for ChildStderr {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        (&*self).write(buf)
    }

    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> Result<usize> {
        (&*self).write_vectored(bufs)
    }

    fn flush(&mut self) -> Result<()> {
        (&*self).flush()
    }

    fn write_all(&mut self, buf: &[u8]) -> Result<()> {
        (&*self).write_all(buf)
    }
}

impl Write for &ChildStderr {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.0.write(buf)
    }

    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> Result<usize> {
        self.0.write_vectored(bufs)
    }

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }

    fn write_all(&mut self, mut buf: &[u8]) -> Result<()> {
        while !buf.is_empty() {
            match self.write(buf) {
                Ok(0) => {
                    return Err(Error::new(
                        ErrorKind::WriteZero,
                        "failed to write whole buffer",
                    ))
                }
                Ok(n) => buf = &buf[n..],
                Err(ref e) if e.kind() == ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
}

impl Read for ChildStderr {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        (&*self).read(buf)
    }

    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> Result<usize> {
        (&*self).read_vectored(bufs)
    }

    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> Result<usize> {
        (&*self).read_to_end(buf)
    }
}

impl Read for &ChildStderr {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.0.read(buf)
    }

    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> Result<usize> {
        self.0.read_vectored(bufs)
    }

    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> Result<usize> {
        self.0.read_to_end(buf)
    }
}

impl Debug for ChildStderr {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("ChildStderr").finish_non_exhaustive()
    }
}

impl Debug for Child {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("Child")
            .field("stdin", &self.stdin)
            .field("stdout", &self.stdout)
            .field("stderr", &self.stderr)
            .finish_non_exhaustive()
    }
}
