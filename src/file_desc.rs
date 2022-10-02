//! This essentially reimplements the code at:
//! https://github.com/rust-lang/rust/blob/3fdd578d72a24d4efc2fe2ad18eec3b6ba72271e/library/std/src/sys/unix/fd.rs
//! for external use to provide fds for sockets to perform Stdio to memfd_exec'ed processes.

use libc::{self, off64_t};
use std::{
    cmp,
    io::{self, BorrowedCursor, IoSlice, IoSliceMut, Read},
    os::unix::{
        io::{AsFd, AsRawFd, FromRawFd, IntoRawFd, OwnedFd},
        prelude::{BorrowedFd, RawFd},
    },
};

use crate::cvt::cvt;

#[derive(Debug)]
pub struct FileDesc(OwnedFd);

const READ_LIMIT: usize = libc::ssize_t::MAX as usize;

const fn max_iov() -> usize {
    libc::UIO_MAXIOV as usize
}

impl FileDesc {
    pub fn read(&self, buf: &mut [u8]) -> io::Result<usize> {
        let ret = cvt(unsafe {
            libc::read(
                self.as_raw_fd(),
                buf.as_mut_ptr() as *mut libc::c_void,
                cmp::min(buf.len(), READ_LIMIT),
            )
        })?;
        Ok(ret as usize)
    }

    pub fn read_vectored(&self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        let ret = cvt(unsafe {
            libc::readv(
                self.as_raw_fd(),
                bufs.as_ptr() as *const libc::iovec,
                cmp::min(bufs.len(), max_iov()) as libc::c_int,
            )
        })?;
        Ok(ret as usize)
    }

    #[inline]
    pub fn is_read_vectored(&self) -> bool {
        cfg!(not(any(target_os = "espidf", target_os = "horizon")))
    }

    pub fn read_to_end(&self, buf: &mut Vec<u8>) -> io::Result<usize> {
        let mut me = self;
        (&mut me).read_to_end(buf)
    }

    pub fn read_at(&self, buf: &mut [u8], offset: u64) -> io::Result<usize> {
        use libc::pread64;

        unsafe {
            cvt(pread64(
                self.as_raw_fd(),
                buf.as_mut_ptr() as *mut libc::c_void,
                cmp::min(buf.len(), READ_LIMIT),
                offset as off64_t,
            ))
            .map(|n| n as usize)
        }
    }

    pub fn read_buf(&self, mut cursor: BorrowedCursor<'_>) -> io::Result<()> {
        let ret = cvt(unsafe {
            libc::read(
                self.as_raw_fd(),
                cursor.as_mut().as_mut_ptr() as *mut libc::c_void,
                cmp::min(cursor.capacity(), READ_LIMIT),
            )
        })?;

        // Safety: `ret` bytes were written to the initialized portion of the buffer
        unsafe {
            cursor.advance(ret as usize);
        }
        Ok(())
    }

    pub fn write(&self, buf: &[u8]) -> io::Result<usize> {
        let ret = cvt(unsafe {
            libc::write(
                self.as_raw_fd(),
                buf.as_ptr() as *const libc::c_void,
                cmp::min(buf.len(), READ_LIMIT),
            )
        })?;
        Ok(ret as usize)
    }

    pub fn write_vectored(&self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        let ret = cvt(unsafe {
            libc::writev(
                self.as_raw_fd(),
                bufs.as_ptr() as *const libc::iovec,
                cmp::min(bufs.len(), max_iov()) as libc::c_int,
            )
        })?;
        Ok(ret as usize)
    }

    #[inline]
    pub fn is_write_vectored(&self) -> bool {
        cfg!(not(any(target_os = "espidf", target_os = "horizon")))
    }

    pub fn write_at(&self, buf: &[u8], offset: u64) -> io::Result<usize> {
        use libc::pwrite64;

        unsafe {
            cvt(pwrite64(
                self.as_raw_fd(),
                buf.as_ptr() as *const libc::c_void,
                cmp::min(buf.len(), READ_LIMIT),
                offset as off64_t,
            ))
            .map(|n| n as usize)
        }
    }

    pub fn get_cloexec(&self) -> io::Result<bool> {
        unsafe { Ok((cvt(libc::fcntl(self.as_raw_fd(), libc::F_GETFD))? & libc::FD_CLOEXEC) != 0) }
    }

    pub fn set_cloexec(&self) -> io::Result<()> {
        unsafe {
            let previous = cvt(libc::fcntl(self.as_raw_fd(), libc::F_GETFD))?;
            let new = previous | libc::FD_CLOEXEC;
            if new != previous {
                cvt(libc::fcntl(self.as_raw_fd(), libc::F_SETFD, new))?;
            }
            Ok(())
        }
    }
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        unsafe {
            let v = nonblocking as libc::c_int;
            cvt(libc::ioctl(self.as_raw_fd(), libc::FIONBIO, &v))?;
            Ok(())
        }
    }

    #[inline]
    pub fn duplicate(&self) -> io::Result<FileDesc> {
        Ok(Self(self.0.try_clone()?))
    }
}

impl<'a> Read for &'a FileDesc {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        (**self).read(buf)
    }
}

impl AsFd for FileDesc {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}

impl AsRawFd for FileDesc {
    fn as_raw_fd(&self) -> RawFd {
        self.0.as_raw_fd()
    }
}

impl IntoRawFd for FileDesc {
    fn into_raw_fd(self) -> RawFd {
        self.0.into_raw_fd()
    }
}

impl FromRawFd for FileDesc {
    unsafe fn from_raw_fd(raw_fd: RawFd) -> Self {
        Self(FromRawFd::from_raw_fd(raw_fd))
    }
}
