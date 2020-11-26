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

// we could use cfg_if here, but intellij currently isn't smart enough to build the module tree with
// cfg_if, so let's do it this way so that our platform modules have syntax highlighting and auto
// completion
#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "linux")]
pub use linux::*;

#[cfg(target_os = "macos")]
pub mod mac;
#[cfg(target_os = "macos")]
pub use mac::*;

use log::debug;

const PLATFORM_TAG: &str = "platform";

use crate::common::settings::Settings;
use std::collections::HashMap;

/// Sorts the known collections (collections with a directory in the collections directory) by the
/// collection creation time, as far as we can determine
pub fn all_collections(settings: &Settings) -> std::io::Result<Vec<String>> {
    let mut entries: Vec<(String, std::time::SystemTime)> =
        std::fs::read_dir(settings.collections_dir())?
            .filter_map(|e| e.ok())
            .filter_map(|e| {
                let maybe_t = e.metadata().and_then(|md| md.created()).ok();
                if let Some(t) = maybe_t {
                    Some((e.file_name().as_os_str().to_str()?.to_owned(), t))
                } else {
                    None
                }
            })
            .collect();

    entries.sort_by_cached_key(|(_col, btime)| btime.to_owned());
    Ok(entries.iter().map(|(col, _btime)| col.to_owned()).collect())
}

/// Returns the first found collection in the mount directory.  It's possible that it doesn't find
/// anything
pub fn primary_collection(
    settings: &Settings,
) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let all_cols = all_collections(settings)?;
    debug!(target: PLATFORM_TAG, "Got all collections {:?}", all_cols);

    // all mounted collections
    let mc = mounted_collections()?;
    debug!(target: PLATFORM_TAG, "Got mounted collections {:?}", mc);

    // now let's loop through our known collections, sorted by creation time.  if we find a
    // collection that is already mounted, then it is our primary collection
    for sorted_col in all_cols {
        if mc.contains_key(&sorted_col) {
            return Ok(Some(sorted_col));
        }
    }

    Ok(None)
}

/// Returns all mounted collections
pub fn mounted_collections() -> Result<HashMap<String, String>, Box<dyn std::error::Error>> {
    let output = std::process::Command::new("mount").output()?.stdout;
    let supertag_re = regex::Regex::new(r"(?m)^supertag:(.+?) on (.+?)\s").unwrap();

    Ok(supertag_re
        .captures_iter(&String::from_utf8(output).unwrap())
        .map(|el| {
            (
                el.get(1).unwrap().as_str().to_string(),
                el.get(2).unwrap().as_str().to_string(),
            )
        })
        .collect())
}
