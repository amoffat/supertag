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

use super::err::SupertagShimError;
use crate::common::err::{STagError, STagResult};
use crate::common::settings::Settings;
use crate::common::types::{TagCollection, TagType, UtcDt};
use crate::common::{constants, get_filename};
use crate::fuse::opcache;
use crate::fuse::opcache::ReaddirCacheEntry;
use crate::fuse::util::open_opts_from_mode;
use crate::sql::tpool::ThreadConnPool;
use crate::{common, sql};
use common::types::file_perms::Permissions;
use fuse_sys::err::FuseErrno;
use fuse_sys::{fuse_file_info, mode_t, new_statvfs, off_t, stat, statvfs};
use fuse_sys::{FileEntry, Filesystem, FuseHandle, FuseResult, Request};
use log::{debug, error, info, warn};
use nix::errno::Errno::{EIO, ENOENT, ENOSYS, EPERM};
use parking_lot::Mutex;
use rusqlite::{Connection, TransactionBehavior};
use std::borrow::Borrow;
use std::convert::TryInto;
use std::fs::OpenOptions;
#[cfg(target_os = "macos")]
use std::os::unix::io::AsRawFd;
use std::os::unix::io::{IntoRawFd, RawFd};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

const OP_TAG: &str = "supertag_op";

mod getattr;
mod readdir;

#[cfg(target_os = "macos")]
mod xattr;

pub struct TagFilesystem<N>
where
    N: common::notify::Notifier + 'static,
{
    conn_pool: Arc<ThreadConnPool>,
    op_cache: Arc<opcache::OpCache>,
    settings: Arc<Settings>,
    handle: Option<Arc<FuseHandle>>,
    notifier: Arc<Mutex<N>>,

    // we'll use this as a weak reference in our infinite-loop threads, so they can exit when TagFilesystem is dropped
    #[allow(dead_code)]
    threads_done: Arc<AtomicBool>,
}

impl<N> Drop for TagFilesystem<N>
where
    N: common::notify::Notifier,
{
    fn drop(&mut self) {
        debug!(target: OP_TAG, "Dropping fs");

        self.threads_done.store(true, Ordering::Relaxed);
    }
}

impl<N> TagFilesystem<N>
where
    N: common::notify::Notifier,
{
    #[must_use]
    pub fn new(
        settings: Arc<Settings>,
        conn_pool: ThreadConnPool,
        notifier: Arc<Mutex<N>>,
    ) -> TagFilesystem<N> {
        let conn_pool_arc = Arc::new(conn_pool);
        let op_cache = Arc::new(opcache::OpCache::new(settings.clone()));
        let threads_done = Arc::new(AtomicBool::new(false));

        TagFilesystem {
            conn_pool: conn_pool_arc,
            op_cache,
            settings,
            handle: None,
            notifier,
            threads_done,
        }
    }

    /// A convenience method for removing a tagdir and its filedir from the readdir cache
    fn flush_readdir_cache(&self, path: &Path) {
        self.op_cache.clear_readdir_entry(&path);
        self.flush_filedir_cache(path);
    }

    fn flush_filedir_cache(&self, path: &Path) {
        let conf = self.settings.get_config();
        self.op_cache
            .clear_readdir_entry(&path.join(&conf.symbols.filedir_str));
        self.op_cache
            .clear_readdir_entry(&path.join(&conf.symbols.filedir_cli_str));
    }

    /// For every tag in `path`, flush it
    fn flush_paths_tags(&self, path: &Path) {
        // this flushes all of the tags that contain the file removed. this is necessary because these tags
        // may exist in the readdir cache with the wrong size/num_files count now
        for comp in path.components() {
            if let Component::Normal(name) = comp {
                let to_flush = PathBuf::from(std::path::MAIN_SEPARATOR.to_string()).join(name);
                self.flush_readdir_cache(&to_flush);
            }
        }
    }

    pub fn strip_sync_char<P: AsRef<Path>>(&self, path: P) -> Option<PathBuf> {
        let mut fname = common::get_filename(path.as_ref()).unwrap().to_owned();
        if fname.ends_with(self.settings.get_config().symbols.sync_char) {
            fname.pop();
            let stripped = path.as_ref().parent().unwrap().join(&fname);
            Some(stripped)
        } else {
            None
        }
    }

    fn get_root_mtime(&self, default_conn: Option<&Connection>) -> STagResult<UtcDt> {
        match default_conn {
            Some(conn) => sql::get_root_mtime(conn).map_err(STagError::from),
            None => {
                let conn_lock = self.conn_pool.get_conn();
                let conn = conn_lock.lock();
                let real_conn = (*conn).borrow_mut();
                sql::get_root_mtime(&real_conn).map_err(STagError::from)
            }
        }
    }

    /// Processes an alias record that has been flushed or released
    #[cfg(target_os = "macos")]
    fn process_alias(&self, path: &Path) -> FuseResult<()> {
        match self.op_cache.check_alias_entry(path) {
            Some(alias_rc) => {
                info!(target: OP_TAG, "Processing alias record {}", path.display(),);
                let mut alias = alias_rc.lock();

                if alias.is_valid() {
                    if !alias.linked {
                        debug!(
                            target: OP_TAG,
                            "Alias is valid and currently not linked, linking it"
                        );

                        let mut tags = TagCollection::new(&self.settings, path);
                        // we pop because path is a full file path, and we don't want our tags to include our
                        // filename
                        tags.pop();

                        let (alias_file, alias_target) = {
                            debug!(
                                target: OP_TAG,
                                "Alias-resolving managed file {}",
                                alias.managed_file.display()
                            );

                            // get the real file that our macos alias points to
                            let alias_target =
                                crate::platform::mac::alias::recursive_resolve_alias(
                                    &alias.managed_file,
                                )
                                .map_err(SupertagShimError::from)?
                                .canonicalize()?;

                            // a heuristic to check if we're creating an alias in the root directory.
                            if alias_target.is_file() && tags.len() == 0 {
                                let _ = self.notifier.lock().dragged_to_root();
                                return Err(EIO.into());
                            }

                            debug!(
                                target: OP_TAG,
                                "Resolved {} to real file {}.  Exists? {}",
                                alias.managed_file.display(),
                                alias_target.display(),
                                alias_target.exists(),
                            );

                            // and move it to a more "real" location
                            let alias_file = self.settings.managed_save_path(
                                &alias.managed_file,
                                &self.settings.get_collection(),
                            );

                            debug!(
                                target: OP_TAG,
                                "Putting {} in its final resting place {}",
                                alias.managed_file.display(),
                                alias_file.display(),
                            );

                            // only if the file doesn't exist should we create it.  if it does exist, it means it's a
                            // file that already is linked into supertag, and we need to preserve its inode
                            if !alias_file.exists() {
                                debug!(
                                    target: OP_TAG,
                                    "Final managed file {} doesn't exist, creating via rename from {}",
                                    alias_file.display(),
                                    alias.managed_file.display()
                                );
                                common::xattr::rename(&alias.managed_file, &alias_file)?;
                            }
                            // since we're not renaming it away, let's remove it
                            else {
                                debug!(
                                    target: OP_TAG,
                                    "Final managed file {} already exists, just removing old {}",
                                    alias_file.display(),
                                    alias.managed_file.display()
                                );
                                std::fs::remove_file(&alias.managed_file)?;
                            }
                            (alias_file, alias_target)
                        };

                        let conn_lock = self.conn_pool.get_conn();
                        let conn = conn_lock.lock();
                        let mut real_conn = (*conn).borrow_mut();
                        let tx = real_conn
                            .transaction_with_behavior(TransactionBehavior::Exclusive)
                            .map_err(SupertagShimError::from)?;

                        let primary_tag = get_filename(&alias_target)?;

                        let _res = common::fsops::ln(
                            self.settings.borrow(),
                            &tx,
                            &alias_target,
                            &tags.join_path(&self.settings),
                            &primary_tag,
                            alias.uid,
                            alias.gid,
                            &alias.umask,
                            Some(&alias_file),
                            &*(self.notifier.lock()),
                        )
                        .map_err(SupertagShimError::from)?;

                        tx.commit().map_err(SupertagShimError::from)?;
                        alias.linked = true;

                        // here we update the managed file to be the final file location. this only really changes on
                        // macos, but what it allows us to do is to set xattrs on the real final file. this is needed
                        // because macos will only set the "alias file" xattrs after the file has been released, and we
                        // need those settings on the final file, not the intermediate managed file
                        alias.managed_file = alias_file;

                        self.flush_paths_tags(path);
                    }
                }
            }
            None => {}
        }

        Ok(())
    }

    /// Takes a path and attempts to resolve it to its underlying file path, but only if we're dealing with a managed
    /// file.  This is because we might do unsafe operations on it, like truncate, and we only want to do it on a file
    /// that we control, ie a MacOS Alias
    fn resolve_to_alias_file(&self, conn: &Connection, path: &Path) -> FuseResult<Option<PathBuf>> {
        debug!(
            target: OP_TAG,
            "Attempting to resolve to {} to a managed file",
            path.display()
        );
        let tags = TagCollection::new(&self.settings, path);

        // it's not necessarily an error if there's only the root tag
        let maybe_pt = tags.primary_type();
        match maybe_pt {
            Err(STagError::NotEnoughTags) => {
                return Ok(None);
            }
            Err(e) => return Err(e.into()),
            _ => {}
        }
        let pt = maybe_pt.unwrap();

        // here we'll short circuit if we find our entry in the readdir cache or alias cache
        match pt {
            TagType::DeviceFileSymlink(_) | TagType::Symlink(_) => {
                if let Some(opcache::ReaddirCacheEntry::File(file)) =
                    self.op_cache.check_readdir_entry(path)
                {
                    if let Some(alias_file) = file.alias_file {
                        debug!(
                            target: OP_TAG,
                            "Found it in the readdir cache, using {}", alias_file
                        );
                        return Ok(Some(PathBuf::from(alias_file)));
                    } else {
                        warn!(
                            target: OP_TAG,
                            "Item {} in readdir cache was not a managed file, aborting",
                            path.display()
                        );
                        return Ok(None);
                    }
                }
            }
            _ => {}
        }

        // alias entries can often appear *outside* of the `_` filedir, so we don't put them underneath a `Symlink`
        // tagtype, like we do with the readdir cache check above.  This is because a user could drag and drop the
        // file directly onto the tag itself, and not the filedir, resulting in queries to `resolve_mf_path` to use
        // a path that does not contain a filedir
        if let Some(bmark_rc) = self.op_cache.check_alias_entry(path) {
            debug!(target: OP_TAG, "Found a alias cache entry, using that");
            let guard = bmark_rc.lock();
            return Ok(Some(guard.managed_file.clone()));
        }

        // if it wasn't in our opcaches, we need to do an actual lookup

        debug!(
            target: OP_TAG,
            "Looking up {} in the database",
            path.display()
        );

        let found = match pt {
            TagType::DeviceFileSymlink(device_file) => {
                sql::contains_file(conn, tags.all_but_last().as_slice(), |tf| {
                    device_file.matches(tf)
                })
                .map_err(SupertagShimError::from)?
            }
            TagType::Symlink(primary_tag) => {
                sql::contains_file(conn, tags.all_but_last().as_slice(), |tf| {
                    &tf.primary_tag == primary_tag
                })
                .map_err(SupertagShimError::from)?
            }
            _ => None,
        };

        match found {
            Some(match_file) => {
                if let Some(alias_file) = match_file.alias_file {
                    Ok(Some(PathBuf::from(alias_file)))
                } else {
                    Ok(None)
                }
            }
            _ => Ok(None),
        }
    }
}

impl<N> Filesystem for TagFilesystem<N>
where
    N: common::notify::Notifier + 'static,
{
    /// Sets up our thread-local request id based on a global atomic request counter
    fn init_request_id(&self) {
        common::log::REQUEST_ID.with(|f| {
            let req_id = common::log::REQ_COUNTER.fetch_add(1, Ordering::SeqCst);
            *f.borrow_mut() = req_id;
        });
    }

    fn getattr(&self, req: &Request, path: &Path) -> FuseResult<stat> {
        self.getattr_impl(req, path)
    }

    fn readdir(
        &self,
        req: &Request,
        path: &Path,
    ) -> FuseResult<Box<dyn Iterator<Item = FileEntry>>> {
        self.readdir_impl(req, path)
    }

    fn readdir_common(
        &self,
        req: &Request,
        path: &Path,
    ) -> FuseResult<Box<dyn Iterator<Item = FileEntry>>> {
        self.readdir_common_impl(req, path)
    }

    fn readlink(&self, _req: &Request, path: &Path) -> FuseResult<PathBuf> {
        let tags = TagCollection::new(&self.settings, path);

        let pt = tags.primary_type().map_err(SupertagShimError::from)?;

        if let Some(opcache::ReaddirCacheEntry::File(tf)) = self.op_cache.check_readdir_entry(path)
        {
            Ok(tf.resolve_path())
        } else {
            if let TagType::DeviceFileSymlink(device_file) = pt {
                let conn_lock = self.conn_pool.get_conn();
                let conn_guard = conn_lock.lock();
                let conn = (*conn_guard).borrow_mut();

                match sql::contains_file(&conn, tags.as_slice(), |tf| device_file.matches(tf))
                    .map_err(SupertagShimError::from)?
                {
                    Some(tf) => {
                        let entry = ReaddirCacheEntry::File(tf.clone());
                        self.op_cache.add_readdir_entry(path, entry);
                        Ok(tf.resolve_path())
                    }
                    None => Err(ENOENT.into()),
                }
            } else if let TagType::Symlink(filename) = pt {
                let conn_lock = self.conn_pool.get_conn();
                let conn_guard = conn_lock.lock();
                let conn = (*conn_guard).borrow_mut();

                match sql::contains_file(&conn, tags.as_slice(), |tf| &tf.primary_tag == filename)
                    .map_err(SupertagShimError::from)?
                {
                    Some(tf) => {
                        let entry = ReaddirCacheEntry::File(tf.clone());
                        self.op_cache.add_readdir_entry(path, entry);
                        Ok(tf.resolve_path())
                    }
                    None => Err(ENOENT.into()),
                }
            } else if path == Path::new(common::constants::DB_FILE_PATH) {
                let col = self.settings.get_collection();
                Ok(self.settings.db_file(&col))
            } else {
                Err(ENOENT.into())
            }
        }
    }

    fn symlink(&self, req: &Request, src: &Path, dst: &Path) -> FuseResult<()> {
        let mut tags = TagCollection::new(&self.settings, dst);

        // dst will always have the filename in the path, so pop that off
        tags.pop();

        info!(
            target: OP_TAG,
            "Tagging {:?} with {:?}, umask: {}", src, tags, req.umask
        );

        // this ensures that if we attempt to make a symlink to an existing supertag symlink, that we are resolving
        // the first symlink to the real file first.  it has to be done outside of an acquired transaction, because it
        // will call readlink and deadlock otherwise
        let abs_src = std::fs::canonicalize(src)?;
        let primary_tag = get_filename(&abs_src)?;

        let conn_lock = self.conn_pool.get_conn();
        let conn = conn_lock.lock();
        let mut real_conn = (*conn).borrow_mut();
        let tx = real_conn
            .transaction_with_behavior(TransactionBehavior::Exclusive)
            .map_err(SupertagShimError::from)?;

        let res = common::fsops::ln(
            self.settings.borrow(),
            &tx,
            &abs_src,
            &tags.join_path(&self.settings),
            &primary_tag,
            req.uid,
            req.gid,
            &req.umask.into(),
            None,
            &*(self.notifier.lock()),
        )
        .map_err(SupertagShimError::from)?;
        tx.commit().map_err(SupertagShimError::from)?;

        info!(target: OP_TAG, "Tagged successfully");

        for tf in res {
            self.op_cache.add_symlink(req, dst, tf);
        }

        self.flush_paths_tags(dst);

        Ok(())
    }

    fn create(&self, _req: &Request, _path: &Path, _mode: mode_t) -> FuseResult<RawFd> {
        #[cfg(target_os = "macos")]
        {
            info!(
                target: OP_TAG,
                "Creating potential macos alias file at {}",
                _path.display()
            );

            // we used to do the drag_to_root check here, but we don't anymore, because we need to let users drag a
            // folder in a finder window

            let managed_file = self
                .settings
                .managed_save_path(_path, &self.settings.get_collection());

            let alias = self.op_cache.create_alias(
                _path,
                _mode,
                _req.umask.into(),
                _req.uid,
                _req.gid,
                managed_file,
            )?;

            // need to flush our readdir caches
            self.op_cache.clear_readdir_entry(_path);

            let guard = alias.lock();
            Ok(guard.file_handle.as_raw_fd())
        }
        #[cfg(not(target_os = "macos"))]
        {
            self.notifier
                .lock()
                .bad_copy()
                .map_err(SupertagShimError::from)?;
            Err(ENOSYS.into())
        }
    }

    fn open(&self, _req: &Request, path: &Path, fi: *const fuse_file_info) -> FuseResult<RawFd> {
        let flags = (unsafe { *fi }).flags;
        info!(target: OP_TAG, "Opening {:?} with flags {}", path, flags);

        let conn_lock = self.conn_pool.get_conn();
        let conn = conn_lock.lock();
        let real_conn = (*conn).borrow_mut();

        if let Some(file_path) = self.resolve_to_alias_file(&real_conn, path)? {
            let mut opts = OpenOptions::new();
            let handle = open_opts_from_mode(&mut opts, flags).open(&file_path)?;
            Ok(handle.into_raw_fd())
        } else {
            Err(ENOENT.into())
        }
    }

    fn read(
        &self,
        _req: &Request,
        _path: &Path,
        buf: &mut [u8],
        offset: off_t,
        fi: *const fuse_file_info,
    ) -> FuseResult<usize> {
        let handle = (unsafe { *fi }).fh as i32;
        info!(
            target: OP_TAG,
            "Calling read on {} for {} bytes, offset {}",
            handle,
            buf.len(),
            offset
        );

        let read = unsafe {
            libc::pread(
                handle,
                buf.as_mut_ptr() as *mut ::std::os::raw::c_void,
                buf.len(),
                offset,
            )
        };

        if read == -1 {
            Err(std::io::Error::last_os_error().into())
        } else {
            Ok(read as usize)
        }
    }

    fn write(
        &self,
        _req: &Request,
        path: &Path,
        data: &[u8],
        offset: off_t,
        _fi: *const fuse_file_info,
    ) -> FuseResult<usize> {
        // we're only allowing writing to alias entries, which is why we don't use `self.resolve_mf_path` here
        match self.op_cache.check_alias_entry(path) {
            // if it's a known alias entry, use alias.write, because it will do validaton on the bytes being
            // written
            Some(alias_rc) => {
                let mut alias = alias_rc.lock();
                match alias.write(data, offset.try_into().unwrap()) {
                    // if we can't write to it, it's bad, so clear it out
                    Err(e) => {
                        drop(alias);
                        self.op_cache.clear_alias(path);
                        self.notifier
                            .lock()
                            .bad_copy()
                            .map_err(SupertagShimError::from)?;
                        Err(e.into())
                    }
                    _ => Ok(data.len()),
                }
            }
            None => Err(EPERM.into()),
        }
    }

    fn flush(&self, _req: &Request, path: &Path, fi: *const fuse_file_info) -> FuseResult<()> {
        let handle = (unsafe { *fi }).fh;
        info!(target: OP_TAG, "Flushing {:?} at fd {}", path, handle);
        #[cfg(target_os = "macos")]
        {
            self.process_alias(path)
        }
        #[cfg(target_os = "linux")]
        {
            Ok(())
        }
    }

    fn truncate(&self, _req: &Request, path: &Path, offset: off_t) -> FuseResult<()> {
        info!(target: OP_TAG, "Truncating {:?}, offset: {}", path, offset);

        let conn_lock = self.conn_pool.get_conn();
        let conn = conn_lock.lock();
        let real_conn = (*conn).borrow_mut();

        if let Some(file_path) = self.resolve_to_alias_file(&real_conn, path)? {
            super::util::truncate(&file_path, offset).map_err(FuseErrno::from)?;

            if let Some(bmark_rc) = self.op_cache.check_alias_entry(path) {
                debug!(target: OP_TAG, "Resetting alias placeholder to 0 written");
                let mut guard = bmark_rc.lock();
                guard.written = 0;
            }
            Ok(())
        } else {
            Err(ENOENT.into())
        }
    }

    /// Important: do not do an actual close on the fd here. That is not our job, it's the kernel's job. We're just
    /// being notified that all handles to a fd have been closed.
    fn release(&self, _req: &Request, _path: &Path, _fi: *const fuse_file_info) -> FuseResult<()> {
        #[cfg(target_os = "macos")]
        {
            let handle = (unsafe { *_fi }).fh;
            info!(
                target: OP_TAG,
                "Releasing to {} at fd {}",
                _path.display(),
                handle
            );
            self.process_alias(_path)
        }

        #[cfg(not(target_os = "macos"))]
        {
            Err(ENOSYS.into())
        }
    }

    fn rmdir(&self, _req: &Request, path: &Path) -> FuseResult<()> {
        info!(target: OP_TAG, "Removing tag dir {}", path.display());

        let tags = TagCollection::new(&self.settings, path);
        let pt = tags.primary_type()?;

        if let TagType::FileDir = pt {
            Ok(())
        } else {
            let full_path = self.settings.abs_mountpoint(path);
            self.notifier
                .lock()
                .unlink(&full_path)
                .map_err(SupertagShimError::from)?;
            Err(ENOSYS.into())
        }
    }

    fn unlink(&self, req: &Request, path: &Path) -> FuseResult<()> {
        info!(target: OP_TAG, "Unlinking symlink {}", path.display());

        // if this is a pid that we're already blocking from working, report an error
        if self.op_cache.check_delete_pid(req.pid) {
            Err(ENOSYS.into())
        }
        // if they're attempting to delete the canary, it means they're doing a recursive delete
        else if path.ends_with(constants::UNLINK_CANARY) {
            self.op_cache.add_deny_delete_pid(req.pid);

            let full_path = self.settings.abs_mountpoint(path);
            self.notifier
                .lock()
                .unlink(&full_path)
                .map_err(SupertagShimError::from)?;
            Err(ENOSYS.into())
        }
        // otherwise, let's allow the delete
        else {
            let conn_lock = self.conn_pool.get_conn();
            let conn = conn_lock.lock();
            let mut real_conn = (*conn).borrow_mut();

            let tx = real_conn
                .transaction_with_behavior(TransactionBehavior::Exclusive)
                .map_err(SupertagShimError::from)?;

            common::fsops::rm(&self.settings, &tx, path)?;

            tx.commit().map_err(SupertagShimError::from)?;

            self.op_cache.clear_alias(path);
            self.flush_paths_tags(path);
            self.flush_readdir_cache(path);
            Ok(())
        }
    }

    fn mkdir(&self, req: &Request, path: &Path, mode: mode_t) -> FuseResult<()> {
        info!(target: OP_TAG, "Making tag dir {}", path.display());

        let conn_lock = self.conn_pool.get_conn();
        let conn = conn_lock.lock();
        let mut real_conn = (*conn).borrow_mut();

        let tx = real_conn
            .transaction_with_behavior(TransactionBehavior::Exclusive)
            .map_err(SupertagShimError::from)?;

        common::fsops::mkdir(
            &self.settings,
            &tx,
            path,
            req.uid,
            req.gid,
            &Permissions::from(mode),
        )
        .map_err(SupertagShimError::from)?;
        tx.commit().map_err(SupertagShimError::from)?;
        Ok(())
    }

    fn rename(&self, req: &Request, src: &Path, dst: &Path) -> FuseResult<()> {
        info!(
            target: OP_TAG,
            "Renaming {} to {}",
            src.display(),
            dst.display()
        );

        let conn_lock = self.conn_pool.get_conn();
        let conn = conn_lock.lock();
        let mut real_conn = (*conn).borrow_mut();

        let tx = real_conn
            .transaction_with_behavior(TransactionBehavior::Exclusive)
            .map_err(SupertagShimError::from)?;

        let dst_name = get_filename(dst)?;
        if common::should_unlink(dst_name) {
            debug!(
                target: OP_TAG,
                "We're renaming to the unlink name, unlinking instead"
            );

            let tags = TagCollection::new(&self.settings, src);
            match tags.primary_type()? {
                TagType::DeviceFileSymlink(_) | TagType::Symlink(_) => {
                    common::fsops::rm(&self.settings, &tx, src)?;
                    self.flush_paths_tags(src);
                }
                TagType::Regular(_) | TagType::Group(_) => {
                    common::fsops::rmdir(&self.settings, &tx, src)?;
                    self.op_cache.add_rename_delete_entry(dst);
                }
                _ => {
                    error!(target: OP_TAG, "Unsupported tagtype for unlink");
                    return Err(EIO.into());
                }
            }
        } else {
            common::fsops::move_or_merge(
                &self.settings,
                &tx,
                src,
                dst,
                req.uid,
                req.gid,
                &req.umask.into(),
                &*(self.notifier.lock()),
            )?;
        }

        tx.commit().map_err(SupertagShimError::from)?;

        // now that our tagdir has been renamed, we need to flush it from our readdir cache, so
        // that it doesn't get reported as existing
        self.flush_readdir_cache(src);

        // this seems counter-intuitive, but it covers the situation where we move in to a collision, meaning two files
        // with the same tag. in this case, the destination "goes away" meaning it switches over to the fully-qualified
        // naming. so we need to flush the readdir cache on it, so the old unqualified name doesn't appear.
        self.flush_readdir_cache(dst);

        self.flush_paths_tags(dst);

        Ok(())
    }

    fn statfs(&self, _req: &Request, _path: &Path) -> FuseResult<statvfs> {
        let mut res = new_statvfs();
        res.f_bsize = 4096;
        res.f_frsize = 4096;

        // 100 GB worth of blocks
        #[cfg(target_os = "macos")]
        {
            res.f_blocks = ((100 * 1024u64.pow(3u32)) / res.f_bsize) as u32;
        }

        #[cfg(not(target_os = "macos"))]
        {
            res.f_blocks = (100 * 1024u64.pow(3u32)) / res.f_bsize;
        }

        // FIXME make all these represent the filesystem that the sqlite db is hosted on
        res.f_bfree = res.f_blocks;
        res.f_bavail = res.f_blocks;
        res.f_files = 100; // FIXME
        res.f_ffree = 10_000;
        res.f_favail = res.f_ffree;
        Ok(res)
    }

    fn set_handle(&mut self, handle: Arc<FuseHandle>) {
        debug!(target: OP_TAG, "Setting fuse handle");
        self.handle = Some(handle);
    }

    #[cfg(target_os = "macos")]
    fn setxattr(
        &self,
        req: &Request,
        path: &Path,
        name: &str,
        value: &[u8],
        position: u32,
        flags: i32,
    ) -> FuseResult<()> {
        self.setxattr_impl(req, path, name, value, position, flags)
    }

    #[cfg(target_os = "macos")]
    fn getxattr(
        &self,
        req: &Request,
        path: &Path,
        name: &str,
        position: u32,
    ) -> FuseResult<Vec<u8>> {
        self.getxattr_impl(req, path, name, position)
    }

    #[cfg(target_os = "macos")]
    fn listxattr(&self, req: &Request, path: &Path, options: i32) -> FuseResult<Vec<String>> {
        self.listxattr_impl(req, path, options)
    }

    #[cfg(target_os = "macos")]
    fn removexattr(&self, req: &Request, path: &Path, name: &str, options: i32) -> FuseResult<()> {
        self.removexattr_impl(req, path, name, options)
    }
}
