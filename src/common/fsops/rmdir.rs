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
use crate::common::settings::Settings;
use crate::common::types::{TagCollectible, TagCollection, TagType};
use crate::sql;
use log::{debug, info};

/// `path` must be relative to the mountpoint!
pub fn rmdir(settings: &Settings, tx: &Transaction, path: &Path) -> STagResult<()> {
    info!(target: WRAPPER_TAG, "rmdir {:?}", path);

    let tags = TagCollection::new(settings, path);
    let pt = tags.primary_type()?;
    let now = sql::get_now_secs();

    match pt {
        TagType::Group(group) => {
            debug!(
                target: WRAPPER_TAG,
                "It's a tag group, attemting to remove it in some form"
            );

            let parts = tags.iter().collect_tags_and_groups();
            match parts.len() {
                0 => Err(STagError::InvalidPath(path.into())),
                1 => {
                    sql::remove_taggroup(tx, group)?;
                    Ok(())
                }
                _ => {
                    sql::remove_taggroup_from_itersection(tx, group, tags.as_slice())?;
                    Ok(())
                }
            }
        }
        TagType::Regular(tag) => {
            debug!(
                target: WRAPPER_TAG,
                "It's a regular tag, attempting to remove it in some form"
            );
            let intersect = tags.iter().collect_tags_and_groups();
            match intersect.len() {
                0 => Err(STagError::InvalidPath(path.into())),
                1 => {
                    debug!(
                        target: WRAPPER_TAG,
                        "Only one tag, {:?}, removing that from the top level", intersect
                    );
                    sql::remove_tag(tx, tag, now, true)?;
                    Ok(())
                }
                _ => {
                    debug!(
                        target: WRAPPER_TAG,
                        "Removing {} from the intersection of {:?}", tag, intersect
                    );
                    let removed =
                        sql::remove_tag_from_intersection(tx, tag, intersect.as_slice(), now)?;
                    debug!(
                        target: WRAPPER_TAG,
                        "Removed {} file associations",
                        removed.len()
                    );
                    Ok(())
                }
            }
        }
        _ => Err(STagError::InvalidPath(path.into())),
    }
}
