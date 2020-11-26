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

pub const VERSION: (&str, &str, &str) = (
    env!("CARGO_PKG_VERSION_MAJOR"),
    env!("CARGO_PKG_VERSION_MINOR"),
    env!("CARGO_PKG_VERSION_PATCH"),
);
pub const ENV_PREFIX: &str = "STAG";
pub const APP_NAME: &str = "supertag";
pub const AUTHOR: &str = "Andrew Moffat";
pub const ORG: &str = "ai.supertag";

// MacOS
pub const FSEVENTS_PATH: &str = "/.fseventsd";
pub const FSEVENTS_NO_LOG_PATH: &str = "/.fseventsd/no_log";
pub const NO_INDEX_PATH: &str = "/.metadata_never_index";

// GNOME tracker/indexer
pub const TRACKER_IGNORE: &str = ".trackerignore";

// TODO put this in the settings symbols
pub const NEGATIVE_TAG_PREFIX: &str = "-";

pub const DB_FILE_NAME: &str = "db.sqlite3";
pub const DB_FILE_PATH: &str = "/.supertag/db.sqlite3";

pub const STAG_ROOT_CONF_PATH: &str = "/.supertag";
pub const STAG_ROOT_CONF_NAME: &str = ".supertag";

// this is the file that the face detector puts in the top level. this isn't entirely accurate, and it's mostly for
// the tests
// TODO move this to test files
pub const FACE_NAME: &str = "face|.png";

pub const MANAGED_FILES_DIR_NAME: &str = "managed_files";

// an unlink on this file helps us detect whether we're deleting an entire directory tree recursively or deleting a
// single file.
pub const UNLINK_CANARY: &str = ".unlink_canary";
#[cfg(target_os = "macos")]
pub const FOLDER_ICON: &str = "Icon\r";

#[cfg(target_os = "macos")]
pub const XATTR_FINDER_INFO: &str = "com.apple.FinderInfo";
#[cfg(target_os = "macos")]
pub const XATTR_RESOURCE_FORK: &str = "com.apple.ResourceFork";

pub const ALIAS_HEADER: &[u8] = b"book\0\0\0\0mark";

pub const UNLINK_NAME: &str = "delete";

pub const DEFAULT_CONFIG_TOML: &str = r###"
[symbols]
inode_char = "-"
device_char = "﹫"
sync_char = "\u007F"
filedir_str = "⋂"
filedir_cli_str = "_"
tag_group_str = "+"

[mount]
"###;

// https://github.com/torvalds/linux/blob/master/Documentation/admin-guide/devices.txt
// 60-63 LOCAL/EXPERIMENTAL USE
// FIXME why can't I actually set this? `stat` on the mountpoint yields a random device id
pub const DEVICE_ID: u64 = 63;
