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

use crate::common;
use crate::common::settings::Settings;
use crate::common::types::file_perms::Permissions;
use crate::common::types::UtcDt;
use fuse_sys::FileEntry;
use libc::{gid_t, uid_t};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct TaggedFile {
    pub id: i64,
    pub inode: u64,
    pub device: u64,
    pub path: String,
    pub primary_tag: String,
    pub mtime: UtcDt,
    pub uid: uid_t,
    pub gid: gid_t,
    pub permissions: Permissions,
    pub alias_file: Option<String>,
}

impl TaggedFile {
    pub fn resolve_path(&self) -> PathBuf {
        #[cfg(target_os = "macos")]
        {
            if let Some(alias) = &self.alias_file {
                if let Ok(resolved) = crate::platform::mac::alias::resolve_alias(alias) {
                    resolved
                } else {
                    PathBuf::from(&self.path)
                }
            } else {
                PathBuf::from(&self.path)
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            PathBuf::from(&self.path)
        }
    }
}

impl From<TaggedFile> for FileEntry {
    fn from(tf: TaggedFile) -> Self {
        FileEntry {
            name: tf.primary_tag,
            mtime: tf.mtime,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Tag {
    pub id: i64,
    pub name: String,
    pub mtime: UtcDt,
    pub uid: uid_t,
    pub gid: gid_t,
    pub permissions: Permissions,
    pub num_files: i64,
}

impl From<Tag> for FileEntry {
    fn from(tag: Tag) -> Self {
        FileEntry {
            name: tag.name,
            mtime: tag.mtime,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TagGroup {
    pub id: i64,
    pub name: String,
    pub mtime: UtcDt,
    pub uid: uid_t,
    pub gid: gid_t,
    pub permissions: Permissions,
    pub tag_ids: Vec<i64>,
    pub num_files: i64,
}

impl TagGroup {
    pub fn to_fileentry(&self, settings: &Settings) -> FileEntry {
        FileEntry {
            name: common::name_to_tag_group(settings, &self.name),
            mtime: self.mtime,
        }
    }
}

#[derive(Debug)]
pub enum TagOrTagGroup {
    Tag(Tag),
    Group(TagGroup),
}

impl TagOrTagGroup {
    #[allow(dead_code)]
    fn to_fileentry(&self, settings: &Settings) -> FileEntry {
        match self {
            TagOrTagGroup::Group(group) => group.to_fileentry(settings),
            TagOrTagGroup::Tag(tag) => tag.to_owned().into(),
        }
    }
}
