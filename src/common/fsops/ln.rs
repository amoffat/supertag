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

use crate::sql;

use super::super::err::STagResult;
use super::super::settings::Settings;
use super::super::types::file_perms::UMask;
use super::WRAPPER_TAG;
use crate::common::err::STagError;
use crate::common::get_device_inode;
use crate::common::notify::Notifier;
use crate::common::types::{TagCollectible, TagCollection};
use crate::sql::types::TaggedFile;
use fuse_sys::{gid_t, uid_t};
use log::{debug, error, info};

pub fn ln<N: Notifier>(
    settings: &Settings,
    tx: &Transaction,
    src: &Path,
    rel_dst: &Path,
    primary_tag: &str,
    uid: uid_t,
    gid: gid_t,
    umask: &UMask,
    alias_file: Option<&Path>,
    notifier: &N,
) -> STagResult<Vec<TaggedFile>> {
    info!(target: WRAPPER_TAG, "ln {:?} to {:?}", src, rel_dst);

    if let Some(src_col) = settings.collection_from_path(src, false) {
        let cur_col = settings.get_collection();
        if src_col == cur_col {
            error!(
                target: WRAPPER_TAG,
                "src {:?} lives in supertag collection, undefined behavior, aborting link", src
            );
            return Err(STagError::RecursiveLink(src.to_owned()));
        }
    } else {
        debug!(
            target: WRAPPER_TAG,
            "Couldn't get source collection, skipping recursive check"
        );
    }

    if rel_dst == Path::new("") {
        notifier.dragged_to_root()?;
        return Err(STagError::InvalidPath(rel_dst.to_owned()));
    }

    let tag_parts = TagCollection::new(&settings, rel_dst);
    let tags = tag_parts.iter().collect_regular_names();
    let (device, inode) = get_device_inode(src)?;
    let maybe_alias_file = alias_file.map(|a| a.to_str().unwrap());

    let tagged = sql::add_file(
        tx,
        device,
        inode,
        src.to_str()
            .ok_or_else(|| STagError::InvalidPath(src.to_owned()))?,
        primary_tag,
        tags.as_slice(),
        uid,
        gid,
        umask,
        sql::get_now_secs(),
        maybe_alias_file,
    )?;

    Ok(tagged)
}
