//! I don't know what this is for but FileDesc uses it...idk, here it is. Copied from:
//! https://github.com/rust-lang/rust/blob/8c6ce6b91b172f77c795a74bfeaf74b865146b3f/library/std/src/sys/unix/mod.rs

use std::io::{Error, ErrorKind, Result};

pub trait IsMinusOne {
    fn is_minus_one(&self) -> bool;
}

macro_rules! impl_is_minus_one {
    ($($t:ident)*) => ($(impl IsMinusOne for $t {
        fn is_minus_one(&self) -> bool {
            *self == -1
        }
    })*)
}

impl_is_minus_one! { i8 i16 i32 i64 isize }

pub fn cvt<T: IsMinusOne>(t: T) -> Result<T> {
    if t.is_minus_one() {
        Err(Error::last_os_error())
    } else {
        Ok(t)
    }
}

pub fn cvt_r<T, F>(mut f: F) -> Result<T>
where
    T: IsMinusOne,
    F: FnMut() -> T,
{
    loop {
        match cvt(f()) {
            Err(ref e) if e.kind() == ErrorKind::Interrupted => {}
            other => return other,
        }
    }
}

pub fn cvt_nz(error: libc::c_int) -> Result<()> {
    if error == 0 {
        Ok(())
    } else {
        Err(Error::from_raw_os_error(error))
    }
}
