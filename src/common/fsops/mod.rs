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

//! These functions contain common entry points for manipulating the filesystem with symlink, rm
//! rmdir, etc.  They are called by the `tag` binary and also through the FUSE interface, which are
//! the only two ways to manipulate supertag files.

const WRAPPER_TAG: &str = "ops_wrapper";

mod ln;
mod mkdir;
mod mv;
mod rm;
mod rmdir;

use crate::common::settings::Settings;
use crate::common::types::TagCollectible;
pub use ln::ln;
use log::debug;
pub use mkdir::mkdir;
pub use mv::move_or_merge;
pub use rm::rm;
pub use rmdir::rmdir;
use std::path::Path;

const TAG: &str = "fsops";

// but now we need to communicate to supertag that we want to clear the entry from its caches.
// we do this by removing the file, but appending a special char, so that when supertag sees this
// path in the unlink handler, it will know that we just want it cleared from the caches
pub fn flush_path(path: impl AsRef<Path>, settings: &Settings) {
    let _ = settings.suffix_sync_char(path.as_ref()).map(|p| {
        debug!(target: TAG, "Sending readdir cache sync for {:?}", p);
        let _ = std::fs::metadata(p);
    });
}

pub fn flush_tags(rel_path: &Path, settings: &Settings, mountpoint: impl AsRef<Path>) {
    let tags = settings.path_to_tags(rel_path);
    for tag in tags.iter().collect_regular_names() {
        let tag_path = mountpoint.as_ref().join(tag);
        flush_path(tag_path, settings);
    }
}
