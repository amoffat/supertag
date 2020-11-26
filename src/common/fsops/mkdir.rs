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

use crate::common::err::STagResult;
use crate::common::fsops::WRAPPER_TAG;
use crate::common::settings::Settings;
use crate::common::types::file_perms::Permissions;
use crate::common::types::{TagCollectible, TagCollection, TagType};
use crate::sql;
use fuse_sys::{gid_t, uid_t};
use log::{debug, info};

pub fn mkdir(
    settings: &Settings,
    tx: &Transaction,
    dir: &Path,
    uid: uid_t,
    gid: gid_t,
    permissions: &Permissions,
) -> STagResult<()> {
    info!(
        target: WRAPPER_TAG,
        "mkdir {:?} uid:{}, gid:{}, perms:{:?}", dir, uid, gid, permissions
    );

    let tags = TagCollection::new(settings, dir);
    let top_level = tags.len() == 1;

    let now = sql::get_now_secs();
    if top_level {
        // can't fail because top_level == true
        let tt = tags.first().unwrap();
        match tt {
            TagType::Group(tag) => {
                debug!(
                    target: WRAPPER_TAG,
                    "{:?} is a top-level tag group, ensuring it exists", dir
                );
                sql::ensure_tag_group(tx, tag, uid, gid, permissions, now)?;
            }
            TagType::Regular(tag) => {
                debug!(
                    target: WRAPPER_TAG,
                    "{:?} is a top-level tag, ensuring it exists", tag
                );
                sql::ensure_tag(tx, tag, uid, gid, permissions, now)?;
            }
            _ => {}
        }
    } else {
        let pinnable = tags.iter().collect_pinnable();
        if !pinnable.is_empty() {
            debug!(target: WRAPPER_TAG, "{:?} is a nested tag, pinning it", dir);
            sql::pin_tags(&tx, pinnable.as_slice(), uid, gid, permissions, now)?;
        }
    }

    Ok(())
}
