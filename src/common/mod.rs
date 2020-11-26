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

use std::path::{Path, PathBuf};

use super::common::constants::NEGATIVE_TAG_PREFIX;
use super::common::err::STagResult;
use crate::common::constants::VERSION;
use crate::common::settings::Settings;
use nix::sys::stat::stat;

pub mod constants;
pub mod err;
pub mod fsops;
pub mod iter;
pub mod log;
pub mod managed_file;
pub mod notify;
pub mod settings;
pub mod types;
pub mod xattr;

/// Takes a normal path on the filesystem and gets the device and inode nums
pub fn get_device_inode(path: &Path) -> err::STagResult<(u64, u64)> {
    let st = stat(path)?;
    // on macos, st_dev is a i32.
    let dev = st.st_dev as u64;
    Ok((dev, st.st_ino))
}

pub fn get_filename(path: &Path) -> STagResult<&str> {
    Ok(path
        .components()
        .last()
        .ok_or_else(|| err::STagError::InvalidPath(path.to_owned()))?
        .as_os_str()
        .to_str()
        .ok_or_else(|| err::STagError::InvalidPath(path.to_owned()))?)
}

/// Parses out the `primary_tag` portion of a path, which is essentially the filename sans the
/// special characters we tack onto the end.
pub fn primary_tag(path: &Path, device_char: char) -> Result<Option<String>, err::STagError> {
    let filename = get_filename(path)?;
    let mut primary_tag_chars = Vec::new();
    for letter in filename.chars() {
        if letter == device_char {
            break;
        }
        primary_tag_chars.push(letter);
    }

    if primary_tag_chars.len() > 1 {
        let primary_tag = primary_tag_chars.into_iter().collect();
        return Ok(Some(primary_tag));
    } else {
        return Ok(None);
    }
}

pub fn strip_negative_tag(tag: &str) -> Option<&str> {
    if tag.starts_with(NEGATIVE_TAG_PREFIX) {
        Some(&tag[NEGATIVE_TAG_PREFIX.len()..])
    } else {
        None
    }
}

pub fn strip_ext_prefix(name: &str, prefix: &str) -> Option<String> {
    let parts: Vec<&str> = name.rsplitn(2, ".").collect();

    match parts.len() {
        1 => {
            if parts[0].ends_with(prefix) {
                Some(parts[0][..parts[0].len() - prefix.len()].to_string())
            } else {
                None
            }
        }
        _ => {
            if parts[1].ends_with(prefix) {
                let name = &parts[1][..parts[1].len() - prefix.len()];
                Some(format!("{name}.{ext}", name = name, ext = parts[0]))
            } else {
                None
            }
        }
    }
}

/// Checks if `to_check` exists right before a (possibly non-existent) file extension
pub fn has_ext_prefix(name: &str, to_check: &str) -> bool {
    let parts: Vec<&str> = name.rsplitn(2, ".").collect();
    match parts.len() {
        1 => parts[0].ends_with(to_check),
        _ => parts[1].ends_with(to_check),
    }
}

pub fn set_ext_prefix(name: &str, to_prefix: &str) -> String {
    let parts: Vec<&str> = name.rsplitn(2, ".").collect();
    match parts.len() {
        1 => format!("{name}{prefix}", name = name, prefix = to_prefix),
        _ => format!(
            "{name}{prefix}.{ext}",
            name = parts[1],
            prefix = to_prefix,
            ext = parts[0]
        ),
    }
}

pub fn creatable_tag_group(settings: &Settings, name: &str) -> bool {
    !has_ext_prefix(name, &settings.get_config().symbols.tag_group_str)
        && !name.contains(std::path::MAIN_SEPARATOR)
        && name != settings.get_config().symbols.filedir_str
}

pub fn name_to_tag_group(settings: &Settings, name: &str) -> String {
    set_ext_prefix(name, &settings.get_config().symbols.tag_group_str)
}

pub fn should_unlink(name: &str) -> bool {
    // on macos, it's not possible(?) to rename a file in Finder and leave off the extension, even when extensions are
    // visible.  so we do this instead to allow for the extension.
    #[cfg(target_os = "macos")]
    {
        name.starts_with(&format!("{}.", constants::UNLINK_NAME)) || name == constants::UNLINK_NAME
    }
    #[cfg(not(target_os = "macos"))]
    {
        name == constants::UNLINK_NAME
    }
}

/// Provides a read interface to a slice, similar
pub fn read_from_slice<T: Copy>(src: &[T], dst: &mut [T], offset: usize) -> usize {
    let desired = dst.len();
    if offset > src.len() {
        0
    } else {
        let read = std::cmp::min(src.len() - offset, desired);
        let slice = &src[offset..offset + read];
        dst[0..read].copy_from_slice(slice);

        read
    }
}

pub fn version_str() -> String {
    format!("{}.{}.{}", VERSION.0, VERSION.1, VERSION.2)
}

/// Returns the APPDIR, if we're in an app image
pub fn appdir() -> Option<PathBuf> {
    std::env::var_os("APPDIR").map(|a| PathBuf::from(a))
}
