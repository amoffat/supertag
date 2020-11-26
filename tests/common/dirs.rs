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
use supertag::common::settings::dirs::Dirs;

pub struct TestDirectories {
    test_basedir: PathBuf,
    dir: tempfile::TempDir,
    project: PathBuf,
    cache: PathBuf,
    config: PathBuf,
    data: PathBuf,
    data_local: PathBuf,
}

impl Dirs for TestDirectories {
    fn project_path(&self) -> &Path {
        &self.project
    }

    fn cache_dir(&self) -> &Path {
        &self.cache
    }

    fn config_dir(&self) -> &Path {
        &self.config
    }

    fn data_dir(&self) -> &Path {
        &self.data
    }

    fn data_local_dir(&self) -> &Path {
        &self.data_local
    }
}

impl TestDirectories {
    pub fn new(test_basedir: PathBuf) -> Self {
        let dir = tempfile::Builder::new().prefix("pd-").tempdir().unwrap();
        let p = dir.path().to_owned();
        Self {
            test_basedir,
            dir,
            project: p.join("project"),
            cache: p.join("cache"),
            config: p.join("config"),
            data: p.join("data"),
            data_local: p.join("data_local"),
        }
    }

    pub fn base(&self) -> PathBuf {
        self.dir.path().to_owned()
    }
}
