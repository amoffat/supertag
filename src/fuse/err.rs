/*
 * Supertag
 * Copyright (C) 2020 Andrew Moffat
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU Affero General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU Affero General Public License for more details.
 *
 * You should have received a copy of the GNU Affero General Public License
 * along with this program.  If not, see <http://www.gnu.org/licenses/>.
 */

use crate::common::err::STagError;
use core::fmt;
#[cfg(target_os = "macos")]
use core_foundation::error::CFError;
use fuse_sys::err::FuseErrno;
use nix::errno::Errno;
use nix::errno::Errno::{EIO, EPERM};
use rusqlite::Error as SqlError;
use std::borrow::Borrow;
use std::error::Error;
use std::fmt::Formatter;
use std::io::ErrorKind;

/// As the name suggests, this serves as a shim.  It was needed because there is no way in Rust currently to have two
/// external packages convert between data types with `From` without an explicit shim.  In our code, this manifests as
/// `fuse-sys` needing to convert a `rusqlite::Error` into a `fuse_sys::FuseErrno`.  We can't define a `From` or `Into`
/// on either of those datatypes in the `supertag` code, because `supertag` doesn't own those types.  The best we can do is
/// this shim, which knows how to do the required conversions.
///
/// https://stackoverflow.com/questions/59240348/whats-the-idiomatic-way-of-creating-a-rust-error-shim
#[derive(Debug)]
pub(crate) struct SupertagShimError {
    errno: Errno,
    original: Option<Box<dyn Error>>,
}

impl std::fmt::Display for SupertagShimError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({:?})", self.errno, self.original)
    }
}

impl Error for SupertagShimError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.original {
            Some(e) => Some(e.borrow()),
            None => None,
        }
    }
}

fn map_io_err(e: &std::io::Error) -> Errno {
    match e.kind() {
        ErrorKind::InvalidData => EIO,
        ErrorKind::PermissionDenied => EPERM,
        _ => Errno::from_i32(e.raw_os_error().unwrap_or(EIO as i32)),
    }
}

#[cfg(target_os = "macos")]
impl From<CFError> for SupertagShimError {
    fn from(e: CFError) -> Self {
        Self {
            errno: Errno::EIO,
            original: Some(Box::new(e)),
        }
    }
}

impl From<std::io::Error> for SupertagShimError {
    fn from(e: std::io::Error) -> Self {
        Self {
            errno: map_io_err(&e),
            original: Some(Box::new(e)),
        }
    }
}

impl From<SqlError> for SupertagShimError {
    fn from(e: SqlError) -> Self {
        Self {
            errno: Errno::EIO,
            original: Some(Box::new(e)),
        }
    }
}

impl From<SupertagShimError> for FuseErrno {
    fn from(e: SupertagShimError) -> Self {
        Self {
            errno: e.errno,
            original: Some(Box::new(e)),
        }
    }
}

impl From<Box<dyn Error>> for SupertagShimError {
    fn from(e: Box<dyn Error>) -> Self {
        Self {
            errno: Errno::EIO,
            original: Some(e),
        }
    }
}

impl From<STagError> for SupertagShimError {
    fn from(e: STagError) -> Self {
        let new_err = match &e {
            STagError::PathExists(_p) => Errno::EEXIST,
            _ => Errno::EIO,
        };
        Self {
            errno: new_err,
            original: Some(Box::new(e)),
        }
    }
}
