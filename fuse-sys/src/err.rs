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

use nix::errno::Errno;
use nix::errno::Errno::{EIO, EPERM};
use std::borrow::Borrow;
use std::ffi::NulError;
use std::io::{Error, ErrorKind};

#[derive(Debug)]
pub struct FuseErrno {
    pub errno: Errno,
    pub original: Option<Box<dyn std::error::Error>>,
}

impl From<FuseErrno> for libc::c_int {
    fn from(e: FuseErrno) -> Self {
        // fuse errno are negative, because that's what they said
        -(e.errno as libc::c_int)
    }
}

impl From<Errno> for FuseErrno {
    fn from(num: Errno) -> Self {
        Self {
            errno: num,
            original: None,
        }
    }
}

impl From<NulError> for FuseErrno {
    fn from(_e: NulError) -> Self {
        Self {
            errno: EIO,
            original: None,
        }
    }
}

fn map_io_err(e: &Error) -> Errno {
    match e.kind() {
        ErrorKind::InvalidData => EIO,
        ErrorKind::PermissionDenied => EPERM,
        _ => Errno::from_i32(e.raw_os_error().unwrap_or(EIO as i32)),
    }
}

impl From<std::io::Error> for FuseErrno {
    fn from(e: std::io::Error) -> Self {
        Self {
            errno: map_io_err(&e),
            original: Some(Box::new(e)),
        }
    }
}

impl std::error::Error for FuseErrno {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.original {
            Some(original) => Some(original.borrow()),
            None => None,
        }
    }
}

impl std::fmt::Display for FuseErrno {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.original {
            Some(original) => write!(f, "{} ({:?})", self.errno, *original),
            None => write!(f, "{}", self.errno),
        }
    }
}
