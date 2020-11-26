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

//!# `OpCache`
//!
//! `opcache` represents some kludge code that we need to make `Supertag` behave like a regular
//! filesystem

use crate::common::constants::ALIAS_HEADER;
use crate::common::settings::Settings;
use crate::common::types::file_perms::UMask;
use crate::common::types::{TagCollection, TagType, UtcDt};
use crate::sql;
use fuse_sys::{gid_t, mode_t, pid_t, uid_t, Request};
use log::{debug, info, trace, warn};
use parking_lot::{Mutex, RwLock};
use std::fs::{File, OpenOptions};
use std::hash::Hash;
use std::io::{Seek, SeekFrom, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use ttl_cache::TtlCache;

pub const SYMLINK_EXPIRE_MS: u64 = 500;
pub const UNLINK_EXPIRE_MS: u64 = 2000;
pub const ALIAS_EXPIRE_MS: u64 = 500;
pub const READDIR_EXPIRE_S: u64 = 1;

#[derive(Hash, Ord, PartialOrd, Eq, PartialEq, Clone)]
struct SymlinkRequest {
    req: Request,
    path: PathBuf,
}

#[derive(Hash, Ord, PartialOrd, Eq, PartialEq, Clone)]
struct ReaddirKey {
    path: PathBuf,
}

#[derive(Hash, Ord, PartialOrd, Eq, PartialEq, Clone)]
struct AliasKey {
    path: PathBuf,
}

#[derive(Clone, Debug)]
pub enum ReaddirCacheEntry {
    File(sql::types::TaggedFile),
    Tag(sql::types::Tag),
    TagGroup(sql::types::TagGroup),
}

#[derive(Hash, Ord, PartialOrd, Eq, PartialEq, Clone)]
struct DeleteKey {
    path: PathBuf,
}

#[derive(Hash, Ord, PartialOrd, Eq, PartialEq, Clone)]
struct UnlinkKey {
    pid: pid_t,
}

#[derive(Debug)]
pub struct Alias {
    // represents the open managed file fd, which is only ever opened once we pass validation
    pub file_handle: File,
    header_ptr: u8,

    // the path that the OS thinks it is writing to, when in reality, its data is being proxied to `managed_file`
    path: PathBuf,

    pub btime: UtcDt,
    pub mtime: UtcDt,
    pub mode: mode_t,
    pub umask: UMask,
    pub uid: uid_t,
    pub gid: gid_t,

    // linked refers to whether or not we've created a symlink from the `managed_file` to `path`, which only ever
    // happens upon release of the fd.
    pub linked: bool,
    valid: Option<bool>,

    // this is the underlying file on the filesystem.  it is only ever created if a alias has passed validation,
    // in which case the buffered data is written out to this file, and all future writes go directly to this file via
    // the `file_handle` attribute
    pub managed_file: PathBuf,

    // represents how many bytes have been written to the alias
    pub written: usize,
}

#[derive(Debug)]
pub struct XAttr {
    pub name: String,
    pub value: Vec<u8>,
    pub position: u32,
    pub flags: i32,
}

impl Alias {
    pub fn new(
        path: PathBuf,
        mode: mode_t,
        umask: UMask,
        uid: uid_t,
        gid: gid_t,
        managed_file: PathBuf,
    ) -> std::io::Result<Self> {
        let parent = managed_file.parent().unwrap();
        if !parent.exists() {
            debug!(
                target: ALIAS_TAG,
                "Ensuring managed file dir {} exists",
                parent.display()
            );
            std::fs::create_dir_all(parent)?;
        }

        // TODO confirm that the ownership is correct even when the mount daemon is a diff user
        let fd = std::fs::OpenOptions::new()
            .write(true)
            .read(true)
            .create(true)
            .truncate(true)
            .mode(mode.into())
            .open(&managed_file)?;

        Ok(Self {
            file_handle: fd,
            header_ptr: 0,
            path,
            btime: chrono::Utc::now(),
            mtime: chrono::Utc::now(),
            mode,
            umask,
            uid,
            gid,
            linked: false,
            valid: None,
            managed_file,
            written: 0,
        })
    }

    pub fn open(&self, mode: i32) -> std::io::Result<File> {
        let mut opts = OpenOptions::new();
        super::util::open_opts_from_mode(&mut opts, mode).open(&self.managed_file)
    }

    pub fn is_valid(&self) -> bool {
        if let Some(true) = self.valid {
            self.written > ALIAS_TAG.len()
        } else {
            false
        }
    }

    pub fn write(&mut self, data: &[u8], offset: usize) -> std::io::Result<()> {
        info!(
            target: ALIAS_TAG,
            "Writing {} bytes to potential alias at offset {}",
            data.len(),
            offset,
        );

        if self.linked {
            warn!(target: ALIAS_TAG, "Alias is already linked, aborting");
            return Err(std::io::ErrorKind::PermissionDenied.into());
        }

        self.mtime = chrono::Utc::now();

        // set the write offset.  it always comes from the beginning
        self.file_handle.seek(SeekFrom::Start(offset as u64))?;
        self.written = offset;
        // if we dip below the alias header, we don't know if we're valid again
        if offset < ALIAS_HEADER.len() {
            self.header_ptr = offset as u8;
            self.valid = None;
        }

        // if we don't have valid alias data so far, return an error
        // if we have valid alias data, write it to the file
        // if we're not sure if it's valid, try to validate it and either return an error or write it to the file
        match self.valid {
            Some(false) => return Err(std::io::ErrorKind::PermissionDenied.into()),
            Some(true) => {}
            None => {
                if self.written < ALIAS_HEADER.len() {
                    for &ch in data {
                        // we've surpassed our alias header during this call of write
                        if self.header_ptr > (ALIAS_HEADER.len() - 1) as u8 {
                            debug!(target: ALIAS_TAG, "Passed alias validation");
                            self.valid = Some(true);
                            break;
                        }
                        // otherwise we need to validate each char as we go
                        else {
                            let bch = ALIAS_HEADER[self.header_ptr as usize];
                            if bch == ch {
                                self.header_ptr += 1;
                            } else {
                                warn!(target: ALIAS_TAG, "Failed alias validation");
                                self.valid = Some(false);
                                return Err(std::io::ErrorKind::PermissionDenied.into());
                            }
                        }
                    }
                }
            }
        }

        self.file_handle.write_all(data)?;
        self.written += data.len();

        Ok(())
    }
}

pub(super) struct OpCache {
    settings: Arc<Settings>,

    // this cache stores
    symlink_cache: RwLock<TtlCache<SymlinkRequest, sql::types::TaggedFile>>,

    // the readdir cache stores paths as they were retrieved during the readdir (list directory)
    // filesystem operation.  we do this because computing tag intersections can be expensive, and
    // often times, the filesystem will list a directory and then getattr (stat) on every item that
    // was listed.  if we didn't cache the items from readdir, we would be recomputing tag
    // intersections for every file called for stat.  so this cache makes things significantly
    // faster.  however, like any useful cache, it's important to invalidate its entries when
    // they have changed or been removed.  failure to do so will cause gettattr and readlink
    // operations to incorrectly report as existing
    readdir_cache: RwLock<TtlCache<ReaddirKey, ReaddirCacheEntry>>,

    // these buffers are so we can look up a alias by fd and by path, respectively.  the latter occurs when we do
    // a getattr.  the former occurs during creates, writes, and releases.  these two buffers are for Aliases, which
    // only exist on macos.  symlinks work on macos, but you cannot drag and drop a symlink in Finder.  only Aliases
    // can be created via drag and drop.  a Alias is like a symlink, except it doesn't have kernel-level support...
    // it is handled entirely in userspace.  but it does have some advantages, like being able to move the target and
    // the Alias will resolve most of the time.
    //
    // so the basic idea here is when we see a "create" in supertag, we'll create a pseudo file as a alias entry in
    // `alias_buffer`.  the OS will then start writing bytes to that pseudo fd, which we will then aggregate in the
    // corresponding alias entry's `data` attribute.  if we determine that the bytes being written match the header
    // of a Alias file, we'll allow writes to continue successfully.  at the end, when the OS closes the file, we'll
    // write the alias's buffer to a real file on disk somewhere, and tell our database about that real alias
    // file.  so in the future, when we get a list of intersecting files, if we see that one of those intersecting files
    // is a Alias, we'll be prepared to load the Alias data up, so it should behave like any symlink in Finder
    alias_cache: RwLock<TtlCache<AliasKey, Arc<Mutex<Alias>>>>,

    // This is for denying pids the ability to delete a file. When a fs operation sees that a pid tries to delete the
    // unlink canary, we add it to this cache, which will cause future deletes to fail. In this way, we can terminate
    // a recursive delete safely, without actually deleting anything, since the unlink canary is always attempted to
    // be deleted first
    unlink_canary_cache: RwLock<TtlCache<UnlinkKey, ()>>,

    // This is for tags that get deleted. Some file browsers will flip out if you rename a tag to "delete" and then it
    // vanishes, so here we remember the name briefly so that when the file browser stats the "delete" file, it sees it
    rename_delete_cache: RwLock<TtlCache<DeleteKey, ()>>,
}

const OPCACHE_TAG: &str = "opcache";
const ALIAS_TAG: &str = "alias";
const MAX_SYMLINK_ENTRIES: usize = 10_000;
const MAX_READDIR_ENTRIES: usize = 100_000;
const MAX_CREATE_ENTRIES: usize = 10_000;
const MAX_RM_ENTRIES: usize = 100_000;

impl OpCache {
    pub fn new(settings: Arc<Settings>) -> Self {
        Self {
            settings,
            symlink_cache: RwLock::new(TtlCache::new(MAX_SYMLINK_ENTRIES)),
            readdir_cache: RwLock::new(TtlCache::new(MAX_READDIR_ENTRIES)),
            alias_cache: RwLock::new(TtlCache::new(MAX_CREATE_ENTRIES)),
            unlink_canary_cache: RwLock::new(TtlCache::new(MAX_RM_ENTRIES)),
            rename_delete_cache: RwLock::new(TtlCache::new(MAX_RM_ENTRIES)),
        }
    }

    /// Takes a path and turns ensures it has a filedir in it.
    /// This function, and it's sister function below, are necessary because we don't know what kind of path will get
    /// put into the alias cache...if the user drags a file onto a filedir or a tagdir. But we need to be able to clear
    /// both, if the file is unlinked. So these utility functions let us generate both versions and clear them both.
    fn ensure_filedir(&self, path: &Path) -> PathBuf {
        let tags = TagCollection::new(self.settings.as_ref(), path);
        let mut needs_filedir = true;
        for tag in tags.iter() {
            if let TagType::FileDir = tag {
                needs_filedir = false;
                break;
            }
        }
        if !needs_filedir {
            return path.to_owned();
        }

        if let Some(last_part) = path.file_name() {
            if let Some(parent) = path.parent() {
                parent
                    .join(self.settings.get_config().symbols.filedir_str.to_string())
                    .join(last_part)
            } else {
                path.to_owned()
            }
        } else {
            path.to_owned()
        }
    }

    /// Takes a path and ensures it doesn't have a filedir in it
    fn ensure_no_filedir(&self, path: &Path) -> PathBuf {
        let tags = TagCollection::new(self.settings.as_ref(), path);
        let mut needs_filedir_removed = false;
        for tag in tags.iter() {
            if let TagType::FileDir = tag {
                needs_filedir_removed = true;
                break;
            }
        }
        if !needs_filedir_removed {
            return path.to_owned();
        }

        if let Some(last_part) = path.file_name() {
            if let Some(parent) = path.parent() {
                if let Some(gparent) = parent.parent() {
                    gparent.join(last_part)
                } else {
                    path.to_owned()
                }
            } else {
                path.to_owned()
            }
        } else {
            path.to_owned()
        }
    }

    #[cfg(target_os = "macos")]
    pub fn create_alias(
        &self,
        path: &Path,
        mode: mode_t,
        umask: UMask,
        uid: uid_t,
        gid: gid_t,
        managed_file: PathBuf,
    ) -> std::io::Result<Arc<Mutex<Alias>>> {
        info!(
            target: OPCACHE_TAG,
            "Creating alias for path {}",
            path.display()
        );
        let alias = Arc::new(Mutex::new(Alias::new(
            path.to_owned(),
            mode,
            umask,
            uid,
            gid,
            managed_file,
        )?));

        let mut cache_guard = self.alias_cache.write();
        let key1 = AliasKey {
            path: path.to_owned(),
        };
        cache_guard.insert(key1, alias.clone(), Duration::from_millis(ALIAS_EXPIRE_MS));

        Ok(alias)
    }

    pub fn clear_alias(&self, path: &Path) {
        let mut guard = self.alias_cache.write();

        let remove_paths = vec![self.ensure_filedir(path), self.ensure_no_filedir(path)];

        for to_remove in remove_paths {
            info!(
                target: OPCACHE_TAG,
                "Clearing alias at handle {:?}", to_remove
            );

            let key = AliasKey {
                path: to_remove.clone(),
            };
            match (*guard).remove(&key) {
                Some(_bmark) => {
                    debug!(
                        target: OPCACHE_TAG,
                        "Found {:?} in the alias cache", to_remove
                    );
                }
                None => {
                    warn!(
                        target: OPCACHE_TAG,
                        "Didn't find {:?} in the cache", to_remove
                    );
                }
            }
        }
    }

    pub fn check_alias_entry(&self, path: &Path) -> Option<Arc<Mutex<Alias>>> {
        info!(target: OPCACHE_TAG, "Checking alias entry for {:?}", path);
        let guard = self.alias_cache.read();

        let key = AliasKey {
            path: path.to_owned(),
        };
        match (*guard).get(&key) {
            Some(alias_rc) => {
                debug!(
                    target: OPCACHE_TAG,
                    "Cache hit! Found {:?} in the alias cache", path
                );
                Some(alias_rc.clone())
            }
            None => {
                debug!(
                    target: OPCACHE_TAG,
                    "Cache miss! Didn't find {:?} in the alias cache", path
                );
                None
            }
        }
    }

    pub fn add_deny_delete_pid(&self, pid: pid_t) {
        let ttl = Duration::from_secs(UNLINK_EXPIRE_MS);

        let mut guard = self.unlink_canary_cache.write();

        let key = UnlinkKey { pid };
        (*guard).insert(key, (), ttl);
    }

    pub fn check_delete_pid(&self, pid: pid_t) -> bool {
        let guard = self.unlink_canary_cache.write();

        let key = UnlinkKey { pid };
        (*guard).contains_key(&key)
    }

    pub fn add_readdir_entry(&self, path: &Path, entry: ReaddirCacheEntry) {
        let ttl = Duration::from_secs(READDIR_EXPIRE_S);
        info!(
            target: OPCACHE_TAG,
            "Adding entry to the readdir cache {:?} at {} with ttl {:?}",
            entry,
            path.display(),
            ttl
        );

        let mut guard = self.readdir_cache.write();

        let key = ReaddirKey {
            path: path.to_owned(),
        };
        (*guard).insert(key, entry, ttl);
    }

    pub fn check_readdir_entry(&self, path: &Path) -> Option<ReaddirCacheEntry> {
        info!(target: OPCACHE_TAG, "Checking readdir cache for {:?}", path);
        let guard = self.readdir_cache.read();
        let key = ReaddirKey {
            path: path.to_owned(),
        };
        match (*guard).get(&key) {
            Some(value) => {
                debug!(
                    target: OPCACHE_TAG,
                    "Cache hit! Found {:?} in the readdir cache", path
                );
                Some((*value).clone())
            }
            None => {
                debug!(
                    target: OPCACHE_TAG,
                    "Cache miss. Didn't find {:?} in the readdir cache", path
                );
                None
            }
        }
    }

    pub fn clear_readdir_entry(&self, path: &Path) -> Option<ReaddirCacheEntry> {
        info!(
            target: OPCACHE_TAG,
            "Clearing {:?} from readdir cache", path
        );
        let key = ReaddirKey {
            path: path.to_owned(),
        };
        let mut guard = self.readdir_cache.write();
        let maybe_entry = (*guard).remove(&key);
        if maybe_entry.is_some() {
            debug!(target: OPCACHE_TAG, "Found entry in readdir cache");
        } else {
            debug!(target: OPCACHE_TAG, "Didn't find entry in readdir cache");
        }
        maybe_entry
    }

    pub fn add_symlink(&self, req: &Request, path: &Path, tagged_file: sql::types::TaggedFile) {
        info!(
            target: OPCACHE_TAG,
            "inserting {:?} into symlink cache", path
        );
        let mut guard = self.symlink_cache.write();
        let key = SymlinkRequest {
            req: req.clone(),
            path: path.to_owned(),
        };
        (*guard).insert(key, tagged_file, Duration::from_millis(SYMLINK_EXPIRE_MS));
    }

    pub fn consume_symlink(&self, req: &Request, path: &Path) -> Option<sql::types::TaggedFile> {
        info!(
            target: OPCACHE_TAG,
            "Checking to consume symlink cache for {:?}", path
        );

        trace!(
            target: OPCACHE_TAG,
            "Attempting to acquire symlink cache lock",
        );
        let mut guard = self.symlink_cache.write();
        trace!(target: OPCACHE_TAG, "Got symlink cache lock");

        let key = SymlinkRequest {
            req: req.clone(),
            path: path.to_owned(),
        };

        match (*guard).remove(&key) {
            Some(v) => {
                debug!(target: OPCACHE_TAG, "Found {:?} in the cache", path);
                Some(v)
            }
            None => {
                debug!(target: OPCACHE_TAG, "Didn't find {:?} in the cache", path);
                None
            }
        }
    }

    pub fn add_rename_delete_entry(&self, path: &Path) {
        info!(
            target: OPCACHE_TAG,
            "Inserting {:?} into delete cache", path
        );
        let mut guard = self.rename_delete_cache.write();
        let key = DeleteKey {
            path: path.to_owned(),
        };
        (*guard).insert(key, (), Duration::from_millis(SYMLINK_EXPIRE_MS));
    }

    pub fn consume_rename_delete(&self, path: &Path) -> bool {
        info!(
            target: OPCACHE_TAG,
            "Checking to consume delete cache for {:?}", path
        );

        trace!(
            target: OPCACHE_TAG,
            "Attempting to acquire delete cache lock",
        );
        let mut guard = self.rename_delete_cache.write();
        trace!(target: OPCACHE_TAG, "Got delete cache lock");

        let key = DeleteKey {
            path: path.to_owned(),
        };

        match (*guard).remove(&key) {
            Some(_) => {
                debug!(target: OPCACHE_TAG, "Found {:?} in the cache", path);
                true
            }
            None => {
                debug!(target: OPCACHE_TAG, "Didn't find {:?} in the cache", path);
                false
            }
        }
    }
}
