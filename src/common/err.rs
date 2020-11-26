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

#[cfg(target_os = "macos")]
use core_foundation::error::CFError;
use fuse_sys::err::FuseErrno;
use nix::errno::Errno;
use std::error::Error;
use std::io::ErrorKind;
use std::path::PathBuf;

pub type STagResult<T> = Result<T, STagError>;

pub enum STagError {
    BadTag(String),
    BadTagGroup(String),
    DatabaseError(rusqlite::Error),
    NotEnoughTags,
    InvalidPath(PathBuf),
    NonCollectionPath(PathBuf),
    BadDeviceFile(String),
    PathExists(PathBuf),
    RecursiveLink(PathBuf),
    IOError(Box<dyn Error>),
    Other(Box<dyn Error>),
    #[cfg(target_os = "macos")]
    MacosError(CFError),
}

impl From<std::io::Error> for STagError {
    fn from(e: std::io::Error) -> Self {
        match e.kind() {
            ErrorKind::NotFound => STagError::IOError(Box::new(e)),
            ErrorKind::Other => STagError::IOError(Box::new(e)),
            _kind => STagError::Other(Box::new(e)),
        }
    }
}

#[cfg(target_os = "macos")]
impl From<CFError> for STagError {
    fn from(e: CFError) -> Self {
        STagError::MacosError(e)
    }
}

impl From<nix::Error> for STagError {
    fn from(e: nix::Error) -> Self {
        STagError::Other(Box::new(e))
    }
}

impl From<rusqlite::Error> for STagError {
    fn from(e: rusqlite::Error) -> Self {
        STagError::DatabaseError(e)
    }
}

impl From<Box<dyn Error>> for STagError {
    fn from(e: Box<dyn Error>) -> Self {
        STagError::Other(e)
    }
}

impl Error for STagError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            STagError::DatabaseError(e) => Some(e),
            _ => None,
        }
    }
}

impl From<STagError> for FuseErrno {
    fn from(e: STagError) -> Self {
        Self {
            errno: Errno::EIO,
            original: Some(Box::new(e)),
        }
    }
}

impl std::fmt::Display for STagError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            STagError::BadTag(tag) => write!(f, "Invalid tag: {}", tag),
            STagError::BadTagGroup(group) => write!(f, "Invalid tag group: {}", group),
            STagError::BadDeviceFile(name) => write!(f, "Invalid device file: {}", name),
            STagError::DatabaseError(dbe) => write!(f, "Database error: {:?}", dbe),
            STagError::InvalidPath(path) => write!(f, "Invalid path {}", path.display()),
            STagError::PathExists(dst) => write!(f, "Path {:?} already exists", dst),
            STagError::IOError(e) => write!(f, "IO error: {:?}", e),
            STagError::Other(e) => write!(f, "Other unknown error: {:?}", e),
            STagError::NotEnoughTags => write!(f, "Not enough tags"),
            STagError::RecursiveLink(src) => write!(f, "Recursive symlink {:?}", src),
            #[cfg(target_os = "macos")]
            STagError::MacosError(cfe) => write!(f, "Macos error: {:?}", cfe),
            STagError::NonCollectionPath(src) => write!(
                f,
                "Path {:?} not found in collection. Try using an absolute path.",
                src
            ),
        }
    }
}

impl std::fmt::Debug for STagError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        std::fmt::Display::fmt(self, f)
    }
}

#[derive(Debug)]
pub struct ParseOctalError;

impl std::fmt::Display for ParseOctalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "Bad octal value")
    }
}
impl Error for ParseOctalError {}
