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
use std::path::Path;

use rusqlite::Transaction;

use crate::common::err::{STagError, STagResult};
use crate::common::fsops::WRAPPER_TAG;
use crate::common::notify::Notifier;
use crate::common::settings::Settings;
use crate::common::types::file_perms::UMask;
use crate::common::types::{TagCollectible, TagCollection, TagType};
use crate::common::{get_filename, primary_tag};
use crate::sql;
use fuse_sys::{gid_t, uid_t};
use log::{debug, error, info, warn};

/// src and dst must be relative
/// This function does way too much, but it's difficult to avoid.  Since the FUSE handler only sees move/rename calls
/// (as opposed to the CLI, which can have different functions for merge, group, rename, etc), we must put all of the
/// logic for merge, group, rename into here.  If we don't put it here, we have to have it in two separate places:
/// the CLI entrypoint and the FUSE entrypoint.  I prefer a larger function over duplicated logic.
pub fn move_or_merge<P: AsRef<Path>, Q: AsRef<Path>, N: Notifier>(
    settings: &Settings,
    tx: &Transaction,
    src: P,
    dst: Q,
    uid: uid_t,
    gid: gid_t,
    umask: &UMask,
    notifier: &N,
) -> STagResult<()> {
    info!(
        target: WRAPPER_TAG,
        "Move or merge from {} to {}",
        src.as_ref().display(),
        dst.as_ref().display()
    );

    // this ugly helper lambda will map a constraint violation to a STagError::PathExists error
    let map_rename = |e| {
        if let rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error {
                code: rusqlite::ErrorCode::ConstraintViolation,
                extended_code: _,
            },
            _,
        ) = &e
        {
            STagError::PathExists(dst.as_ref().into())
        } else {
            STagError::Other(Box::new(e))
        }
    };

    let src_tags = TagCollection::new(&settings, src.as_ref());
    let dst_tags = TagCollection::new(&settings, dst.as_ref());
    let src_pt = src_tags.primary_type()?;

    match src_pt {
        TagType::DeviceFileSymlink(device_file) => {
            info!(
                target: WRAPPER_TAG,
                "Renaming tagged device-file {} to {}",
                src.as_ref().display(),
                dst.as_ref().display()
            );
            let new_name = primary_tag(dst.as_ref(), settings.get_config().symbols.device_char)?
                .ok_or(STagError::InvalidPath(dst.as_ref().to_owned()))?;
            sql::rename_file(tx, &device_file, &new_name, sql::get_now_secs())
                .map_err(map_rename)?;
        }
        // this arm is very similar to DeviceFileSymlink arm, except we need to first derive a device file by finding
        // the file by the tags first.  it's slower because we don't already immediately have the device/inode combo
        TagType::Symlink(primary_tag) => {
            info!(
                target: WRAPPER_TAG,
                "Renaming tagged file {} to {}",
                src.as_ref().display(),
                dst.as_ref().display()
            );
            let new_name = get_filename(dst.as_ref())?;
            let now = sql::get_now_secs();
            let maybe_tf =
                sql::contains_file(tx, src_tags.as_slice(), |tf| &tf.primary_tag == primary_tag)?;
            if let Some(tf) = maybe_tf {
                sql::rename_file(tx, &tf.into(), &new_name, now).map_err(map_rename)?;
            } else {
                return Err(STagError::InvalidPath(src.as_ref().into()));
            }
        }
        TagType::Regular(src_tag) => {
            if !sql::tag_exists(tx, src_tag)? {
                error!(target: WRAPPER_TAG, "Source tag {} doesn't exist", src_tag);
                return Err(STagError::BadTag(src_tag.clone()));
            }

            info!(
                target: WRAPPER_TAG,
                "Merging or renaming tag directory {} to {}",
                src.as_ref().display(),
                dst.as_ref().display()
            );
            let mut dst_tags = TagCollection::new(&settings, dst.as_ref());
            let src_tags = TagCollection::new(&settings, src.as_ref());

            // this happens often when a file browser is doing a move.  if you try to do mv /t1 to /t2, it will do a
            // mv /t1 to /t2/t1.  we can detect that and be smart with it
            let same_name = src_tags.last().unwrap() == dst_tags.last().unwrap();
            // if we've specified the source directory name in the destination, pop it off, so
            // the merge works correctly
            if same_name {
                dst_tags.pop();
            }

            match dst_tags.primary_type()? {
                TagType::Regular(new_name) => {
                    // if the tag doesn't exist, we're doing a simple move
                    if !sql::tag_exists(tx, &new_name)? {
                        debug!(
                            target: WRAPPER_TAG,
                            "Destination tag {} doesn't exist, we're doing a simple tag rename",
                            new_name
                        );
                        // TODO test that we can't rename to a non-creatable tag
                        sql::rename_tag(tx, &src_tag, &new_name, sql::get_now_secs())?;
                    }
                    // however, if the tag does exist, we need to merge our old tag with it
                    else {
                        debug!(
                            target: WRAPPER_TAG,
                            "Destination tag {} does exist, we're merging from {}",
                            new_name,
                            src_tag
                        );
                        sql::merge_tags(
                            tx,
                            src_tag,
                            src_tags.as_slice(),
                            dst_tags.iter().collect_regular_names().as_slice(),
                            sql::get_now_secs(),
                        )?;
                    }
                }
                TagType::Group(new_name) => {
                    debug!(
                        target: WRAPPER_TAG,
                        "Moving from regular tag into tag group"
                    );

                    if !super::super::creatable_tag_group(settings, &new_name) {
                        error!(
                            target: WRAPPER_TAG,
                            "Tag group {} doesn't have a valid name", new_name
                        );
                        return Err(STagError::BadTagGroup(new_name.to_string()));
                    }

                    let now = sql::get_now_secs();
                    let tagged_files = sql::files_tagged_with(tx, &[src_pt.to_owned()])?;

                    if !sql::tag_group_exists(tx, new_name)? {
                        warn!(
                            target: WRAPPER_TAG,
                            "Tag group {} doesn't exist yet", new_name
                        );

                        return if tagged_files.is_empty() {
                            debug!(
                                target: WRAPPER_TAG,
                                "No tagged files yet, so it s safe to transmute into a tag group"
                            );

                            sql::remove_tag(tx, src_tag, now, true)?;
                            sql::ensure_tag_group(tx, new_name, uid, gid, &umask.dir_perms(), now)?;
                            Ok(())
                        } else {
                            let _ = notifier.tag_to_tg(src_tag);
                            Err(STagError::BadTagGroup(new_name.to_string()))
                        };
                    }

                    sql::add_tag_to_group(
                        tx,
                        &src_tag,
                        &new_name,
                        uid,
                        gid,
                        &umask.dir_perms(),
                        now,
                    )?;

                    if tagged_files.is_empty() {
                        debug!(
                            target: WRAPPER_TAG,
                            "{:?} doesn't have any linked files, pinning it",
                            src.as_ref()
                        );
                        let mut pinnable = dst_tags.iter().collect_pinnable().clone();
                        if !pinnable.is_empty() {
                            pinnable.push(TagType::Regular(src_tag.to_string()));
                            sql::pin_tags(
                                &tx,
                                pinnable.as_slice(),
                                uid,
                                gid,
                                &umask.dir_perms(),
                                now,
                            )?;
                        }
                    }
                }
                _ => return Err(STagError::BadTag("unknown".to_string())),
            }
        }
        TagType::Group(tag_group) => match dst_tags.primary_type()? {
            // we're allowing for a `Group` or `Regular`, in the case that the user typed the prefix character or they
            // left it off.  we know it's a tag group, so we shouldn't care if they leave off the prefix char
            TagType::Group(new_name) | TagType::Regular(new_name) => {
                sql::rename_tag_group(tx, &tag_group, &new_name, sql::get_now_secs())?;
            }
            _ => {
                return Err(STagError::InvalidPath(dst.as_ref().into()));
            }
        },
        _ => {
            error!(
                target: WRAPPER_TAG,
                "{} is not a tagged file or tagdir",
                src.as_ref().display(),
            );
            return Err(STagError::InvalidPath(src.as_ref().into()));
        }
    }

    Ok(())
}
