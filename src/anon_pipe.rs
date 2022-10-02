use std::{
    io::{IoSlice, IoSliceMut, Result},
    mem::zeroed,
    os::unix::prelude::{AsFd, AsRawFd, BorrowedFd, FromRawFd, IntoRawFd, RawFd},
};

use crate::{
    cvt::{cvt, cvt_r},
    file_desc::FileDesc,
};

pub struct AnonPipe(FileDesc);

pub fn anon_pipe() -> Result<(AnonPipe, AnonPipe)> {
    let mut fds = [0; 2];
    unsafe {
        cvt(libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC))?;
        Ok((
            AnonPipe(FileDesc::from_raw_fd(fds[0])),
            AnonPipe(FileDesc::from_raw_fd(fds[1])),
        ))
    }
}

impl AnonPipe {
    pub fn read(&self, buf: &mut [u8]) -> Result<usize> {
        self.0.read(buf)
    }

    pub fn read_vectored(&self, bufs: &mut [IoSliceMut<'_>]) -> Result<usize> {
        self.0.read_vectored(bufs)
    }

    #[inline]
    pub fn is_read_vectored(&self) -> bool {
        self.0.is_read_vectored()
    }

    pub fn write(&self, buf: &[u8]) -> Result<usize> {
        self.0.write(buf)
    }

    pub fn write_vectored(&self, bufs: &[IoSlice<'_>]) -> Result<usize> {
        self.0.write_vectored(bufs)
    }

    #[inline]
    pub fn is_write_vectored(&self) -> bool {
        self.0.is_write_vectored()
    }

    pub fn set_nonblocking(&self, nonblocking: bool) -> Result<()> {
        self.0.set_nonblocking(nonblocking)
    }

    pub fn read_to_end(&self, buf: &mut Vec<u8>) -> Result<usize> {
        self.0.read_to_end(buf)
    }
}

impl AsRawFd for AnonPipe {
    fn as_raw_fd(&self) -> RawFd {
        self.0.as_raw_fd()
    }
}

impl AsFd for AnonPipe {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}

impl IntoRawFd for AnonPipe {
    fn into_raw_fd(self) -> RawFd {
        self.0.into_raw_fd()
    }
}

impl FromRawFd for AnonPipe {
    unsafe fn from_raw_fd(raw_fd: RawFd) -> Self {
        Self(FromRawFd::from_raw_fd(raw_fd))
    }
}

impl From<AnonPipe> for FileDesc {
    fn from(p: AnonPipe) -> FileDesc {
        p.0
    }
}

pub fn read2(p1: AnonPipe, v1: &mut Vec<u8>, p2: AnonPipe, v2: &mut Vec<u8>) -> Result<()> {
    // Set both pipes into nonblocking mode as we're gonna be reading from both
    // in the `select` loop below, and we wouldn't want one to block the other!
    p1.set_nonblocking(true)?;
    p2.set_nonblocking(true)?;

    let mut fds: [libc::pollfd; 2] = unsafe { zeroed() };
    fds[0].fd = p1.as_raw_fd();
    fds[0].events = libc::POLLIN;
    fds[1].fd = p2.as_raw_fd();
    fds[1].events = libc::POLLIN;
    loop {
        // wait for either pipe to become readable using `poll`
        cvt_r(|| unsafe { libc::poll(fds.as_mut_ptr(), 2, -1) })?;

        if fds[0].revents != 0 && read(&p1.0, v1)? {
            p2.set_nonblocking(false)?;
            return p2.read_to_end(v2).map(drop);
        }
        if fds[1].revents != 0 && read(&p2.0, v2)? {
            p1.set_nonblocking(false)?;
            return p1.read_to_end(v1).map(drop);
        }
    }

    // Read as much as we can from each pipe, ignoring EWOULDBLOCK or
    // EAGAIN. If we hit EOF, then this will happen because the underlying
    // reader will return Ok(0), in which case we'll see `Ok` ourselves. In
    // this case we flip the other fd back into blocking mode and read
    // whatever's leftover on that file descriptor.
    fn read(fd: &FileDesc, dst: &mut Vec<u8>) -> Result<bool> {
        match fd.read_to_end(dst) {
            Ok(_) => Ok(true),
            Err(e) => {
                if e.raw_os_error() == Some(libc::EWOULDBLOCK)
                    || e.raw_os_error() == Some(libc::EAGAIN)
                {
                    Ok(false)
                } else {
                    Err(e)
                }
            }
        }
    }
}
