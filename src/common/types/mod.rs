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

use crate::common::constants::NEGATIVE_TAG_PREFIX;
use crate::common::err::{STagError, STagResult};
use crate::common::set_ext_prefix;
use crate::common::settings::Settings;
use crate::sql::types::TaggedFile;
use std::fmt::Debug;
use std::path::{Path, PathBuf};
use std::slice::Iter;

pub type UtcDt = chrono::DateTime<chrono::Utc>;

pub mod cli;
pub mod file_perms;
pub mod note;

#[derive(PartialEq, Eq, Debug, Clone)]
pub struct DeviceFile {
    pub filename: String,
    pub device: u64,
    pub inode: u64,
}

impl DeviceFile {
    pub fn new(filename: &str, device: u64, inode: u64) -> Self {
        Self {
            filename: filename.to_string(),
            device,
            inode,
        }
    }

    pub fn from_path(path: impl AsRef<Path>) -> STagResult<Self> {
        let (device, inode) = super::get_device_inode(path.as_ref())?;

        let filename = path
            .as_ref()
            .file_name()
            .ok_or(STagError::InvalidPath(path.as_ref().to_owned()))?
            .to_string_lossy()
            .to_string();

        Ok(Self {
            filename,
            device,
            inode,
        })
    }

    pub fn inodify(&self, settings: &Settings) -> String {
        settings.inodify_filename(&self.filename, self.device, self.inode)
    }

    pub fn matches(&self, tf: &TaggedFile) -> bool {
        tf.primary_tag == self.filename && tf.device == self.device && tf.inode == self.inode
    }
}

impl From<TaggedFile> for DeviceFile {
    fn from(tf: TaggedFile) -> Self {
        Self::new(&tf.primary_tag, tf.device, tf.inode)
    }
}

impl std::fmt::Display for DeviceFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(
            f,
            "<DeviceFile filename={} device={} inode={}>",
            self.filename, self.device, self.inode
        )
    }
}

#[derive(PartialEq, Eq, Debug, Clone)]
pub enum TagType {
    Regular(String),
    Negation(String),
    Group(String),
    FileDir,
    DeviceFileSymlink(DeviceFile),
    Symlink(String),
}

impl TagType {
    fn to_path_part(&self, settings: &Settings) -> String {
        let syms = &settings.get_config().symbols;
        match self {
            TagType::Regular(tag) => tag.to_string(),
            TagType::Negation(tag) => format!("{}{}", NEGATIVE_TAG_PREFIX, tag),
            TagType::Group(tag) => set_ext_prefix(&tag, &syms.tag_group_str),
            TagType::FileDir => syms.filedir_str.to_string(),
            TagType::DeviceFileSymlink(df) => df.inodify(settings),
            TagType::Symlink(f) => f.to_string(),
        }
    }
}

impl std::fmt::Display for TagType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TagType::Regular(tag) => write!(f, "Regular({})", tag),
            TagType::Negation(tag) => write!(f, "Negation({})", tag),
            TagType::Group(tag) => write!(f, "Group({})", tag),
            TagType::FileDir => write!(f, "FileDir"),
            TagType::DeviceFileSymlink(df) => write!(f, "{}", df),
            TagType::Symlink(fl) => write!(f, "Symlink({})", fl),
        }
    }
}

pub trait TagCollectible<'a> {
    fn collect_regular_names(self) -> Vec<&'a str>;
    fn collect_regular(self) -> Vec<TagType>;
    fn collect_pinnable(self) -> Vec<TagType>;
    fn collect_tags_and_groups(self) -> Vec<TagType>;
    fn taggroup_pairs(self) -> Vec<(&'a str, &'a str)>;
}

impl<'a, T> TagCollectible<'a> for T
where
    T: Iterator<Item = &'a TagType>,
{
    fn collect_regular_names(self) -> Vec<&'a str> {
        self.filter_map(|tt| {
            if let TagType::Regular(tag) = tt {
                Some(tag.as_str())
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
    }

    fn collect_regular(self) -> Vec<TagType> {
        self.filter_map(|tt| {
            if let TagType::Regular(_tag) = tt {
                Some(tt.to_owned())
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
    }

    /// Collect together all the tagtypes that can be considered valid for pinning.
    /// This filters out consecutive tag groups, as it doesn't make sense (does it?)
    fn collect_pinnable(self) -> Vec<TagType> {
        let mut pinnable = vec![];
        let mut last_was_group = false;
        for tt in self {
            match tt {
                TagType::Regular(_tag) => {
                    pinnable.push(tt.to_owned());
                    last_was_group = false;
                }
                TagType::Group(_tag) => {
                    if last_was_group {
                        continue;
                    }
                    pinnable.push(tt.to_owned());
                    last_was_group = true;
                }
                _ => {
                    last_was_group = false;
                }
            }
        }
        pinnable
    }

    fn collect_tags_and_groups(self) -> Vec<TagType> {
        self.filter_map(|tt| match tt {
            TagType::Regular(_tag) | TagType::Group(_tag) => Some(tt.to_owned()),
            _ => None,
        })
        .collect::<Vec<_>>()
    }

    fn taggroup_pairs(self) -> Vec<(&'a str, &'a str)> {
        self.collect::<Vec<_>>()
            .windows(2)
            .filter_map(|el| {
                if let (TagType::Group(group), TagType::Regular(tag)) = (el[0], el[1]) {
                    Some((group.as_str(), tag.as_str()))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
    }
}

#[derive(Debug)]
pub struct TagCollection {
    path: PathBuf,
    tags: Vec<TagType>,
    pub unlinking: bool,
}

impl std::fmt::Display for TagCollection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.tags.fmt(f)
    }
}

impl TagCollection {
    pub fn new(settings: &Settings, path: &Path) -> Self {
        let unlinking = path
            .to_str()
            .unwrap()
            .ends_with(settings.get_config().symbols.sync_char);
        Self {
            path: path.to_path_buf(),
            tags: settings.path_to_tags(path),
            unlinking,
        }
    }

    pub fn pop(&mut self) -> Option<TagType> {
        self.tags.pop()
    }

    pub fn push(&mut self, val: TagType) {
        self.tags.push(val)
    }

    pub fn len(&self) -> usize {
        self.tags.len()
    }

    pub fn iter(&self) -> Iter<TagType> {
        self.tags.iter()
    }

    pub fn first(&self) -> Option<&TagType> {
        self.tags.first()
    }

    pub fn last(&self) -> Option<&TagType> {
        self.tags.last()
    }

    pub fn as_slice(&self) -> &[TagType] {
        self.tags.as_slice()
    }

    pub fn join_path(&self, settings: &Settings) -> PathBuf {
        self.iter()
            .map(|t| t.to_path_part(settings))
            .collect::<Vec<_>>()
            .join(&std::path::MAIN_SEPARATOR.to_string())
            .into()
    }

    pub fn all_but_last(&self) -> Iter<TagType> {
        self.iter().as_slice()[..self.len() - 1].iter()
    }

    pub fn primary_parent(&self) -> Option<&TagType> {
        let parent_idx = self.tags.len() as i32 - 2;
        if parent_idx < 0 {
            None
        } else {
            self.tags.get(parent_idx as usize)
        }
    }

    pub fn primary_type(&self) -> STagResult<&TagType> {
        self.last().ok_or(STagError::NotEnoughTags)
    }
}
