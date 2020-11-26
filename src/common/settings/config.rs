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
use crate::common::constants;
use crate::common::types::file_perms::Permissions;
use ::config::{ConfigError, Source, Value};
use libc::{gid_t, uid_t};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct HashMapSource(pub HashMap<String, config::Value>);

impl config::Source for HashMapSource {
    fn clone_into_box(&self) -> Box<dyn Source + Send + Sync> {
        Box::new((*self).clone())
    }

    fn collect(&self) -> Result<HashMap<String, Value>, ConfigError> {
        Ok(self.0.clone())
    }
}

/// These are mount settings.  They only apply to the root dir, the mounted dir.  Other permissions, for other dirs,
/// are derived from the fuse config umask and uid/gid fields.
#[derive(Serialize, Deserialize, Clone)]
pub struct Mount {
    pub base_dir: PathBuf,

    pub uid: uid_t,
    pub gid: gid_t,
    pub permissions: Permissions,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Symbols {
    pub device_char: char,
    pub inode_char: char,
    pub sync_char: char,
    pub filedir_str: String,
    pub filedir_cli_str: String,
    pub tag_group_str: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Config {
    pub symbols: Symbols,
    pub mount: Mount,
}

/// Builds a default config based off of our default toml, environment variables, and a specified app toml file
pub fn build<T>(source: T, project_dirs: &dyn super::dirs::Dirs) -> ::config::Config
where
    T: config::Source + Send + Sync + 'static,
{
    let mut merged_config = config::Config::new();

    merged_config
        .merge(config::File::from_str(
            constants::DEFAULT_CONFIG_TOML,
            config::FileFormat::Toml,
        ))
        .expect("Unable to merge default config")
        .merge(source)
        .expect("Unable to merge app config")
        .merge(config::Environment::with_prefix(
            super::constants::ENV_PREFIX,
        ))
        .expect("Unable to merge settings from environment variables")
        .set_default(
            "mount.base_dir",
            project_dirs
                .mount_dir()
                .to_str()
                .expect("Unable to determine platform mountdir"),
        )
        .expect("Couldn't set default for mount.base_dir");

    merged_config
}
