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
use super::super::util;
use super::TagFilesystem;
use super::OP_TAG;
use crate::common;
use fuse_sys::err::FuseErrno;
use fuse_sys::{FuseResult, Request};
use log::info;
use nix::errno::Errno::{ENOATTR, ENOENT};
use std::path::Path;

impl<N> TagFilesystem<N>
where
    N: common::notify::Notifier,
{
    pub fn setxattr_impl(
        &self,
        _req: &Request,
        path: &Path,
        name: &str,
        value: &[u8],
        position: u32,
        flags: i32,
    ) -> FuseResult<()> {
        info!(
            target: OP_TAG,
            "Calling setxattr on {} for name {}",
            path.display(),
            name
        );

        let conn_lock = self.conn_pool.get_conn();
        let conn = conn_lock.lock();
        let real_conn = (*conn).borrow_mut();

        match self.resolve_to_alias_file(&real_conn, path)? {
            Some(file_path) => Ok(util::setxattr(&file_path, name, value, position, flags)
                .map_err(FuseErrno::from)?),
            None => Err(ENOENT.into()),
        }
    }

    pub fn getxattr_impl(
        &self,
        _req: &Request,
        path: &Path,
        name: &str,
        position: u32,
    ) -> FuseResult<Vec<u8>> {
        info!(
            target: OP_TAG,
            "Calling getxattr on {:?} for name {}", path, name
        );

        #[cfg(target_os = "macos")]
        let noattr_err = Err(ENOATTR.into());
        #[cfg(target_os = "linux")]
        let noattr_err = Err(ENODATA.into());

        #[cfg(target_os = "macos")]
        {
            // if path.ends_with(common::constants::FOLDER_ICON) {
            //     if name == XATTR_RESOURCE_FORK {
            //         // FIXME
            //         let volicon = std::path::PathBuf::from("/Users/amoffat/Desktop/icon.icns"); //self.settings.volicon().unwrap();
            //         let rfork_data = crate::platform::mac::rf::generate_icns_fork(&volicon)?;
            //         return Ok(rfork_data);
            //     } else if name == XATTR_FINDER_INFO {
            //         let mut rfork_data = vec![b'\0'; 32];
            //         rfork_data[0..10].copy_from_slice(b"iconMACS@\x10");
            //         return Ok(rfork_data);
            //     }
            // }
            //
            // if name == XATTR_FINDER_INFO {
            //     let mut rfork_data = vec![b'\0'; 32];
            //     rfork_data[8] = b'\x04';
            //     return Ok(rfork_data);
            // }
        }

        let conn_lock = self.conn_pool.get_conn();
        let conn = conn_lock.lock();
        let real_conn = (*conn).borrow_mut();

        match self.resolve_to_alias_file(&real_conn, path)? {
            Some(file_path) => {
                Ok(util::getxattr(&file_path, name, position).map_err(FuseErrno::from)?)
            }
            None => noattr_err,
        }
    }

    pub fn listxattr_impl(
        &self,
        _req: &Request,
        path: &Path,
        options: i32,
    ) -> FuseResult<Vec<String>> {
        info!(target: OP_TAG, "Calling listxattr on {}", path.display());

        #[cfg(target_os = "macos")]
        {
            // if path.ends_with(common::constants::FOLDER_ICON) {
            //     return Ok(vec![
            //         XATTR_FINDER_INFO.to_string(),
            //         XATTR_RESOURCE_FORK.to_string(),
            //     ]);
            // }
        }

        let conn_lock = self.conn_pool.get_conn();
        let conn = conn_lock.lock();
        let real_conn = (*conn).borrow_mut();

        if let Some(file_path) = self.resolve_to_alias_file(&real_conn, path)? {
            return Ok(util::listxattr(&file_path, options).map_err(FuseErrno::from)?);
        }

        Ok(vec![])
    }

    pub fn removexattr_impl(
        &self,
        _req: &Request,
        path: &Path,
        name: &str,
        options: i32,
    ) -> FuseResult<()> {
        info!(
            target: OP_TAG,
            "Calling removexattr on {} for name {}",
            path.display(),
            name
        );

        let conn_lock = self.conn_pool.get_conn();
        let conn = conn_lock.lock();
        let real_conn = (*conn).borrow_mut();

        match self.resolve_to_alias_file(&real_conn, path)? {
            Some(file_path) => {
                Ok(util::removexattr(&file_path, name, options).map_err(FuseErrno::from)?)
            }
            None => Err(ENOENT.into()),
        }
    }
}
