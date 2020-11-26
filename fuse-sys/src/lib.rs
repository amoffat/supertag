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

use libc::{c_char, c_int, c_void};
use nix::errno::Errno::{ENOENT, ENOSYS};
use parking_lot::Mutex;
use std::ffi::{CStr, CString, OsStr};
use std::mem::size_of;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::io::{FromRawFd, RawFd};
use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;

use log::{debug, error, info, trace, warn};

pub use bindings::*;

use crate::bindings::conf::FuseConfig;
use crate::bindings::fuse_get_context;
use crate::conf::MountConfig;
use crate::err::FuseErrno;
use std::fmt::{Debug, Error, Formatter};

mod bindings;
pub mod err;

type FuseOperations = fuse_operations;

pub type FuseResult<T> = Result<T, err::FuseErrno>;

const FUSEOP_TAG: &str = "fuse_op";
const FUSE_TAG: &str = "fuse";
const FS_TAG: &str = "fuse_fs";

#[derive(Ord, PartialOrd, Eq, PartialEq, Hash, Clone, Debug)]
pub struct Request {
    pub uid: uid_t,
    pub gid: gid_t,
    pub pid: pid_t,
    pub umask: mode_t,
}

#[cfg(target_os = "linux")]
pub fn new_statvfs() -> statvfs {
    statvfs {
        // Filesystem block size
        f_bsize: 0,
        // Fragment size
        f_frsize: 0,
        // Size of fs in f_frsize units
        f_blocks: 0,
        // Number of free blocks
        f_bfree: 0,
        // Number of free blocks for unprivileged users
        f_bavail: 0,
        // Number of inodes
        f_files: 0,
        // Number of free inodes
        f_ffree: 0,
        // Number of free inodes for unprivileged users
        f_favail: 0,
        // Filesystem ID
        f_fsid: 0,
        // Mount flags
        f_flag: 0,
        // Maximum filename length
        f_namemax: 0,
        __f_spare: [0; 6usize],
    }
}

#[cfg(target_os = "macos")]
pub fn new_statvfs() -> statvfs {
    statvfs {
        f_bsize: 0,
        f_frsize: 0,
        f_blocks: 0,
        f_bfree: 0,
        f_bavail: 0,
        f_files: 0,
        f_ffree: 0,
        f_favail: 0,
        f_fsid: 0,
        f_flag: 0,
        f_namemax: 0,
    }
}

/// `FuseHandle` represents the C handles we get back from fuse for controlling the connection.  The
/// handle fields are Arcs because we're sharing them with `MountHandle` which needs them in
/// `drop()` in order to tear down the connection.  The main use of `FuseHandle` is to pass it to
/// the Filesystem trait implementor, so that it can talk directly to fuse if it needs to, for
/// things like invalidating paths.
pub struct FuseHandle {
    disabled: AtomicBool,
    handle_struct: AtomicPtr<fuse>,
    channel_struct: AtomicPtr<fuse_chan>,
}

impl FuseHandle {
    fn disable(&self) {
        self.disabled.store(true, Ordering::SeqCst);
    }
    pub fn invalidate(&self, path: &Path) {
        if self.disabled.load(Ordering::SeqCst) {
            return;
        }

        let path_bytes = CString::new(path.as_os_str().as_bytes()).unwrap();
        let _path_raw = path_bytes.into_raw();

        // fuse_invalidate_path only lives on libfuse >= 3.0, which isn't in ubuntu 18.04 LTS. it's on mac though.
        // FIXME make this smarter wrt detecting fuse version number
        #[cfg(target_os = "macos")]
        unsafe {
            fuse_invalidate_path(self.handle_struct.load(Ordering::Relaxed), _path_raw);
        }
    }
}

#[cfg(target_os = "macos")]
unsafe fn fdatasync(fd: std::os::raw::c_int) -> std::os::raw::c_int {
    libc::fsync(fd)
}
#[cfg(not(target_os = "macos"))]
unsafe fn fdatasync(fd: std::os::raw::c_int) -> std::os::raw::c_int {
    libc::fdatasync(fd)
}

/// A Filesystem represents a filesystem with callbacks for fuse to call.  Notice not all of the
/// fuse functions are implemented.  They can be fleshed out as needed.
pub trait Filesystem {
    // notice that none of the methods are &mut self.  this is because we want libfuse to be able
    // to process requests in a thread-safe manner.  libfuse may be already processing requests
    // serially, but we don't want to rely on that behavior.  so we don't let our Filesystem
    // implementors mutate.  if you need mutation, use interior mutation and use locking.

    fn init_request_id(&self);

    fn getattr(&self, req: &Request, path: &Path) -> FuseResult<stat>;
    fn readdir(
        &self,
        req: &Request,
        path: &Path,
    ) -> FuseResult<Box<dyn Iterator<Item = FileEntry>>>;
    fn readdir_common(
        &self,
        _req: &Request,
        _path: &Path,
    ) -> FuseResult<Box<dyn Iterator<Item = FileEntry>>> {
        debug!(
            target: FS_TAG,
            "Calling default readdir_common implementation"
        );
        let mut common = vec![];
        common.push(FileEntry {
            name: ".".into(),
            mtime: chrono::Utc::now(),
        });
        common.push(FileEntry {
            name: "..".into(),
            mtime: chrono::Utc::now(),
        });
        Ok(Box::new(common.into_iter()))
    }

    fn readlink(&self, req: &Request, path: &Path) -> FuseResult<PathBuf>;
    fn symlink(&self, req: &Request, src: &Path, dst: &Path) -> FuseResult<()>;
    fn create(&self, req: &Request, path: &Path, mode: mode_t) -> FuseResult<RawFd>;

    // TODO add default impl for this
    fn open(&self, req: &Request, path: &Path, fi: *const fuse_file_info) -> FuseResult<RawFd>;

    fn read(
        &self,
        _req: &Request,
        _path: &Path,
        buf: &mut [u8],
        offset: off_t,
        fi: *const fuse_file_info,
    ) -> FuseResult<usize> {
        unsafe {
            info!(
                target: FS_TAG,
                "Calling default read implementation on {} for {} bytes",
                (*fi).fh,
                buf.len()
            );

            let read = libc::pread(
                (*fi).fh as i32,
                buf.as_mut_ptr() as *mut ::std::os::raw::c_void,
                buf.len(),
                offset,
            );

            if read == -1 {
                Err(std::io::Error::last_os_error().into())
            } else {
                Ok(read as usize)
            }
        }
    }

    fn write(
        &self,
        _req: &Request,
        _path: &Path,
        data: &[u8],
        offset: off_t,
        fi: *const fuse_file_info,
    ) -> FuseResult<usize> {
        unsafe {
            info!(
                target: FS_TAG,
                "Calling default write implementation on {}",
                (*fi).fh
            );

            let seeked_to = libc::lseek((*fi).fh as i32, offset, libc::SEEK_SET);
            if seeked_to == -1 {
                return Err(std::io::Error::last_os_error().into());
            }

            let written = libc::write(
                (*fi).fh as i32,
                data.as_ptr() as *const ::std::os::raw::c_void,
                data.len(),
            );

            if written == -1 {
                Err(std::io::Error::last_os_error().into())
            } else {
                Ok(written as usize)
            }
        }
    }

    fn flush(&self, _req: &Request, _path: &Path, fi: *const fuse_file_info) -> FuseResult<()> {
        unsafe {
            info!(
                target: FS_TAG,
                "Calling default (empty) flush implementation on {}",
                (*fi).fh
            );
        }
        Ok(())
    }

    fn truncate(&self, _req: &Request, path: &Path, _offset: off_t) -> FuseResult<()> {
        info!(
            target: FS_TAG,
            "Calling default truncate on {}",
            path.display()
        );
        Err(ENOSYS.into())
    }

    fn fsync(
        &self,
        _req: &Request,
        _path: &Path,
        datasync: i32,
        fi: *const fuse_file_info,
    ) -> FuseResult<()> {
        unsafe {
            info!(
                target: FS_TAG,
                "Calling default fsync implementation on {}",
                (*fi).fh
            );
            // TODO is this safe to remove? it consumes ownership
            //let mut fh = std::fs::File::from_raw_fd((*fi).fh as RawFd);

            let err = if datasync > 0 {
                fdatasync((*fi).fh as i32)
            } else {
                libc::fsync((*fi).fh as i32)
            };

            if err == -1 {
                Err(std::io::Error::last_os_error().into())
            } else {
                Ok(())
            }
        }
    }
    fn release(&self, _req: &Request, _path: &Path, fi: *const fuse_file_info) -> FuseResult<()> {
        unsafe {
            info!(
                target: FS_TAG,
                "Calling default release implementation on {}",
                (*fi).fh
            );

            // collect our fd into a File object, so that it is dropped and closed when it goes out
            // of scope
            let mut _fh = std::fs::File::from_raw_fd((*fi).fh as RawFd);
        }
        Ok(())
    }
    fn rmdir(&self, req: &Request, path: &Path) -> FuseResult<()>;
    fn unlink(&self, req: &Request, path: &Path) -> FuseResult<()>;
    fn mkdir(&self, req: &Request, path: &Path, mode: mode_t) -> FuseResult<()>;
    fn rename(&self, req: &Request, src: &Path, dst: &Path) -> FuseResult<()>;
    fn statfs(&self, req: &Request, path: &Path) -> FuseResult<statvfs>;

    fn set_handle(&mut self, _handle: Arc<FuseHandle>) {}

    fn chmod(&self, _req: &Request, path: &Path, mode: mode_t) -> FuseResult<()> {
        info!(
            target: FS_TAG,
            "Calling default chmod implementation on {}",
            path.display()
        );

        unsafe {
            let path_cs = CString::new(path.as_os_str().as_bytes()).unwrap();
            let err = libc::chmod(path_cs.as_ptr(), mode);
            if err == -1 {
                Err(std::io::Error::last_os_error().into())
            } else {
                Ok(())
            }
        }
    }

    fn chown(&self, _req: &Request, path: &Path, uid: uid_t, gid: gid_t) -> FuseResult<()> {
        info!(
            target: FS_TAG,
            "Calling default chown implementation on {}",
            path.display()
        );

        unsafe {
            let path_cs = CString::new(path.as_os_str().as_bytes()).unwrap();
            let err = libc::chown(path_cs.as_ptr(), uid, gid);
            if err == -1 {
                Err(std::io::Error::last_os_error().into())
            } else {
                Ok(())
            }
        }
    }

    // this convenience function combines calls to chmod, chown, utimens, truncate, ftruncate, chflags, setbkuptime
    // and setcrtime
    #[cfg(target_os = "macos")]
    fn setattr_x(&self, _req: &Request, _path: &Path, _attrs: *const setattr_x) -> FuseResult<()> {
        Err(ENOSYS.into())
    }

    // this allows setting of extended attributes
    fn setxattr(
        &self,
        _req: &Request,
        _path: &Path,
        _name: &str,
        _value: &[u8],
        _position: u32,
        _flags: i32,
    ) -> FuseResult<()> {
        Err(ENOSYS.into())
    }

    fn getxattr(
        &self,
        _req: &Request,
        _path: &Path,
        _name: &str,
        _position: u32,
    ) -> FuseResult<Vec<u8>> {
        Err(ENOSYS.into())
    }

    fn listxattr(&self, _req: &Request, _path: &Path, _options: i32) -> FuseResult<Vec<String>> {
        Err(ENOSYS.into())
    }

    fn removexattr(
        &self,
        _req: &Request,
        _path: &Path,
        _name: &str,
        _options: i32,
    ) -> FuseResult<()> {
        Err(ENOSYS.into())
    }
}

#[derive(Debug)]
pub struct FileEntry {
    pub name: String,
    pub mtime: chrono::DateTime<chrono::Utc>,
}

fn to_pathname(ptr: *const c_char) -> PathBuf {
    let slice = unsafe { CStr::from_ptr(ptr) };
    let osstr = OsStr::from_bytes(slice.to_bytes());
    let path: &Path = osstr.as_ref();
    path.to_owned()
}

/// Get the Filesystem trait object that we passed into mount
fn ops_from_ctx() -> (Request, &'static dyn Filesystem) {
    unsafe {
        let ctx = fuse_get_context();

        // sometimes umasks weirdly come in as 0 for processes where it shouldn't be 0
        // FIXME figure out why this is
        let umask = match (*ctx).umask {
            0 => 0o022,
            _ => (*ctx).umask,
        };

        let req = Request {
            uid: (*ctx).uid,
            gid: (*ctx).gid,
            pid: (*ctx).pid,
            umask,
        };
        trace!(target: FUSEOP_TAG, "{:?}", req);

        // (*ctx).private_data is a Box(&dyn Filesystem)
        // See the comment in the mount() function for more information on exactly what is happening
        let boxed = (*ctx).private_data as *const &dyn Filesystem;
        let fs_trait_ref = *boxed;
        fs_trait_ref.init_request_id();
        (req, fs_trait_ref)
    }
}

extern "C" fn readdir(
    arg1: *const ::std::os::raw::c_char,
    arg2: *mut ::std::os::raw::c_void,
    arg3: fuse_fill_dir_t,
    offset: off_t,
    _arg5: *mut fuse_file_info,
) -> ::std::os::raw::c_int {
    let name = to_pathname(arg1);

    let filler = arg3.unwrap();
    let (req, ops) = ops_from_ctx();

    info!(target: FUSEOP_TAG, "readdir {:?}", name);

    if offset == 0 {
        match ops.readdir_common(&req, &name) {
            Ok(entry_iter) => {
                for entry in entry_iter {
                    let entry_name = CString::new(entry.name).unwrap();
                    let done = unsafe { filler(arg2, entry_name.as_ptr(), ptr::null(), 0) };

                    // this should never happen while we're filling our common directories, since
                    // there should only be a few, and the fill buffer is supposedly large, but
                    // let's handle it anyways
                    if done > 0 {
                        return 0;
                    }
                }
            }
            Err(num) => {
                error!(target: FUSEOP_TAG, "Error getting readdir_common {}", num);
                return num.into();
            }
        }
    }

    match ops.readdir(&req, &name) {
        Ok(entry_iter) => {
            for entry in entry_iter {
                let entry_name = CString::new(entry.name).unwrap();
                let done = unsafe { filler(arg2, entry_name.as_ptr(), ptr::null(), 0) };
                if done > 0 {
                    break;
                }
            }
            0
        }
        Err(num) => {
            error!(
                target: FUSEOP_TAG,
                "readdir error {} for {}",
                num,
                name.display()
            );
            num.into()
        }
    }
}

extern "C" fn opendir(
    arg1: *const ::std::os::raw::c_char,
    _arg2: *mut fuse_file_info,
) -> ::std::os::raw::c_int {
    let name = to_pathname(arg1);
    info!(target: FUSEOP_TAG, "opendir {:?}", name);
    0
}

extern "C" fn releasedir(
    arg1: *const ::std::os::raw::c_char,
    _arg2: *mut fuse_file_info,
) -> ::std::os::raw::c_int {
    let name = to_pathname(arg1);
    info!(target: FUSEOP_TAG, "releasedir {:?}", name);
    0
}

extern "C" fn readlink(
    arg1: *const ::std::os::raw::c_char,
    arg2: *mut ::std::os::raw::c_char,
    _arg3: usize,
) -> ::std::os::raw::c_int {
    let name = to_pathname(arg1);
    let (req, ops) = ops_from_ctx();
    info!(target: FUSEOP_TAG, "readlink {:?}", name);

    match ops.readlink(&req, &name) {
        Ok(link_path) => {
            // FIXME can fail if path as an interior null byte
            let link_str = CString::new(link_path.as_os_str().as_bytes()).unwrap();
            unsafe {
                ptr::copy(link_str.as_ptr(), arg2, link_str.as_bytes_with_nul().len());
            };
            0
        }
        Err(num) => {
            error!(target: FUSEOP_TAG, "readlink error {}", num);
            num.into()
        }
    }
}

extern "C" fn flush(
    arg1: *const ::std::os::raw::c_char,
    arg2: *mut fuse_file_info,
) -> ::std::os::raw::c_int {
    let name = to_pathname(arg1);
    let (req, ops) = ops_from_ctx();
    info!(target: FUSEOP_TAG, "flush {:?}", name);

    match ops.flush(&req, &name, arg2) {
        Ok(_) => 0,
        Err(num) => {
            error!(target: FUSEOP_TAG, "flush error {}", num,);
            num.into()
        }
    }
}

extern "C" fn getattr(
    arg1: *const ::std::os::raw::c_char,
    arg2: *mut stat,
) -> ::std::os::raw::c_int {
    let name = to_pathname(arg1);
    let (req, ops) = ops_from_ctx();
    info!(target: FUSEOP_TAG, "getattr {:?}", name);

    let maybe_file_stat = ops.getattr(&req, &name);
    match maybe_file_stat {
        Ok(file_stat) => {
            debug!(target: FUSEOP_TAG, "stat for {:?} is {:?}", name, file_stat);
            unsafe {
                let attr = &mut *arg2;
                *attr = file_stat;
            }
            0
        }
        Err(num) => {
            if num.errno == ENOENT {
                warn!(target: FUSEOP_TAG, "getattr ENOENT for {:?}", name);
            } else {
                error!(target: FUSEOP_TAG, "getattr error {:?} for {:?}", num, name);
            }
            num.into()
        }
    }
}

extern "C" fn symlink(
    arg1: *const ::std::os::raw::c_char,
    arg2: *const ::std::os::raw::c_char,
) -> ::std::os::raw::c_int {
    let src = to_pathname(arg1);
    let dst = to_pathname(arg2);
    let (req, ops) = ops_from_ctx();
    info!(target: FUSEOP_TAG, "symlink {:?} to {:?}", src, dst);

    match ops.symlink(&req, &src, &dst) {
        Ok(_) => 0,
        Err(num) => {
            error!(
                target: FUSEOP_TAG,
                "symlink error {} for {} => {}",
                num,
                src.display(),
                dst.display()
            );
            num.into()
        }
    }
}

extern "C" fn rmdir(arg1: *const ::std::os::raw::c_char) -> ::std::os::raw::c_int {
    let (req, ops) = ops_from_ctx();
    let name = to_pathname(arg1);
    info!(target: FUSEOP_TAG, "rmdir {:?}", name);

    match ops.rmdir(&req, &name) {
        Ok(_) => 0,
        Err(num) => {
            error!(
                target: FUSEOP_TAG,
                "rmdir error {} for {}",
                num,
                name.display()
            );
            num.into()
        }
    }
}

extern "C" fn unlink(arg1: *const ::std::os::raw::c_char) -> ::std::os::raw::c_int {
    let (req, ops) = ops_from_ctx();
    let name = to_pathname(arg1);
    info!(target: FUSEOP_TAG, "unlink {:?}", name);

    match ops.unlink(&req, &name) {
        Ok(_) => 0,
        Err(num) => {
            error!(
                target: FUSEOP_TAG,
                "unlink error {} for {}",
                num,
                name.display()
            );
            num.into()
        }
    }
}

extern "C" fn mkdir(arg1: *const ::std::os::raw::c_char, arg2: mode_t) -> ::std::os::raw::c_int {
    let (req, ops) = ops_from_ctx();
    let name = to_pathname(arg1);
    info!(target: FUSEOP_TAG, "mkdir {:?}", name);

    match ops.mkdir(&req, &name, arg2) {
        Ok(_) => 0,
        Err(num) => {
            error!(
                target: FUSEOP_TAG,
                "mkdir error {} for {}",
                num,
                name.display()
            );
            num.into()
        }
    }
}

extern "C" fn rename(
    arg1: *const ::std::os::raw::c_char,
    arg2: *const ::std::os::raw::c_char,
) -> ::std::os::raw::c_int {
    let (req, ops) = ops_from_ctx();
    let src = to_pathname(arg1);
    let dst = to_pathname(arg2);
    info!(target: FUSEOP_TAG, "rename {:?} to {:?}", src, dst);

    match ops.rename(&req, &src, &dst) {
        Ok(_) => 0,
        Err(num) => {
            error!(
                target: FUSEOP_TAG,
                "rename error {} for {}",
                num,
                src.display()
            );
            num.into()
        }
    }
}

extern "C" fn write(
    arg1: *const ::std::os::raw::c_char,
    arg2: *const ::std::os::raw::c_char,
    arg3: usize,
    arg4: off_t,
    arg5: *mut fuse_file_info,
) -> ::std::os::raw::c_int {
    let (req, ops) = ops_from_ctx();
    let name = to_pathname(arg1);
    info!(
        target: FUSEOP_TAG,
        "write {} bytes to {:?} at offset {}", arg3, name, arg4
    );

    let data = unsafe {
        let tmp_slice = std::slice::from_raw_parts(arg2, arg3);
        &*(tmp_slice as *const _ as *const [u8])
    };
    match ops.write(&req, &name, data, arg4, arg5) {
        Ok(written) => {
            debug!(target: FUSEOP_TAG, "wrote {} bytes", written);
            written as i32
        }
        Err(num) => {
            error!(target: FUSEOP_TAG, "write error {}", num,);
            num.into()
        }
    }
}

extern "C" fn fsync(
    arg1: *const ::std::os::raw::c_char,
    arg2: ::std::os::raw::c_int,
    arg3: *mut fuse_file_info,
) -> ::std::os::raw::c_int {
    let (req, ops) = ops_from_ctx();
    let name = to_pathname(arg1);
    info!(target: FUSEOP_TAG, "fsync {:?}", name);

    match ops.fsync(&req, &name, arg2, arg3) {
        Ok(_) => 0,
        Err(num) => {
            error!(
                target: FUSEOP_TAG,
                "fsync error {} for {}",
                num,
                name.display()
            );
            num.into()
        }
    }
}

extern "C" fn truncate(arg1: *const ::std::os::raw::c_char, arg2: off_t) -> ::std::os::raw::c_int {
    let (req, ops) = ops_from_ctx();
    let name = to_pathname(arg1);
    info!(target: FUSEOP_TAG, "truncate {:?}", name);

    match ops.truncate(&req, &name, arg2) {
        Ok(_) => 0,
        Err(num) => {
            error!(
                target: FUSEOP_TAG,
                "truncate error {} for {}",
                num,
                name.display()
            );
            num.into()
        }
    }
}

extern "C" fn release(
    arg1: *const ::std::os::raw::c_char,
    arg2: *mut fuse_file_info,
) -> ::std::os::raw::c_int {
    let (req, ops) = ops_from_ctx();
    let name = to_pathname(arg1);
    info!(target: FUSEOP_TAG, "release {:?}", name);

    match ops.release(&req, &name, arg2) {
        Ok(_) => 0,
        Err(num) => {
            error!(
                target: FUSEOP_TAG,
                "release error {} for {}",
                num,
                name.display()
            );
            num.into()
        }
    }
}

extern "C" fn open(
    arg1: *const ::std::os::raw::c_char,
    arg2: *mut fuse_file_info,
) -> ::std::os::raw::c_int {
    let (req, ops) = ops_from_ctx();
    let name = to_pathname(arg1);
    info!(target: FUSEOP_TAG, "open {:?}", name);

    match ops.open(&req, &name, arg2) {
        Ok(fd) => {
            unsafe {
                (*arg2).fh = fd as u64;
                debug!(target: FUSEOP_TAG, "open made fd {}", fd);
            }
            0
        }
        Err(num) => {
            error!(
                target: FUSEOP_TAG,
                "open error {} for {}",
                num,
                name.display()
            );
            num.into()
        }
    }
}

extern "C" fn create(
    arg1: *const ::std::os::raw::c_char,
    mode: mode_t,
    arg3: *mut fuse_file_info,
) -> ::std::os::raw::c_int {
    let (req, ops) = ops_from_ctx();
    let name = to_pathname(arg1);
    info!(target: FUSEOP_TAG, "create {:?} with mode {}", name, mode);

    match ops.create(&req, &name, mode) {
        Ok(fd) => {
            unsafe {
                (*arg3).fh = fd as u64;
                debug!(target: FUSEOP_TAG, "create made fd {}", (*arg3).fh);
            }
            0
        }
        Err(num) => {
            error!(
                target: FUSEOP_TAG,
                "create error {} for {}",
                num,
                name.display()
            );
            num.into()
        }
    }
}

extern "C" fn read(
    arg1: *const ::std::os::raw::c_char,
    arg2: *mut ::std::os::raw::c_char,
    arg3: usize,
    arg4: off_t,
    arg5: *mut fuse_file_info,
) -> ::std::os::raw::c_int {
    let (req, ops) = ops_from_ctx();
    let name = to_pathname(arg1);
    info!(
        target: FUSEOP_TAG,
        "read desired {} bytes at offset {} for {:?} ", arg3, arg4, name
    );

    let buf = unsafe {
        let tmp_slice = std::slice::from_raw_parts(arg2, arg3);
        &mut *(tmp_slice as *const _ as *mut [u8])
    };

    match ops.read(&req, &name, buf, arg4, arg5) {
        Ok(read) => {
            debug!(target: FUSEOP_TAG, "read {} bytes", read);
            read as i32
        }
        Err(num) => {
            error!(
                target: FUSEOP_TAG,
                "read error {} for {}",
                num,
                name.display()
            );
            num.into()
        }
    }
}

extern "C" fn statfs(
    arg1: *const ::std::os::raw::c_char,
    arg2: *mut statvfs,
) -> ::std::os::raw::c_int {
    let (req, ops) = ops_from_ctx();
    let name = to_pathname(arg1);
    info!(target: FUSEOP_TAG, "statfs {:?}", name);

    match ops.statfs(&req, &name) {
        Ok(data) => unsafe {
            *arg2 = data;
            0
        },
        Err(num) => {
            error!(
                target: FUSEOP_TAG,
                "statfs error {} for {}",
                num,
                name.display()
            );
            num.into()
        }
    }
}

extern "C" fn chmod(arg1: *const ::std::os::raw::c_char, mode: mode_t) -> ::std::os::raw::c_int {
    let (req, ops) = ops_from_ctx();
    let name = to_pathname(arg1);
    info!(target: FUSEOP_TAG, "chmod {:?} with mode {}", name, mode);

    match ops.chmod(&req, &name, mode) {
        Ok(_) => 0,
        Err(num) => {
            error!(
                target: FUSEOP_TAG,
                "chmod error {} for {}",
                num,
                name.display()
            );
            num.into()
        }
    }
}

extern "C" fn chown(
    arg1: *const ::std::os::raw::c_char,
    uid: uid_t,
    gid: gid_t,
) -> ::std::os::raw::c_int {
    let (req, ops) = ops_from_ctx();
    let name = to_pathname(arg1);
    info!(
        target: FUSEOP_TAG,
        "chown {:?} with uid:gid {}:{}", name, uid, gid
    );

    match ops.chown(&req, &name, uid, gid) {
        Ok(_) => 0,
        Err(num) => {
            error!(
                target: FUSEOP_TAG,
                "chown error {} for {}",
                num,
                name.display()
            );
            num.into()
        }
    }
}

extern "C" fn access(
    _arg1: *const ::std::os::raw::c_char,
    _arg2: ::std::os::raw::c_int,
) -> ::std::os::raw::c_int {
    info!(target: FUSEOP_TAG, "access");
    FuseErrno::from(ENOSYS).into()
}

#[allow(dead_code)]
extern "C" fn chflags(
    _arg1: *const ::std::os::raw::c_char,
    _arg2: ::std::os::raw::c_uint,
) -> ::std::os::raw::c_int {
    info!(target: FUSEOP_TAG, "chflags");
    FuseErrno::from(ENOSYS).into()
}

extern "C" fn fsyncdir(
    arg1: *const ::std::os::raw::c_char,
    _arg2: ::std::os::raw::c_int,
    _arg3: *mut fuse_file_info,
) -> ::std::os::raw::c_int {
    let name = to_pathname(arg1);
    info!(target: FUSEOP_TAG, "fsyncdir on {:?}", name);
    FuseErrno::from(ENOSYS).into()
}

extern "C" fn ftruncate(
    arg1: *const ::std::os::raw::c_char,
    arg2: off_t,
    _arg3: *mut fuse_file_info,
) -> ::std::os::raw::c_int {
    info!(target: FUSEOP_TAG, "ftruncate, delegating to truncate");
    truncate(arg1, arg2)
}

extern "C" fn ioctl(
    _arg1: *const ::std::os::raw::c_char,
    _cmd: ::std::os::raw::c_int,
    _arg: *mut ::std::os::raw::c_void,
    _arg2: *mut fuse_file_info,
    _flags: ::std::os::raw::c_uint,
    _data: *mut ::std::os::raw::c_void,
) -> ::std::os::raw::c_int {
    info!(target: FUSEOP_TAG, "ioctl");
    FuseErrno::from(ENOSYS).into()
}

extern "C" fn poll(
    _arg1: *const ::std::os::raw::c_char,
    _arg2: *mut fuse_file_info,
    _ph: *mut fuse_pollhandle,
    _reventsp: *mut ::std::os::raw::c_uint,
) -> ::std::os::raw::c_int {
    info!(target: FUSEOP_TAG, "poll");
    FuseErrno::from(ENOSYS).into()
}

extern "C" fn mknod(
    arg1: *const ::std::os::raw::c_char,
    arg2: mode_t,
    arg3: dev_t,
) -> ::std::os::raw::c_int {
    let name = to_pathname(arg1);

    info!(
        target: FUSEOP_TAG,
        "mknod for {}, mode {}, device {}",
        name.display(),
        arg2,
        arg3
    );
    FuseErrno::from(ENOSYS).into()
}

#[cfg(target_os = "macos")]
extern "C" fn setxattr(
    arg1: *const ::std::os::raw::c_char,
    arg2: *const ::std::os::raw::c_char,
    arg3: *const ::std::os::raw::c_char,
    arg4: usize,
    arg5: ::std::os::raw::c_int,
    arg6: ::std::os::raw::c_uint,
) -> ::std::os::raw::c_int {
    unsafe { common_setxattr(arg1, arg2, arg3, arg4, arg5, arg6) }
}

#[cfg(target_os = "linux")]
extern "C" fn setxattr(
    arg1: *const ::std::os::raw::c_char,
    arg2: *const ::std::os::raw::c_char,
    arg3: *const ::std::os::raw::c_char,
    arg4: usize,
    arg5: ::std::os::raw::c_int,
) -> ::std::os::raw::c_int {
    unsafe { common_setxattr(arg1, arg2, arg3, arg4, arg5, 0) }
}

unsafe fn common_setxattr(
    arg1: *const ::std::os::raw::c_char,
    arg2: *const ::std::os::raw::c_char,
    arg3: *const ::std::os::raw::c_char,
    val_size: usize,
    flags: ::std::os::raw::c_int,
    position: ::std::os::raw::c_uint,
) -> ::std::os::raw::c_int {
    let (req, ops) = ops_from_ctx();
    let path = to_pathname(arg1);

    let name = CStr::from_ptr(arg2).to_string_lossy().into_owned();
    let value = std::slice::from_raw_parts(arg3 as *const ::std::os::raw::c_uchar, val_size);

    info!(
        target: FUSEOP_TAG,
        "setxattr for {}, name {}, value {:?}, position {}, flags {}",
        path.display(),
        name,
        value,
        position,
        flags,
    );

    match ops.setxattr(&req, &path, &name, value, position, flags) {
        Ok(_) => 0,
        Err(num) => {
            error!(
                target: FUSEOP_TAG,
                "setxattr error {} for {}",
                num,
                path.display()
            );
            num.into()
        }
    }
}

extern "C" fn utime(
    _arg1: *const ::std::os::raw::c_char,
    _arg2: *mut utimbuf,
) -> ::std::os::raw::c_int {
    info!(target: FUSEOP_TAG, "utime");
    FuseErrno::from(ENOSYS).into()
}

extern "C" fn utimens(
    _arg1: *const ::std::os::raw::c_char,
    _tv: *const timespec,
) -> ::std::os::raw::c_int {
    info!(target: FUSEOP_TAG, "utimens");
    FuseErrno::from(ENOSYS).into()
}

#[allow(dead_code)]
extern "C" fn setchgtime(
    _arg1: *const ::std::os::raw::c_char,
    _tv: *const timespec,
) -> ::std::os::raw::c_int {
    info!(target: FUSEOP_TAG, "setchgtime");
    FuseErrno::from(ENOSYS).into()
}

#[allow(dead_code)]
extern "C" fn setcrtime(
    _arg1: *const ::std::os::raw::c_char,
    _tv: *const timespec,
) -> ::std::os::raw::c_int {
    info!(target: FUSEOP_TAG, "setcrtime");
    FuseErrno::from(ENOSYS).into()
}

#[cfg(target_os = "macos")]
extern "C" fn setattr_x(
    arg1: *const ::std::os::raw::c_char,
    arg2: *mut setattr_x,
) -> ::std::os::raw::c_int {
    let (req, ops) = ops_from_ctx();
    let name = to_pathname(arg1);
    info!(target: FUSEOP_TAG, "setattr_x {}", name.display());

    match ops.setattr_x(&req, &name, arg2 as *const setattr_x) {
        Ok(_) => 0,
        Err(num) => {
            error!(
                target: FUSEOP_TAG,
                "setattr_x error {} for {}",
                num,
                name.display()
            );
            num.into()
        }
    }
}

#[cfg(target_os = "macos")]
extern "C" fn fsetattr_x(
    _arg1: *const ::std::os::raw::c_char,
    _arg2: *mut setattr_x,
    _arg3: *mut fuse_file_info,
) -> ::std::os::raw::c_int {
    info!(target: FUSEOP_TAG, "fsetattr_x");
    FuseErrno::from(ENOSYS).into()
}

#[cfg(target_os = "linux")]
extern "C" fn listxattr(
    arg1: *const ::std::os::raw::c_char,
    arg2: *mut ::std::os::raw::c_char,
    arg3: usize,
) -> ::std::os::raw::c_int {
    listxattr_common(arg1, arg2, arg3, 0)
}

#[cfg(target_os = "macos")]
extern "C" fn listxattr(
    arg1: *const ::std::os::raw::c_char,
    arg2: *mut ::std::os::raw::c_char,
    arg3: usize,
) -> ::std::os::raw::c_int {
    // the man page says listxattr on bsd takes a 4th argument, "options", but osxfuse does not provide it
    listxattr_common(arg1, arg2, arg3, 0)
}

fn listxattr_common(
    arg1: *const ::std::os::raw::c_char,
    buf: *mut ::std::os::raw::c_char,
    bufsize: usize,
    options: ::std::os::raw::c_int,
) -> ::std::os::raw::c_int {
    let (req, ops) = ops_from_ctx();
    let path = to_pathname(arg1);

    info!(
        target: FUSEOP_TAG,
        "listxattr {}, bufsize {}, options {}",
        path.display(),
        bufsize,
        options
    );

    let size_only = buf.is_null() || bufsize == 0;

    if size_only {
        debug!(
            target: FUSEOP_TAG,
            "Caller is interested in the size of the xattrs"
        );
    }

    match ops.listxattr(&req, &path, options) {
        Ok(names) => {
            let mut size = 0;
            unsafe {
                let mut offset = 0;
                for name in names {
                    let c_name = CString::new(name).unwrap().into_bytes_with_nul();
                    size += c_name.len();

                    if !size_only {
                        ptr::copy_nonoverlapping(
                            c_name.as_ptr() as *const i8,
                            buf.offset(offset),
                            c_name.len(),
                        );
                        trace!(
                            target: FUSEOP_TAG,
                            "Copying {:?} to offset {} with len {}",
                            c_name,
                            offset,
                            c_name.len()
                        );
                        offset += c_name.len() as isize;
                    }
                }
            }

            size as i32
        }
        Err(num) => {
            error!(
                target: FUSEOP_TAG,
                "listxattr error {} for {}",
                num,
                path.display()
            );
            num.into()
        }
    }
}

#[cfg(target_os = "linux")]
extern "C" fn removexattr(
    arg1: *const ::std::os::raw::c_char,
    arg2: *const ::std::os::raw::c_char,
) -> ::std::os::raw::c_int {
    removexattr_common(arg1, arg2, 0)
}

#[cfg(target_os = "macos")]
extern "C" fn removexattr(
    arg1: *const ::std::os::raw::c_char,
    arg2: *const ::std::os::raw::c_char,
    //arg3: ::std::os::raw::c_int,
) -> ::std::os::raw::c_int {
    // the man page says removexattr on bsd takes a 3rd argument, "options", but osxfuse does not provide it
    removexattr_common(arg1, arg2, 0)
}

fn removexattr_common(
    arg1: *const ::std::os::raw::c_char,
    arg2: *const ::std::os::raw::c_char,
    arg3: ::std::os::raw::c_int,
) -> ::std::os::raw::c_int {
    let (req, ops) = ops_from_ctx();
    let path = to_pathname(arg1);
    let name = unsafe { CStr::from_ptr(arg2).to_string_lossy().into_owned() };

    info!(
        target: FUSEOP_TAG,
        "removexattr {} name {}, options {}",
        path.display(),
        name,
        arg3
    );
    match ops.removexattr(&req, &path, &name, arg3) {
        Ok(_) => 0,
        Err(num) => {
            error!(
                target: FUSEOP_TAG,
                "removexattr error {} for {}",
                num,
                path.display()
            );
            num.into()
        }
    }
}

#[cfg(target_os = "linux")]
extern "C" fn getxattr(
    arg1: *const ::std::os::raw::c_char,
    arg2: *const ::std::os::raw::c_char,
    arg3: *mut ::std::os::raw::c_char,
    bufsize: usize,
) -> ::std::os::raw::c_int {
    getxattr_common(arg1, arg2, arg3, bufsize, 0)
}

#[cfg(target_os = "macos")]
extern "C" fn getxattr(
    arg1: *const ::std::os::raw::c_char,
    arg2: *const ::std::os::raw::c_char,
    arg3: *mut ::std::os::raw::c_char,
    bufsize: usize,
    position: ::std::os::raw::c_uint,
) -> ::std::os::raw::c_int {
    getxattr_common(arg1, arg2, arg3, bufsize, position)
}

fn getxattr_common(
    arg1: *const ::std::os::raw::c_char,
    arg2: *const ::std::os::raw::c_char,
    arg3: *mut ::std::os::raw::c_char,
    bufsize: usize,
    position: ::std::os::raw::c_uint,
) -> ::std::os::raw::c_int {
    let (req, ops) = ops_from_ctx();
    let path = to_pathname(arg1);

    let name = unsafe { CStr::from_ptr(arg2) }
        .to_string_lossy()
        .into_owned();

    info!(target: FUSEOP_TAG, "getxattr for {:?}, name {}", path, name,);

    match ops.getxattr(&req, &path, &name, position) {
        Ok(value) => unsafe {
            // according to the man pages, if size is 0, the caller is requesting the size of the value, in order to
            // determine what size buffer to call us again with
            if bufsize == 0 {
                value.len() as i32
            } else {
                let copied = std::cmp::min(value.len(), bufsize);
                ptr::copy(value.as_ptr(), arg3 as *mut u8, copied);
                copied as i32
            }
        },
        Err(num) => {
            error!(target: FUSEOP_TAG, "getxattr error {} for {:?}", num, path);
            num.into()
        }
    }
}

extern "C" fn fgetattr(
    arg1: *const ::std::os::raw::c_char,
    arg2: *mut stat,
    _arg3: *mut fuse_file_info,
) -> ::std::os::raw::c_int {
    let name = to_pathname(arg1);
    info!(
        target: FUSEOP_TAG,
        "fgetattr for {}, delegating to getattr",
        name.display()
    );

    getattr(arg1, arg2)
}

impl FuseOperations {
    pub fn new() -> Self {
        Default::default()
    }
}

impl Default for FuseOperations {
    fn default() -> Self {
        #[cfg(target_os = "linux")]
        return Self {
            access: Some(access),
            bmap: None,
            chmod: Some(chmod),
            chown: Some(chown),
            create: Some(create),
            destroy: None,
            fallocate: None,
            fgetattr: Some(fgetattr),
            flock: None,
            flush: Some(flush),
            fsync: Some(fsync),
            fsyncdir: Some(fsyncdir),
            ftruncate: Some(ftruncate),
            getattr: Some(getattr),
            getdir: None,
            getxattr: Some(getxattr),
            init: None,
            ioctl: Some(ioctl),
            link: None,
            listxattr: Some(listxattr),
            lock: None,
            mkdir: Some(mkdir),
            mknod: Some(mknod),
            open: Some(open),
            opendir: Some(opendir),
            poll: Some(poll),
            read: Some(read),
            read_buf: None,
            readdir: Some(readdir),
            readlink: Some(readlink),
            release: Some(release),
            releasedir: Some(releasedir),
            removexattr: Some(removexattr),
            rename: Some(rename),
            rmdir: Some(rmdir),
            setxattr: Some(setxattr),
            statfs: Some(statfs),
            symlink: Some(symlink),
            truncate: Some(truncate),
            unlink: Some(unlink),
            utime: Some(utime),
            utimens: Some(utimens),
            write: Some(write),
            write_buf: None,

            _bitfield_1: Default::default(),
        };

        #[cfg(target_os = "macos")]
        return Self {
            access: Some(access),
            bmap: None,
            chflags: Some(chflags),
            chmod: Some(chmod),
            chown: Some(chown),
            create: Some(create),
            destroy: None,
            exchange: None,
            fallocate: None,
            fgetattr: Some(fgetattr),
            flock: None,
            flush: Some(flush),
            fsetattr_x: Some(fsetattr_x),
            fsync: Some(fsync),
            fsyncdir: Some(fsyncdir),
            ftruncate: Some(ftruncate),
            getattr: Some(getattr),
            getdir: None,
            getxattr: Some(getxattr),
            getxtimes: None,
            init: None,
            ioctl: Some(ioctl),
            link: None,
            listxattr: Some(listxattr),
            lock: None,
            mkdir: Some(mkdir),
            mknod: Some(mknod),
            open: Some(open),
            opendir: Some(opendir),
            poll: Some(poll),
            read: Some(read),
            read_buf: None,
            readdir: Some(readdir),
            readlink: Some(readlink),
            release: Some(release),
            releasedir: Some(releasedir),
            removexattr: Some(removexattr),
            rename: Some(rename),
            reserved00: None,
            reserved01: None,
            reserved02: None,
            rmdir: Some(rmdir),
            setxattr: Some(setxattr),
            setattr_x: Some(setattr_x),
            setbkuptime: None,
            setchgtime: Some(setchgtime),
            setcrtime: Some(setcrtime),
            setvolname: None,
            statfs: Some(statfs),
            statfs_x: None,
            symlink: Some(symlink),
            truncate: Some(truncate),
            unlink: Some(unlink),
            utime: Some(utime),
            utimens: Some(utimens),
            write: Some(write),
            write_buf: None,

            _bitfield_1: Default::default(),
        };
    }
}

pub struct MountHandle {
    mountpoint: PathBuf,
    loop_join: Option<thread::JoinHandle<i32>>,
    handle: Arc<FuseHandle>,
    user_data: *const c_void,
}

impl MountHandle {
    fn new(
        mountpoint: &Path,
        handle: Arc<FuseHandle>,
        loop_join: thread::JoinHandle<i32>,
        user_data: *const c_void,
    ) -> Self {
        Self {
            mountpoint: mountpoint.to_owned(),
            handle,
            loop_join: Some(loop_join),
            user_data,
        }
    }

    /// Waits for the fuse_loop event loop to terminate.  This can block indefinitely if it is not
    /// part of the fuse shutdown process.  This consumes the thread's join handle, so it only
    /// ever runs once.
    pub fn wait(&mut self) -> Option<i32> {
        debug!(target: FUSE_TAG, "Waiting for fuse_loop to terminate...");
        // it may have been already consumed by a previous MountHandle::wait() call
        if self.loop_join.is_some() {
            let ret_val = self.loop_join.take().unwrap().join().ok();
            debug!(
                target: FUSE_TAG,
                "fuse_loop has terminated with {:?}", ret_val
            );
            ret_val
        } else {
            debug!(target: FUSE_TAG, "fuse_loop was already joined, skipping");
            None
        }
    }
}
impl Drop for MountHandle {
    fn drop(&mut self) {
        info!(target: FUSE_TAG, "Unmounting {:?}", self.mountpoint);
        let mount_char = CString::new(self.mountpoint.to_str().unwrap())
            .unwrap()
            .into_raw();

        self.handle.disable();

        // if we don't sleep, we sometimes get a:
        //     fuse_kern_chan.c:67: fuse_kern_chan_send: Assertion `se != NULL' failed
        std::thread::sleep(std::time::Duration::from_millis(100));

        unsafe {
            debug!(target: FUSE_TAG, "Calling fuse_exit");
            // exits our fuse_loop
            fuse_exit(self.handle.handle_struct.load(Ordering::Relaxed));

            debug!(target: FUSE_TAG, "Calling fuse_unmount");
            // unmounts the file system and destroys the comm channel
            fuse_unmount(
                mount_char,
                self.handle.channel_struct.load(Ordering::Relaxed),
            );

            debug!(target: FUSE_TAG, "Joining on loop handle");
            self.wait();

            debug!(target: FUSE_TAG, "Calling fuse_destroy");
            // destroys the fuse handle
            fuse_destroy(self.handle.handle_struct.load(Ordering::Relaxed));

            // clean up some memory
            CString::from_raw(mount_char);

            // releases our leaked Filesystem memory
            let boxed = self.user_data as *mut &mut dyn Filesystem;
            let ops = Box::from_raw(*boxed);
            drop(ops);
        }
        info!(
            target: FUSE_TAG,
            "Done unmounting {}",
            self.mountpoint.display()
        );
    }
}

#[derive(Debug)]
pub enum MountError {
    BadFuseChannel,
    BadFuseHandle,
    LoopDied,
}

impl std::error::Error for MountError {}
impl std::fmt::Display for MountError {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "{:?}", self) // just use Debug for now
    }
}

#[allow(unused_mut)]
/// mount mounts the filesystem.
pub fn mount<T>(
    mountpoint: &Path,
    ops: T,
    serial_ops: bool,
    fuse_conf: FuseConfig,
    mount_conf: MountConfig,
) -> Result<Arc<Mutex<MountHandle>>, MountError>
where
    T: Filesystem + Send + Sync + 'static,
{
    //args.push(mountpoint.to_str().unwrap());

    let fuse_args: Vec<String> = fuse_conf.into();
    debug!(target: FUSE_TAG, "Aggregating fuse args {:?}", fuse_args);
    let mut fuse_argv: Vec<*mut c_char> = Vec::new();
    fuse_argv.push(CString::new("supertag").expect("CString failed").into_raw() as *mut c_char);
    for arg in fuse_args {
        fuse_argv.push(CString::new(arg).expect("CString failed").into_raw() as *mut c_char);
    }

    let mount_args: Vec<String> = mount_conf.into();
    debug!(target: FUSE_TAG, "Aggregating mount args {:?}", mount_args);
    let mut mount_argv: Vec<*mut c_char> = Vec::new();
    mount_argv.push(CString::new("supertag").expect("CString failed").into_raw() as *mut c_char);
    for arg in mount_args {
        mount_argv.push(CString::new(arg).expect("CString failed").into_raw() as *mut c_char);
    }

    // What is happening here is fairly complicated and nuanced.  There are subtleties here that
    // are easy to miss.  Basically what we're doing first is moving ops onto the heap and then ensuring
    // it won't be dropped.  We do this by leaking the box, which is fine because `mount` is only
    // ever called once per program run, so we're not leaking much.  The reason we need
    // to do this is because we need to cast `ops` to a `&dyn Filesystem`, and if `ops` gets dropped
    // at the end of this function, we would SEGFAULT.  This is not obvious, doing that compiles:
    //
    //     let trait_ref: &dyn Filesystem = &ops;
    //     let user_data = Box::into_raw(Box::new(trait_ref)) as *const c_void;
    //
    // but it WILL segfault when the function returns, drops `ops`, and a request is made to Supertag,
    // which uses the user_data.
    //
    // You may think the solution is: "Oh, well `ops` needs to live as long as the fuse_loop thread,
    // so let's move `ops` into that thread and everything will be fine."  This *doesn't* work
    // because when `ops` is moved into the thread's closure, it now has a new address, and the
    // `&ops` reference will be broken, resulting in SEGFAULT.
    //
    // So now that we have a 'static reference to dyn Filesystem, we need to make it into a usize
    // pointer.  We can't use it as-is because a `&dyn T` is 2 usize: 1 for the data, 1 for the
    // vtable.  So we put that into a box, and a box is 1 usize.  We then `Box::into_raw()` to get
    // our single pointer, and to ensure that the `&dyn T` it holds is never dropped (because
    // B`ox::into_raw()` leaks the contents).
    //
    // So to re-cap, ops is moved into this function, then we move it to the heap and ensure it is
    // never dropped.  Then we take a reference to it and cast it into a Filesystem trait.  Then
    // we put that trait reference into a box because we want a single usize'd pointer whose
    // contents won't be dropped.  Then we cast it to `*const c_void` and pass that to fuse.
    let trait_ref: &'static dyn Filesystem = Box::leak(Box::new(ops));
    let user_data = Box::into_raw(Box::new(trait_ref)) as *const c_void;

    let low_level_ops = FuseOperations::new();

    // FIXME does this leak?
    let mount_char = CString::new(mountpoint.to_str().unwrap())
        .unwrap()
        .into_raw();
    let fuse_args_struct = &mut fuse_args {
        argc: fuse_argv.len() as c_int,
        argv: fuse_argv.as_mut_ptr(),
        allocated: 0,
    } as *mut fuse_args;

    let mount_args_struct = &mut fuse_args {
        argc: mount_argv.len() as c_int,
        argv: mount_argv.as_mut_ptr(),
        allocated: 0,
    } as *mut fuse_args;

    debug!(target: FUSE_TAG, "Mounting {:?}", mountpoint);
    let chan = AtomicPtr::new(unsafe { fuse_mount(mount_char, mount_args_struct) });

    if chan.load(Ordering::Relaxed).is_null() {
        error!(target: FUSE_TAG, "fuse_chan was NULL!");
        return Err(MountError::BadFuseChannel);
    }

    debug!(target: FUSE_TAG, "Creating fuse handle");
    let handle = AtomicPtr::new(unsafe {
        fuse_new(
            chan.load(Ordering::Relaxed),
            fuse_args_struct,
            &low_level_ops,
            size_of::<FuseOperations>(),
            user_data as *mut c_void,
        )
    });

    if handle.load(Ordering::Relaxed).is_null() {
        error!(target: FUSE_TAG, "fuse handle was NULL!");
        unsafe {
            fuse_unmount(mount_char, chan.load(Ordering::Relaxed));
        }
        return Err(MountError::BadFuseHandle);
    }

    unsafe {
        debug!(target: FUSE_TAG, "Installing fuse signal handlers");
        let session_handle = fuse_get_session(handle.load(Ordering::Relaxed));
        let success = fuse_set_signal_handlers(session_handle);
        if success != 0 {
            error!(
                target: FUSE_TAG,
                "Unable to install signal handlers, continuing anyway"
            );
        }
    }

    let fuse_handle = Arc::new(FuseHandle {
        disabled: AtomicBool::new(false),
        handle_struct: handle,
        channel_struct: chan,
    });

    let (tx, rx) = mpsc::sync_channel(1);
    let join_handle: thread::JoinHandle<i32>;
    {
        let fuse_handle = fuse_handle.clone();
        debug!(target: FUSE_TAG, "Starting fuse_loop thread");
        join_handle = thread::Builder::new()
            .name("fuse_loop".to_string())
            .spawn(move || {
                let handle = {
                    if serial_ops {
                        // a single-threaded blocking event dispatch loop
                        // FIXME use return code somehow?
                        let _ = tx.send(true);
                        unsafe { fuse_loop(fuse_handle.handle_struct.load(Ordering::Relaxed)) }
                    } else {
                        // despite the "mt" (multithreaded) this is still a blocking loop.  the only
                        // difference between it and fuse_loop is that it will potentially spin up a thread for
                        // each request it receives.  because of this, any implementations of Filesystem should be
                        // thread safe
                        // FIXME use return code
                        let _ = tx.send(true);
                        unsafe { fuse_loop_mt(fuse_handle.handle_struct.load(Ordering::Relaxed)) }
                    }
                };
                debug!(target: FUSE_TAG, "Stopped fuse_loop thread");
                handle
            })
            .expect("Coudln't spawn join thread");
        debug!(
            target: FUSE_TAG,
            "Started fuse_loop thread with id {:?}",
            join_handle.thread().id()
        );
    }

    // It seems that if we don't ensure that fuse_loop has been running for a short amount of time
    // we can non-deterministically receive:
    // fuse_kern_chan.c:67: fuse_kern_chan_send: Assertion `se != NULL' failed.
    // on ubuntu 18.04 LTS, 4.15.0-72-generic, x86_64, fuse 2.9.7
    if let Err(mpsc::RecvError) = rx.recv() {
        return Err(MountError::LoopDied);
    }
    thread::sleep(std::time::Duration::from_millis(400));

    let mount_handle = Arc::new(Mutex::new(MountHandle::new(
        mountpoint,
        fuse_handle.clone(),
        join_handle,
        user_data,
    )));

    // this is ugly but the only way to use our passed in Filesystem, since by this point, it has
    // been moved into a box and leaked into userdata
    unsafe {
        let boxed = user_data as *mut &mut dyn Filesystem;
        (*boxed).set_handle(fuse_handle.clone());
    }

    Ok(mount_handle)
}
