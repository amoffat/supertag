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

use crate::common::constants::DEVICE_ID;
use crate::common::types::file_perms::Permissions;
use crate::common::types::UtcDt;
use crate::sql::types::TaggedFile;
use fuse_sys::conf::{FuseConfig, MountConfig};
use fuse_sys::{stat, timespec, O_RDWR, O_WRONLY};
use libc::{mode_t, S_IFDIR, S_IFLNK, S_IFREG};
use log::{debug, info};
use std::convert::TryInto;
use std::ffi::CString;
use std::fs::OpenOptions;
#[cfg(target_os = "macos")]
use std::hash::Hasher;
use std::os::raw::{c_char, c_void};
use std::path::Path;

const UTIL_TAG: &str = "util";

struct Stat {
    device: u64,
    inode: u64,
    mode: mode_t,
    nlink: u64,
    uid: u32,
    gid: u32,
    size: i64,
    atime: timespec,
    mtime: timespec,
    ctime: timespec,
    #[cfg(target_os = "macos")]
    birthtime: timespec,
}

impl From<Stat> for stat {
    fn from(s: Stat) -> Self {
        #[cfg(target_os = "linux")]
        return stat {
            st_dev: s.device,
            st_ino: s.inode,
            // good read: https://sourceforge.net/p/fuse/mailman/message/29281571/
            st_nlink: s.nlink,
            st_mode: s.mode,
            st_uid: s.uid,
            st_gid: s.gid,
            __pad0: 0,
            st_rdev: 0,
            st_size: s.size,
            st_blksize: 4096,
            st_blocks: 8,
            st_atim: s.atime,
            st_mtim: s.mtime,
            st_ctim: s.ctime,
            __glibc_reserved: [0; 3],
        };

        #[cfg(target_os = "macos")]
        return stat {
            st_dev: s.device as i32,
            st_mode: s.mode as u16,
            st_nlink: s.nlink as u16,
            st_ino: s.inode,
            st_uid: s.uid,
            st_gid: s.gid,
            st_rdev: 0,
            st_atimespec: s.atime,
            st_mtimespec: s.mtime,
            st_ctimespec: s.ctime,
            st_birthtimespec: s.birthtime,
            st_size: s.size,
            st_blocks: 8,
            // https://github.com/libfuse/sshfs/blob/master/sshfs.c#L3394-L3396 ????
            st_blksize: 0,
            st_flags: 0,
            st_gen: 0,
            st_lspare: 0,
            st_qspare: [0; 2],
        };
    }
}

pub fn new_dir(mtime: &UtcDt, uid: u32, gid: u32, perm: &Permissions, num_files: i64) -> stat {
    let ts = utcdt_to_timespec(mtime);
    Stat {
        device: DEVICE_ID,
        inode: 1,
        mode: S_IFDIR | perm.mode(),
        // good read: https://sourceforge.net/p/fuse/mailman/message/29281571/
        nlink: 2,
        uid,
        gid,
        size: num_files,
        atime: ts,
        mtime: ts,
        ctime: ts,
        #[cfg(target_os = "macos")]
        birthtime: ts,
    }
    .into()
}

pub fn new_statfile(tf: TaggedFile) -> stat {
    //if tf.managed_file {
    //new_regfile(&tf.mtime, tf.uid, tf.gid, &tf.permissions, tf.size)
    //new_link(&tf.mtime, tf.uid, tf.gid, &tf.permissions, tf.size)
    //} else {
    new_link(&tf.mtime, tf.uid, tf.gid, &tf.permissions, tf.path.len())
    //}
}

pub fn new_link(mtime: &UtcDt, uid: u32, gid: u32, perm: &Permissions, size: usize) -> stat {
    let ts = utcdt_to_timespec(mtime);
    Stat {
        device: DEVICE_ID,
        inode: 1,
        mode: S_IFLNK | perm.mode(),
        nlink: 1,
        uid,
        gid,
        size: size as i64,
        atime: ts,
        mtime: ts,
        ctime: ts,
        #[cfg(target_os = "macos")]
        birthtime: ts,
    }
    .into()
}

pub fn new_alias(
    _btime: &UtcDt,
    mtime: &UtcDt,
    ctime: &UtcDt,
    size: usize,
    uid: u32,
    gid: u32,
    mode: mode_t,
) -> stat {
    let mtime_ts = utcdt_to_timespec(mtime);
    #[cfg(target_os = "macos")]
    let btime_ts = utcdt_to_timespec(_btime);
    let ctime_ts = utcdt_to_timespec(ctime);
    Stat {
        device: DEVICE_ID,
        inode: 1,
        mode,
        nlink: 1,
        uid,
        gid,
        size: size as i64,
        atime: mtime_ts,
        mtime: mtime_ts,
        ctime: ctime_ts,
        #[cfg(target_os = "macos")]
        birthtime: btime_ts,
    }
    .into()
}

pub fn new_regfile(mtime: &UtcDt, uid: u32, gid: u32, perm: &Permissions, size: usize) -> stat {
    let ts = utcdt_to_timespec(mtime);
    Stat {
        device: DEVICE_ID,
        inode: 1,
        mode: S_IFREG | perm.mode(),
        nlink: 1,
        uid,
        gid,
        size: size as i64,
        atime: ts,
        mtime: ts,
        ctime: ts,
        #[cfg(target_os = "macos")]
        birthtime: ts,
    }
    .into()
}

pub fn mac_volume_icon(uid: u32, gid: u32, mtime: &UtcDt) -> stat {
    let ts = utcdt_to_timespec(mtime);
    Stat {
        device: DEVICE_ID,
        inode: 1,
        mode: S_IFREG | Permissions::from(0o777).mode(),
        nlink: 1,
        uid,
        gid,
        size: 100000, // FIXME
        atime: ts,
        mtime: ts,
        ctime: ts,
        #[cfg(target_os = "macos")]
        birthtime: ts,
    }
    .into()
}

pub fn db_file(uid: u32, gid: u32, mtime: &UtcDt) -> stat {
    new_link(mtime, uid, gid, &Permissions::from(0o777), 0)
}

fn utcdt_to_timespec(dt: &UtcDt) -> timespec {
    timespec {
        tv_sec: dt.timestamp(),
        tv_nsec: dt.timestamp_subsec_nanos() as i64,
    }
}

#[cfg(target_os = "macos")]
fn make_mount_name(col: &str) -> String {
    format!("{}", col)
}

fn make_fs_name<P: AsRef<Path>>(col: &str, _db_file: P) -> String {
    format!("supertag:{}", AsRef::<Path>::as_ref(col).display())
}

#[cfg(target_os = "macos")]
fn compute_fsid(col: &str) -> i32 {
    let mut hasher = metrohash::MetroHash64::new();
    hasher.write(col.as_bytes());

    // fsid must be a 24-bit value
    (hasher.finish() & 0xffffff) as i32
}

pub fn make_mount_config<P: AsRef<Path>>(collection: &str, db_path: P) -> MountConfig {
    let mut mount_conf = MountConfig::default();
    mount_conf.direct_io = Some(true);
    mount_conf.fsname = Some(make_fs_name(collection, &db_path));
    mount_conf.subtype = Some("manifold".to_string());
    mount_conf.default_permissions = Some(true); // enable kernel-enforced permission checks
    mount_conf.allow_other = Some(true); // allow other users to access files

    #[cfg(target_os = "macos")]
    {
        mount_conf.volname = Some(make_mount_name(collection));
        mount_conf.local = Some(true); // necessary so it appears in finder sidebar
        mount_conf.noappledouble = Some(true);

        // this MUST be false so that we can create finder aliases, which require calls to setattr_x
        mount_conf.noapplexattr = Some(false);

        mount_conf.auto_cache = Some(true);
        mount_conf.kill_on_unmount = Some(true);
        mount_conf.nolocalcaches = Some(true);
        mount_conf.noubc = Some(true);
        mount_conf.negative_vncache = Some(false);
        mount_conf.novncache = Some(true);
        mount_conf.fsid = Some(compute_fsid(collection));

        // despite us setting up signal handlers correctly via fuse_set_signal_handlers, the call
        // to fuse_unmount will hang for about 120 seconds on macos.  lowering this will reduce
        // that time to 2n (for some reason, these seconds are multiplied by 2)
        mount_conf.daemon_timeout = Some(5);
    }
    mount_conf
}

pub fn make_fuse_config(_volicon: Option<&Path>) -> FuseConfig {
    let mut fuse_conf = FuseConfig::default();
    // the database can be changed directly through the tag command, so tell fuse not to cache
    // the file and directory metadata
    fuse_conf.attr_timeout = Some(0);
    fuse_conf.entry_timeout = Some(0);
    fuse_conf.hard_remove = Some(true);
    fuse_conf.kernel_cache = Some(false);

    #[cfg(target_os = "macos")]
    {
        if let Some(icon) = _volicon {
            fuse_conf.modules = Some(vec![format!("volicon,iconpath={}", icon.display())]);
        }
    }

    fuse_conf
}

pub fn open_opts_from_mode(opts: &mut OpenOptions, mode: i32) -> &OpenOptions {
    // O_RDONLY is 0 on my system, so we start with this, since we can't bitwise test for it like the others
    // FIXME is this a portable assumption?
    let mut fopts = opts.read(true).write(false);

    let mode = mode as u32;
    if mode & O_RDWR > 0 {
        fopts = fopts.read(true).write(true)
    } else if mode & O_WRONLY > 0 {
        fopts = fopts.read(false).write(true)
    }
    fopts
}

pub fn truncate(path: &Path, offset: i64) -> std::io::Result<()> {
    let c_path = CString::new(path.to_string_lossy().to_string()).unwrap();
    let err;
    unsafe { err = libc::truncate(c_path.as_ptr(), offset) }
    if err == -1 {
        Err(std::io::Error::last_os_error().into())
    } else {
        Ok(())
    }
}

pub fn getxattr(path: &Path, name: &str, position: u32) -> std::io::Result<Vec<u8>> {
    info!(
        target: UTIL_TAG,
        "getxattr {} on {:?}, position {}", name, path, position
    );

    let c_path = CString::new(path.to_string_lossy().to_string()).unwrap();
    let c_name = CString::new(name).unwrap();
    let desired_size: isize;

    #[cfg(target_os = "linux")]
    unsafe {
        desired_size = libc::getxattr(
            c_path.as_ptr(),
            c_name.as_ptr() as *const c_char,
            std::ptr::null_mut(),
            0,
        )
        .try_into()
        .unwrap();
    }
    #[cfg(target_os = "macos")]
    unsafe {
        desired_size = libc::getxattr(
            c_path.as_ptr(),
            c_name.as_ptr() as *const c_char,
            std::ptr::null_mut(),
            0,
            position,
            0, // options
        )
    }

    if desired_size == -1 {
        return Err(std::io::Error::last_os_error().into());
    }

    let mut value: Vec<u8> = Vec::new();
    value.resize(desired_size as usize, 0);
    let read_size: isize;

    #[cfg(target_os = "linux")]
    unsafe {
        read_size = libc::getxattr(
            c_path.as_ptr(),
            c_name.as_ptr() as *const c_char,
            value.as_mut_ptr() as *mut c_void,
            desired_size.try_into().unwrap(),
        )
    }
    #[cfg(target_os = "macos")]
    unsafe {
        read_size = libc::getxattr(
            c_path.as_ptr(),
            c_name.as_ptr() as *const c_char,
            value.as_mut_ptr() as *mut c_void,
            desired_size.try_into().unwrap(),
            position,
            0, // options
        )
    }

    if read_size == -1 {
        Err(std::io::Error::last_os_error().into())
    } else {
        Ok(value)
    }
}

pub fn setxattr(
    path: &Path,
    name: &str,
    value: &[u8],
    _position: u32,
    flags: i32,
) -> std::io::Result<()> {
    info!(
        target: UTIL_TAG,
        "setxattr {} on {:?}, flags {}", name, path, flags
    );

    let c_path = CString::new(path.to_string_lossy().to_string())?;
    let c_name = CString::new(name)?;
    let err;

    #[cfg(target_os = "linux")]
    unsafe {
        err = libc::setxattr(
            c_path.as_ptr(),
            c_name.as_ptr() as *const c_char,
            value.as_ptr() as *const c_void,
            value.len(),
            flags,
        );
    }
    #[cfg(target_os = "macos")]
    unsafe {
        err = libc::setxattr(
            c_path.as_ptr(),
            c_name.as_ptr(),
            value.as_ptr() as *const c_void,
            value.len(),
            _position,
            0, // FIXME  this works but is it correct?  macos is sending 8 for flags, which causes an error
        );
    }

    if err == -1 {
        Err(std::io::Error::last_os_error().into())
    } else {
        Ok(())
    }
}

pub fn listxattr(path: &Path, options: i32) -> std::io::Result<Vec<String>> {
    info!(
        target: UTIL_TAG,
        "listxattr on {:?}, options {}", path, options
    );
    let c_path = CString::new(path.to_string_lossy().to_string())?;
    let err_or_size;

    // first get the size we need to allocate
    #[cfg(target_os = "linux")]
    unsafe {
        err_or_size = libc::listxattr(c_path.as_ptr(), std::ptr::null_mut(), 0);
    }
    #[cfg(target_os = "macos")]
    unsafe {
        err_or_size = libc::listxattr(c_path.as_ptr(), std::ptr::null_mut(), 0, options);
    }

    if err_or_size == -1 {
        return Err(std::io::Error::last_os_error().into());
    }

    let mut buf: Vec<u8> = vec![0; err_or_size as usize];
    let err_or_size;

    #[cfg(target_os = "linux")]
    unsafe {
        err_or_size = libc::listxattr(c_path.as_ptr(), buf.as_mut_ptr() as *mut i8, buf.len());
    }
    #[cfg(target_os = "macos")]
    unsafe {
        err_or_size = libc::listxattr(
            c_path.as_ptr(),
            buf.as_mut_ptr() as *mut i8,
            buf.len(),
            options,
        );
    }

    if err_or_size == -1 {
        return Err(std::io::Error::last_os_error().into());
    }

    debug!(target: UTIL_TAG, "Fetched xattr buffer {:?}", buf);
    let mut attrs = vec![];
    for chunk in buf.split(|c| c == &0u8) {
        if chunk.is_empty() {
            continue;
        }
        let name = String::from_utf8_lossy(chunk).to_string();
        attrs.push(name);
    }
    debug!(
        target: UTIL_TAG,
        "Fetched xattrs resulted in parsed attrs {:?}", attrs
    );

    Ok(attrs)
}

pub fn removexattr(path: &Path, name: &str, options: i32) -> std::io::Result<()> {
    info!(
        target: UTIL_TAG,
        "removexattr {} on {:?}, options {}", name, path, options
    );
    let c_path = CString::new(path.to_string_lossy().to_string())?;
    let c_name = CString::new(name)?;
    let err;

    #[cfg(target_os = "linux")]
    unsafe {
        err = libc::removexattr(c_path.as_ptr(), c_name.as_ptr());
    }
    #[cfg(target_os = "macos")]
    unsafe {
        err = libc::removexattr(c_path.as_ptr(), c_name.as_ptr(), options);
    }

    if err == -1 {
        Err(std::io::Error::last_os_error().into())
    } else {
        Ok(())
    }
}
