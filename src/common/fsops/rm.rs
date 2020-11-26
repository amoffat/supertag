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
use log::info;

/// `file` must be relative to the collection, not an absolute path
pub fn rm(settings: &Settings, tx: &Transaction, file: &Path) -> STagResult<Vec<i64>> {
    info!(target: WRAPPER_TAG, "rm {:?}", file);

    let tags = TagCollection::new(settings, file);
    let now = sql::get_now_secs();

    match tags.primary_type()? {
        TagType::DeviceFileSymlink(device_file) => {
            let last_tag = tags
                .iter()
                .collect_regular_names()
                .last()
                .unwrap()
                .to_owned();
            let removed = sql::remove_devicefile(tx, &device_file, &[last_tag], now)?;
            Ok(removed)
        }
        TagType::Symlink(filename) => {
            let last_tag = tags.iter().collect_regular().last().unwrap().to_owned();
            let removed = sql::remove_links(tx, filename, &[last_tag], now)?;
            Ok(removed)
        }
        _ => Err(STagError::InvalidPath(file.into())),
    }
}
