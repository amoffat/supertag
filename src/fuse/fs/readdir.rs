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

use super::TagFilesystem;
use super::OP_TAG;
use crate::common::constants;
use crate::common::err::STagResult;
use crate::common::types::{TagCollectible, TagCollection, TagType, UtcDt};
use crate::fuse::err::SupertagShimError;
use crate::fuse::opcache;
use crate::sql::types::{Tag, TagOrTagGroup};
use crate::{common, sql};
use fuse_sys::err::FuseErrno;
use fuse_sys::{FileEntry, FuseResult, Request};
use log::{debug, error, info, trace};
use nix::errno::Errno::ENOENT;
use rusqlite::Connection;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;

impl<N> TagFilesystem<N>
where
    N: common::notify::Notifier,
{
    // FIXME see https://users.rust-lang.org/t/internal-visibility-for-trait-methods/15596/2 for a better way
    pub fn readdir_impl(
        &self,
        _req: &Request,
        path: &Path,
    ) -> FuseResult<Box<dyn Iterator<Item = FileEntry>>> {
        info!(target: OP_TAG, "Listing directory {:?}", path);

        let conn_lock = self.conn_pool.get_conn();
        let conn = conn_lock.lock();
        let real_conn = &(*conn).borrow_mut();
        let root_mtime = self.get_root_mtime(Some(&real_conn))?;

        let query_tags = TagCollection::new(&self.settings, path);

        match query_tags.len() {
            // just the root dir?  display all the tags
            0 => {
                debug!(
                    target: OP_TAG,
                    "It's a root directory, so listing all tags and tag groups"
                );
                let tags = sql::get_all_tags(real_conn).map_err(SupertagShimError::from)?;
                let tag_groups =
                    sql::get_all_tag_groups(real_conn).map_err(SupertagShimError::from)?;
                debug!(
                    target: OP_TAG,
                    "Got {} tags and {} tag groups",
                    tags.len(),
                    tag_groups.len()
                );

                // this will serve to ignore a tagdir if we find that it has a tag group that would be displayed
                // here
                let has_taggroup: Arc<RefCell<HashSet<i64>>> =
                    Arc::new(RefCell::new(HashSet::new()));

                // FIXME generalize this logic
                // fill has_taggroup with all of the tagdirs that we should skip listing
                {
                    let mut has_tg_mut = has_taggroup.borrow_mut();
                    for tg in tag_groups.iter() {
                        has_tg_mut.extend(tg.tag_ids.iter());
                    }
                }
                debug!(
                    target: OP_TAG,
                    "Excluding tagdirs {:?} from listing because they have tag groups",
                    has_taggroup.as_ref().borrow()
                );

                let has_taggroup_closure1 = has_taggroup.clone();
                let closure_settings = self.settings.clone();
                let extra = self.extra_root_entries(&root_mtime);

                let entry_iter = tags
                    .into_iter()
                    .filter_map(move |tag| {
                        if has_taggroup_closure1.as_ref().borrow().contains(&tag.id) {
                            None
                        } else {
                            Some(tag.into())
                        }
                    })
                    .chain(
                        tag_groups
                            .into_iter()
                            .map(move |tg| tg.to_fileentry(&closure_settings)),
                    )
                    .chain(extra);

                Ok(Box::new(entry_iter))
            }
            // we're in a subdirectory, find the intersecting tags and associated files
            _ => {
                debug!(
                    target: OP_TAG,
                    "It's a sub directory, doing tag intersection"
                );

                if path == Path::new(constants::STAG_ROOT_CONF_PATH) {
                    debug!(target: OP_TAG, "readdir on supertag conf path");
                    let conf_iter = self.readdir_supertag_root_conf(root_mtime).into_iter();
                    return Ok(Box::new(conf_iter));
                } else if path
                    == Path::new(&format!(
                        "/{}",
                        self.settings.get_config().symbols.filedir_str
                    ))
                {
                    debug!(target: OP_TAG, "readdir on root filedir with all tags");
                    return self
                        .readdir_root_filedir(&real_conn)
                        .map_err(FuseErrno::from);
                }

                // we need to validate tag group pairs, which ensure that if a tag group is followed by a regular
                // tag, the tag actually is part of the tag group
                for (tg, tg_tag) in query_tags.iter().taggroup_pairs() {
                    if !sql::tag_is_in_group(real_conn, tg, tg_tag)
                        .map_err(SupertagShimError::from)?
                    {
                        error!(
                            target: OP_TAG,
                            "Tag {} wasn't found under tag group {}", tg_tag, tg
                        );
                        return Err(ENOENT.into());
                    }
                }

                let primary_type = query_tags.primary_type()?;

                match primary_type {
                    // are we in the directory designated for file intersections?  list the intersecting
                    // files
                    TagType::FileDir => {
                        let extra = self.extra_filedir_entries(&root_mtime);

                        let intersect_files =
                            sql::files_tagged_with(real_conn, query_tags.as_slice())
                                .map_err(SupertagShimError::from)?;

                        // we need to compute duplicate names, so first we'll build up a hashmap of names and their
                        // count in the result set.  later we'll use this map to determine if we have a duplicate and
                        // need to render the name with inodify
                        let mut name_count = HashMap::new();
                        for ifile in intersect_files.iter() {
                            *name_count.entry(ifile.primary_tag.to_string()).or_insert(0) += 1;
                        }

                        let opcache = self.op_cache.clone();
                        let path = path.to_owned();

                        let settings_closure = self.settings.clone();
                        let intersect_iter = intersect_files.into_iter().map(move |file| {
                            // here we're deciding how we want to render the filename.  if there's duplicates for that
                            // name, we need to fully qualify the name with inodify.  otherwise, we can just use the
                            // name as-is
                            let ifilename = {
                                if name_count[&file.primary_tag] > 1 {
                                    settings_closure.inodify_filename(
                                        &file.primary_tag,
                                        file.device,
                                        file.inode,
                                    )
                                } else {
                                    file.primary_tag.to_string()
                                }
                            };
                            let full_path = path.join(&ifilename);
                            let cache_entry = opcache::ReaddirCacheEntry::File(file.clone());
                            opcache.add_readdir_entry(&full_path, cache_entry);
                            FileEntry {
                                name: ifilename,
                                mtime: file.mtime,
                            }
                        });

                        Ok(Box::new(extra.into_iter().chain(intersect_iter)))
                    }
                    // otherwise we're only supposed to list our intersecting tagdirs and tag groups
                    _ => {
                        // get all of our tags that intersect with `query_tags`
                        let intersect_tags =
                            sql::intersect_tag(real_conn, query_tags.as_slice(), true)
                                .map_err(SupertagShimError::from)?;

                        // for every tag in our intersection, find all of the tag groups that they should be grouped into
                        let all_tag_ids =
                            intersect_tags.iter().map(|tag| tag.id).collect::<Vec<_>>();
                        let tag_groups =
                            sql::tag_groups_for_tags(real_conn, all_tag_ids.as_slice())
                                .map_err(SupertagShimError::from)?;

                        // this will serve to ignore a tagdir if we find that it has a tag group that would be displayed
                        // here instead
                        let has_taggroup: Arc<RefCell<HashSet<i64>>> =
                            Arc::new(RefCell::new(HashSet::new()));
                        let tag = query_tags.last().unwrap();

                        // if we're currently listing a tag group dir, do not collapse any `intersect_tags` into
                        // further tag groups.
                        let mut in_a_taggroup = false;
                        if let TagType::Group(_tag_group) = primary_type {
                            debug!(target: OP_TAG, "Skipping tag group exclusions",);
                            in_a_taggroup = true;
                        } else {
                            // fill has_taggroup with all of the tagdirs that we should skip listing.  notice that we're only
                            // doing this if the discovered tag group doesn't match our parent tag group (if exists).
                            {
                                let mut has_tg_mut = has_taggroup.borrow_mut();
                                for tg in tag_groups.iter() {
                                    if let TagType::Group(parent_group) = tag {
                                        if &tg.name != parent_group {
                                            has_tg_mut.extend(tg.tag_ids.iter());
                                        }
                                    } else {
                                        has_tg_mut.extend(tg.tag_ids.iter());
                                    }
                                }
                            }
                            debug!(
                                target: OP_TAG,
                                "Excluding tagdirs {:?} from listing because they have tag groups",
                                has_taggroup.borrow()
                            );
                        }

                        // this iter will either render out tag groups as file entries, or not, based on if we're
                        // already in a tag group, so we don't end up with things like /+a_tags/+a_tags
                        let settings_c1 = self.settings.clone();
                        let path2 = path.to_owned();
                        let op_cache2 = self.op_cache.clone();
                        let tag_groups_iter = tag_groups
                            .into_iter()
                            // if we're in a tag group, we shouldn't render any tag groups
                            .filter(move |tg| {
                                op_cache2.add_readdir_entry(
                                    &path2.join(&tg.name),
                                    opcache::ReaddirCacheEntry::TagGroup(tg.to_owned()),
                                );
                                !in_a_taggroup
                            })
                            .map(move |tg| tg.to_fileentry(&settings_c1))
                            .inspect(|fe| {
                                trace!(target: OP_TAG, "Yielding {:?} from tag groups", fe)
                            });

                        // this will be used to prune out tagdirs from our pinned results.  basically, we'll populate it
                        // from our tag intersection results, and then throw out a result from the pinned iter, if it's in
                        // this set
                        let seen_tagdirs: Arc<RefCell<HashSet<i64>>> =
                            Arc::new(RefCell::new(HashSet::new()));

                        let opcache1 = self.op_cache.clone();
                        let path1 = path.to_owned();
                        let seen_tagdirs1 = seen_tagdirs.clone();
                        let has_taggroup1 = has_taggroup.clone();
                        // transform our tags into FileEntries and make an iterator out of it
                        let tag_intersect_iter = intersect_tags
                            .into_iter()
                            // we'll skip the tag if it appears in a tag group
                            .filter_map(move |tag| {
                                if has_taggroup1.borrow().contains(&tag.id) {
                                    None
                                } else {
                                    seen_tagdirs1.borrow_mut().insert(tag.id);
                                    let cache_entry = opcache::ReaddirCacheEntry::Tag(tag.clone());
                                    opcache1.add_readdir_entry(&path1.join(&tag.name), cache_entry);
                                    Some(tag.into())
                                }
                            })
                            .inspect(|fe| {
                                trace!(target: OP_TAG, "Yielding {:?} from tag intersections", fe)
                            });

                        debug!(
                            target: OP_TAG,
                            "Getting pinned subdirectories for {:?}", query_tags
                        );
                        // now we need to append our pinned subdirectories
                        let pinned_subdirs = sql::pinned_subdirs(real_conn, query_tags.as_slice())
                            .map_err(SupertagShimError::from)?;
                        debug!(target: OP_TAG, "Got pinned subdirs {:?}", pinned_subdirs);

                        let opcache2 = self.op_cache.clone();
                        let path2 = path.to_owned();
                        let seen_tagdirs2 = seen_tagdirs.clone();
                        let has_taggroup2 = has_taggroup.clone();
                        let settings_c2 = self.settings.clone();
                        // same as before, map them to FileEntries and make an iterator out of it
                        let pin_iter = pinned_subdirs
                            .into_iter()
                            .filter_map(move |tag_or_group| match tag_or_group {
                                TagOrTagGroup::Tag(tag) => {
                                    if seen_tagdirs2.borrow().contains(&tag.id)
                                        || has_taggroup2.borrow().contains(&tag.id)
                                    {
                                        None
                                    } else {
                                        seen_tagdirs2.borrow_mut().insert(tag.id);
                                        let cache_entry =
                                            opcache::ReaddirCacheEntry::Tag(tag.clone());
                                        opcache2
                                            .add_readdir_entry(&path2.join(&tag.name), cache_entry);
                                        Some(FileEntry::from(tag))
                                    }
                                }
                                TagOrTagGroup::Group(group) => {
                                    Some(group.to_fileentry(&settings_c2))
                                }
                            })
                            .inspect(|fe| trace!(target: OP_TAG, "Yielding {:?} from pins", fe));

                        let final_iter = tag_groups_iter.chain(tag_intersect_iter).chain(pin_iter);

                        Ok(Box::new(final_iter))
                    }
                }
            }
        }
    }

    pub fn readdir_common_impl(
        &self,
        _req: &Request,
        path: &Path,
    ) -> FuseResult<Box<dyn Iterator<Item = FileEntry>>> {
        let now = self.get_root_mtime(None)?;
        let mut common = vec![];
        common.push(FileEntry {
            name: ".".into(),
            mtime: now,
        });
        common.push(FileEntry {
            name: "..".into(),
            mtime: now,
        });

        let tags = TagCollection::new(&self.settings, path);
        let is_root = tags.len() == 0;
        if !is_root {
            let pt = tags.primary_type()?;
            let is_filedir = pt == &TagType::FileDir;
            let is_tag_group = match pt {
                TagType::Group(_) => true,
                _ => false,
            };

            if !is_filedir && !is_tag_group {
                let conn_lock = self.conn_pool.get_conn();
                let conn = conn_lock.lock();
                let real_conn = &(*conn).borrow_mut();

                let intersect_files = sql::files_tagged_with(real_conn, tags.as_slice())
                    .map_err(SupertagShimError::from)?;

                if !intersect_files.is_empty() {
                    common.push(FileEntry {
                        name: self.settings.get_config().symbols.filedir_str.clone(),
                        mtime: now,
                    });
                }
            }
        }

        Ok(Box::new(common.into_iter()))
    }

    fn readdir_supertag_root_conf(&self, now: UtcDt) -> Vec<FileEntry> {
        let mut entries = vec![];
        entries.push(FileEntry {
            name: common::constants::DB_FILE_NAME.to_string(),
            mtime: now,
        });

        entries
    }

    fn readdir_root_filedir(
        &self,
        conn: &Connection,
    ) -> STagResult<Box<dyn Iterator<Item = FileEntry>>> {
        let tags = sql::get_all_tags(conn)?;

        let tag_iter = tags
            .into_iter()
            .map(|tag: Tag| FileEntry {
                name: tag.name,
                mtime: tag.mtime,
            })
            .inspect(|fe| trace!(target: OP_TAG, "Yielding {:?} from getting all tags", fe));

        Ok(Box::new(tag_iter))
    }

    fn extra_filedir_entries(&self, mtime: &UtcDt) -> Vec<FileEntry> {
        let mut entries = vec![];
        entries.push(FileEntry {
            name: constants::UNLINK_CANARY.to_string(),
            mtime: *mtime,
        });

        entries
    }

    fn extra_root_entries(&self, _mtime: &UtcDt) -> Vec<FileEntry> {
        let entries = vec![];

        // entries.push(FileEntry {
        //     name: constants::STAG_ROOT_CONF_NAME.to_string(),
        //     mtime: *mtime,
        // });

        // TODO make this work eventually
        //
        //        entries.push(FileEntry {
        //            name: self.settings.symbols.filedir_str.to_string(),
        //            mtime: now,
        //        });

        entries
    }
}
