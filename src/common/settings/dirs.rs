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
use directories::ProjectDirs;
use std::path::{Path, PathBuf};

pub trait Dirs: Send + Sync {
    fn project_path(&self) -> &Path;
    fn cache_dir(&self) -> &Path;
    fn config_dir(&self) -> &Path;
    fn data_dir(&self) -> &Path;
    fn data_local_dir(&self) -> &Path;
    fn mount_dir(&self) -> PathBuf {
        crate::platform::mountdir()
    }
}

impl Dirs for ProjectDirs {
    fn project_path(&self) -> &Path {
        ProjectDirs::project_path(self)
    }

    fn cache_dir(&self) -> &Path {
        ProjectDirs::cache_dir(self)
    }

    fn config_dir(&self) -> &Path {
        ProjectDirs::config_dir(self)
    }

    fn data_dir(&self) -> &Path {
        ProjectDirs::data_dir(self)
    }

    fn data_local_dir(&self) -> &Path {
        ProjectDirs::data_local_dir(self)
    }
}
