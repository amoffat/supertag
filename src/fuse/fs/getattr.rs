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

use super::super::err::SupertagShimError;
use super::super::util;
use super::TagFilesystem;
use super::OP_TAG;
use crate::common::constants;
use crate::common::types::file_perms::UMask;
use crate::common::types::{TagCollectible, TagCollection, TagType, UtcDt};
use crate::fuse::opcache;
use crate::sql::types::TaggedFile;
use crate::{common, sql};
use fuse_sys::stat;
use fuse_sys::{FuseResult, Request};
use log::{debug, info, warn};
use nix::errno::Errno::ENOENT;
use std::path::Path;

impl<N> TagFilesystem<N>
where
    N: common::notify::Notifier,
{
    fn getattr_supertag_root_conf(
        &self,
        req: &Request,
        path: &Path,
        mtime: &UtcDt,
    ) -> FuseResult<stat> {
        if path == Path::new(constants::DB_FILE_PATH) {
            Ok(util::db_file(req.uid, req.gid, mtime))
        } else {
            Err(ENOENT.into())
        }
    }

    pub fn getattr_impl(&self, req: &Request, path: &Path) -> FuseResult<stat> {
        info!(target: OP_TAG, "Stating {:?} from PID {}", path, req.pid);

        let root_mtime = self.get_root_mtime(None)?;

        #[cfg(target_os = "macos")]
        {
            if path == Path::new(constants::FSEVENTS_PATH) {
                debug!(target: OP_TAG, "Mac is looking for fseventsd dir");
                return Ok(util::new_dir(
                    &root_mtime,
                    req.uid,
                    req.gid,
                    &UMask::from(req.umask).dir_perms(),
                    0,
                ));
            }

            if path == Path::new(constants::FSEVENTS_NO_LOG_PATH) {
                debug!(target: OP_TAG, "Mac is looking for fseventsd no_log file");
                return Ok(util::new_regfile(
                    &root_mtime,
                    req.uid,
                    req.gid,
                    &UMask::from(req.umask).file_perms(),
                    0,
                ));
            }

            if path == Path::new(constants::NO_INDEX_PATH) {
                debug!(
                    target: OP_TAG,
                    "Mac is looking for metadata_never_index file"
                );
                return Ok(util::new_regfile(
                    &root_mtime,
                    req.uid,
                    req.gid,
                    &UMask::from(req.umask).file_perms(),
                    0,
                ));
            }
        }

        if path.ends_with(constants::TRACKER_IGNORE) {
            return Ok(util::new_regfile(
                &root_mtime,
                req.uid,
                req.gid,
                &UMask::from(req.umask).file_perms(),
                0,
            ));
        }

        if path.ends_with(constants::UNLINK_CANARY) {
            return Ok(util::new_regfile(
                &root_mtime,
                req.uid,
                req.gid,
                &UMask::from(req.umask).file_perms(),
                0,
            ));
        }

        // if this is our root supertag config directory, report it as present
        if path == Path::new(constants::STAG_ROOT_CONF_PATH) {
            return Ok(util::new_dir(
                &root_mtime,
                req.uid,
                req.gid,
                &UMask::from(req.umask).dir_perms(),
                0,
            ));
        }
        // if it's a file in our supertag config directory, delegate to our helper method
        else if path.starts_with(Path::new(constants::STAG_ROOT_CONF_PATH)) {
            return self.getattr_supertag_root_conf(req, path, &root_mtime);
        }

        #[cfg(target_os = "macos")]
        {
            if let Some(alias_rc) = self.op_cache.check_alias_entry(path) {
                debug!(target: OP_TAG, "Found {:?} in the MacOS Alias cache", path);
                let bm = alias_rc.lock();

                //let md = (*bm).managed_file.metadata()?;

                return Ok(util::new_alias(
                    &(*bm).btime,
                    &(*bm).mtime,
                    &(*bm).mtime,
                    (*bm).written,
                    (*bm).uid,
                    (*bm).gid,
                    (*bm).mode,
                ));
            }
        }

        // if it's just our regular root directory, short-circuit and say it exists
        if path == Path::new(&std::path::MAIN_SEPARATOR.to_string()) {
            debug!(target: OP_TAG, "It's a root directory, saying it exists");

            let conn_lock = self.conn_pool.get_conn();
            let conn = conn_lock.lock();

            let mtime =
                sql::get_root_mtime(&(*conn).borrow_mut()).map_err(SupertagShimError::from)?;

            let conf = self.settings.get_config();

            return Ok(util::new_dir(
                &mtime,
                conf.mount.uid,
                conf.mount.gid,
                &conf.mount.permissions,
                0,
            ));
        }

        let tags = TagCollection::new(&self.settings, path);
        let pt = tags.primary_type().map_err(SupertagShimError::from)?;

        {
            let conn_lock = self.conn_pool.get_conn();
            let conn = conn_lock.lock();
            let real_conn = &(*conn).borrow_mut();

            // we need to validate tag group pairs, which ensure that if a tag group is followed by a regular
            // tag, the tag actually is part of the tag group
            for (tg, tg_tag) in tags.iter().taggroup_pairs() {
                if !sql::tag_is_in_group(real_conn, tg, tg_tag).map_err(SupertagShimError::from)? {
                    debug!(
                        target: OP_TAG,
                        "Tag {} wasn't found under tag group {}", tg_tag, tg
                    );
                    return Err(ENOENT.into());
                }
            }
        }

        match pt {
            TagType::Symlink(_sfile) if tags.unlinking => {
                debug!(
                    target: OP_TAG,
                    "{:?} is a regular symlink needs its cache flushed, flushing", path
                );
                if let Some(stripped) = self.strip_sync_char(path) {
                    self.flush_readdir_cache(&stripped);
                }
                Err(ENOENT.into())
            }

            TagType::DeviceFileSymlink(_device_file) if tags.unlinking => {
                debug!(
                    target: OP_TAG,
                    "{:?} is a devicefile symlink needs its cache flushed, flushing", path
                );
                if let Some(stripped) = self.strip_sync_char(path) {
                    self.flush_readdir_cache(&stripped);
                }
                Err(ENOENT.into())
            }

            TagType::Regular(_) if tags.unlinking => {
                debug!(
                    target: OP_TAG,
                    "{:?} is a tag that needs its cache flushed, flushing", path
                );
                if let Some(stripped) = self.strip_sync_char(path) {
                    self.flush_readdir_cache(&stripped);
                }
                Err(ENOENT.into())
            }

            TagType::DeviceFileSymlink(device_file) => {
                if let Some(opcache::ReaddirCacheEntry::File(cached_file)) =
                    self.op_cache.check_readdir_entry(path)
                {
                    return Ok(util::new_statfile(cached_file));
                }

                debug!(
                    target: OP_TAG,
                    "{:?} looks like a symlink, making sure it exists in the path intersection",
                    path
                );

                let conn_lock = self.conn_pool.get_conn();
                let conn = conn_lock.lock();

                if let Some(match_file) = sql::contains_file(
                    &(*conn).borrow_mut(),
                    tags.all_but_last().as_slice(),
                    |tf| device_file.matches(tf),
                )
                .map_err(SupertagShimError::from)?
                {
                    debug!(target: OP_TAG, "{:?} exists at the intersection", path);
                    return Ok(util::new_statfile(match_file));
                }

                debug!(target: OP_TAG, "{:?} doesn't exist", path);
                Err(ENOENT.into())
            }

            TagType::Symlink(sfile) => {
                if let Some(opcache::ReaddirCacheEntry::File(cached_file)) =
                    self.op_cache.check_readdir_entry(path)
                {
                    return Ok(util::new_statfile(cached_file));
                }

                debug!(
                    target: OP_TAG,
                    "{:?} Is a regular, non device-file symlink", path
                );

                let conn_lock = self.conn_pool.get_conn();
                let conn = conn_lock.lock();

                // let's get all of our tagged files
                let ifiles =
                    sql::files_tagged_with(&(*conn).borrow_mut(), tags.all_but_last().as_slice())
                        .map_err(SupertagShimError::from)?;

                // then lets filter out the ones that don't match by name
                let matches: Vec<TaggedFile> = ifiles
                    .into_iter()
                    .filter(|tf| sfile == &tf.primary_tag)
                    .collect();

                // and only if we have a single match do we say that everything is fine.  if we have multiple matches,
                // that indicates that we attempted to stat a file that did not have an device/inode in the name, and
                // there's no way to distinguish that file from other files with the same tags and same name
                if matches.len() == 1 {
                    self.op_cache.add_readdir_entry(
                        &path,
                        opcache::ReaddirCacheEntry::File(matches[0].clone()),
                    );

                    debug!(target: OP_TAG, "{:?} exists at the intersection", path);
                    return Ok(util::new_statfile(matches[0].clone()));
                }

                Err(ENOENT.into())
            }

            TagType::Group(_) if tags.unlinking => {
                debug!(
                    target: OP_TAG,
                    "{:?} is a taggroup that needs its cache flushed, flushing", path
                );
                if let Some(stripped) = self.strip_sync_char(path) {
                    self.flush_readdir_cache(&stripped);
                }
                Err(ENOENT.into())
            }

            TagType::Group(tag_group) => {
                // here we're checking if it's an entry already in the readdir cache, which will
                // allow us to quickly say it's present
                // TODO check that this is working
                if let Some(opcache::ReaddirCacheEntry::TagGroup(cached_tg)) =
                    self.op_cache.check_readdir_entry(path)
                {
                    return Ok(util::new_dir(
                        &cached_tg.mtime,
                        cached_tg.uid,
                        cached_tg.gid,
                        &cached_tg.permissions,
                        cached_tg.num_files,
                    ));
                }

                debug!(target: OP_TAG, "Last component {} is a tag group, checking if it exists in the tag intersections", tag_group);

                let conn_lock = self.conn_pool.get_conn();
                let conn = conn_lock.lock();

                // if there's only one part in our path, and it's a tag group, we should just check
                // if it exists in the database
                if tags.len() == 1 {
                    let real_conn = &(*conn).borrow_mut();
                    if let Some(mut tg) = sql::get_tag_group(real_conn, &tag_group)
                        .map_err(SupertagShimError::from)?
                    {
                        let num_files = sql::num_files_for_tag_group(real_conn, &tag_group)
                            .map_err(SupertagShimError::from)?;
                        debug!(
                            target: OP_TAG,
                            "Adjusting tag group num_files to {}", num_files
                        );
                        tg.num_files = num_files as i64;

                        self.op_cache.add_readdir_entry(
                            &path,
                            opcache::ReaddirCacheEntry::TagGroup(tg.clone()),
                        );

                        return Ok(util::new_dir(
                            &tg.mtime,
                            tg.uid,
                            tg.gid,
                            &tg.permissions,
                            tg.num_files,
                        ));
                    }
                }
                // but if there are more parts in the path, we need to check tag intersections
                else {
                    // if there are two consecutive tag groups, it's an error
                    let mut last_was_group = false;
                    for qt in tags.iter() {
                        match qt {
                            TagType::Group(_) => {
                                if last_was_group {
                                    return Err(ENOENT.into());
                                } else {
                                    last_was_group = true;
                                }
                            }
                            _ => last_was_group = false,
                        }
                    }

                    // here we'll get all of the possible tag groups for the tag intersection
                    let tag_groups =
                        sql::tag_group_intersections(&(*conn).borrow_mut(), tags.as_slice())
                            .map_err(SupertagShimError::from)?;

                    for tg in tag_groups {
                        if &tg.name == tag_group {
                            self.op_cache.add_readdir_entry(
                                &path,
                                opcache::ReaddirCacheEntry::TagGroup(tg.clone()),
                            );

                            return Ok(util::new_dir(
                                &tg.mtime,
                                tg.uid,
                                tg.gid,
                                &tg.permissions,
                                tg.num_files,
                            ));
                        }
                    }
                }

                debug!(
                    target: OP_TAG,
                    "Tag group {:?} didn't exist in tag intersection", tag_group
                );
                Err(ENOENT.into())
            }

            // this might be a filedir.  if it is, we need to make sure it's a filedir that
            // isn't listed directly under the root directory, in order to say that it exists.
            // for example, /filedir shouldn't exist, but /tag/filedir should
            TagType::FileDir => {
                if let Some(opcache::ReaddirCacheEntry::Tag(cached_tag)) =
                    self.op_cache.check_readdir_entry(path)
                {
                    return Ok(util::new_dir(
                        &cached_tag.mtime,
                        cached_tag.uid,
                        cached_tag.gid,
                        &cached_tag.permissions,
                        cached_tag.num_files,
                    ));
                }

                let parent = tags.primary_parent();

                match parent {
                    Some(TagType::Regular(parent_tag)) | Some(TagType::Negation(parent_tag)) => {
                        let conn_lock = self.conn_pool.get_conn();
                        let conn = conn_lock.lock();

                        let maybe_tag = sql::get_tag(&(*conn).borrow_mut(), parent_tag)
                            .map_err(SupertagShimError::from)?;

                        // we can't use the parent_tag's num_files because it does not accurately reflect the num_files
                        // in the intersection. so we do this.
                        let num_files = sql::get_num_files(&(*conn).borrow_mut(), tags.as_slice())
                            .map_err(SupertagShimError::from)?;

                        return match maybe_tag {
                            Some(tag) => {
                                // this is the best place to add the cache entry for the filedir.  the
                                // alternative place would be in readdir, but even if we did that, we would
                                // still need to do a bunch of logic above in case of a cache miss.  so
                                // the logic would still have to be here, and also duplicated in readdir.
                                // let's just do it here instead.
                                let mut cloned_tag = tag.clone();
                                cloned_tag.num_files = num_files as i64;
                                let entry = opcache::ReaddirCacheEntry::Tag(cloned_tag);
                                self.op_cache.add_readdir_entry(&path, entry);

                                Ok(util::new_dir(
                                    &tag.mtime,
                                    tag.uid,
                                    tag.gid,
                                    &tag.permissions,
                                    num_files as i64,
                                ))
                            }
                            None => {
                                debug!(target: OP_TAG, "Tag {:?} wasn't found", parent_tag);
                                Err(ENOENT.into())
                            }
                        };
                    }
                    _ => Err(ENOENT.into()),
                }
            }

            TagType::Regular(tag) | TagType::Negation(tag) => {
                debug!(target: OP_TAG, "{:?} is a tagdir", path);
                // here we're checking if it's an entry already in the readdir cache, which will
                // allow us to quickly say it's present
                if let Some(opcache::ReaddirCacheEntry::Tag(cached_tag)) =
                    self.op_cache.check_readdir_entry(path)
                {
                    return Ok(util::new_dir(
                        &cached_tag.mtime,
                        cached_tag.uid,
                        cached_tag.gid,
                        &cached_tag.permissions,
                        cached_tag.num_files,
                    ));
                }

                // if this path has recently been created by an external command (like a gui file browser
                // or the ln commandline command), we'll be receiving a stat request for the *non-suffixed*
                // filename (filename without @ inode).  here, we're checking if this is the case, and if it
                // is, we'll consume that cached inode so that external programs play nice with symlinking
                //
                // is this still relevant???
                //     yes.  if a user symlinks or drags and drops to the filedir or the tag dir itself, the OS will
                //     expect the file to live in that exact location, so the symlink cache is still useful.
                if let Some(cached_file) = self.op_cache.consume_symlink(req, path) {
                    debug!(
                        target: OP_TAG,
                        "Found {:?} in the recent symlink cache", path
                    );
                    // it's ok to use now for the timespec if we find the symlink in the opcache, because
                    // it was likely *just* created anyways
                    let now_ts = chrono::Utc::now();
                    return Ok(util::new_link(
                        &now_ts,
                        cached_file.uid,
                        cached_file.gid,
                        &cached_file.permissions,
                        0,
                    ));
                }

                if self.op_cache.consume_rename_delete(path) {
                    debug!(
                        target: OP_TAG,
                        "Found {:?} in the recent rename-delete cache", path
                    );
                    // it's ok to use now for the timespec if we find the symlink in the opcache, because
                    // it was likely *just* created anyways
                    let now_ts = chrono::Utc::now();
                    return Ok(util::new_dir(
                        &now_ts,
                        req.uid,
                        req.gid,
                        &UMask::from(req.umask).dir_perms(),
                        0,
                    ));
                }

                debug!(
                    target: OP_TAG,
                    "It looks like a directory, checking last path component: {:?}",
                    path.components().last()
                );

                let query_tags = self.settings.path_to_tags(path);

                let conn_lock = self.conn_pool.get_conn();
                let conn = conn_lock.lock();

                // if our path has more than one component, essentially what we need to do
                // is prove that the last component has an intersection with all of the
                // previous components.  if one exists, then we can say that the tag
                // directory exists
                if query_tags.len() > 1 {
                    debug!(
                        target: OP_TAG,
                        "It has multiple parts, let's see if the intersection exists"
                    );

                    let qt_slice = query_tags.as_slice();
                    let all_but_last = &qt_slice[0..qt_slice.len() - 1];
                    let itags =
                        sql::intersect_tag(&(*conn).borrow_mut(), all_but_last, true).unwrap();

                    debug!(
                        target: OP_TAG,
                        "Got a tag intersection of {} tags",
                        itags.len()
                    );

                    for itag in itags {
                        if &itag.name == tag {
                            debug!(target: OP_TAG, "It does exist");

                            // we can frequently see a cache miss on the parent directory
                            // to a filedir.  if we're listing a filedir directly, without
                            // ever letting readdir put the tag into the readdir cache,
                            // we'll always have a cache miss.  so let's always make sure
                            // it's in the cache
                            self.op_cache.add_readdir_entry(
                                &path,
                                opcache::ReaddirCacheEntry::Tag(itag.clone()),
                            );

                            return Ok(util::new_dir(
                                &itag.mtime,
                                itag.uid,
                                itag.gid,
                                &itag.permissions,
                                itag.num_files,
                            ));
                        }
                    }

                    // maybe this tag directory is pinned, as in, we've created it from
                    // a file save dialog in order to save a file to it
                    debug!(target: OP_TAG, "Checking if {:?} is pinned", path);
                    if sql::is_pinned(&(*conn).borrow_mut(), query_tags.as_slice())
                        .map_err(SupertagShimError::from)?
                    {
                        debug!(
                            target: OP_TAG,
                            "{:?} is pinned, saying it exists once we find the record", path
                        );

                        let last_tt = query_tags.last().unwrap();
                        match last_tt {
                            TagType::Regular(last_tag) => {
                                debug!(target: OP_TAG, "Last record is a regular tag");
                                let record = sql::get_tag(&(*conn).borrow_mut(), &last_tag)
                                    .map_err(SupertagShimError::from)?;

                                if let Some(tag) = record {
                                    return Ok(util::new_dir(
                                        &tag.mtime,
                                        tag.uid,
                                        tag.gid,
                                        &tag.permissions,
                                        tag.num_files,
                                    ));
                                }
                            }
                            TagType::Group(last_tag) => {
                                debug!(target: OP_TAG, "Last record is a group tag");
                                let record = sql::get_tag_group(&(*conn).borrow_mut(), &last_tag)
                                    .map_err(SupertagShimError::from)?;

                                if let Some(group) = record {
                                    return Ok(util::new_dir(
                                        &group.mtime,
                                        group.uid,
                                        group.gid,
                                        &group.permissions,
                                        0,
                                    ));
                                }
                            }
                            _ => {
                                warn!(target: OP_TAG, "Last tag in pin is {:?}", last_tt);
                            }
                        }
                    }

                    warn!(target: OP_TAG, "Doesn't exist");
                    Err(ENOENT.into())
                } else {
                    debug!(
                        target: OP_TAG,
                        "It's a top-level tag directory, just checking that it exists"
                    );

                    // we want to make sure the tag actually exists.  the alternative is
                    // to say that all tags exist, and this would let you do things like
                    // symlink to long tag paths of non-existant tags, but we're choosing
                    // not to do that.  i can't remember exactly why FIXME

                    if let Some(found_tag) = sql::get_tag(&(*conn).borrow_mut(), &tag)
                        .map_err(SupertagShimError::from)?
                    {
                        debug!(target: OP_TAG, "It does exist");

                        // we can frequently see a cache miss on the parent directory
                        // to a filedir.  if we're listing a filedir directly, without
                        // ever letting readdir put the tag into the readdir cache,
                        // we'll always have a cache miss.  so let's always make sure
                        // it's in the cache
                        self.op_cache.add_readdir_entry(
                            &path,
                            opcache::ReaddirCacheEntry::Tag(found_tag.clone()),
                        );

                        Ok(util::new_dir(
                            &found_tag.mtime,
                            found_tag.uid,
                            found_tag.gid,
                            &found_tag.permissions,
                            found_tag.num_files,
                        ))
                    } else {
                        warn!(target: OP_TAG, "Doesn't exist");
                        Err(ENOENT.into())
                    }
                }
            }
        }
    }
}
