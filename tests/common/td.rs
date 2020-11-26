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
use std::path::PathBuf;

pub struct TempDir(tempfile::TempDir);

impl TempDir {
    pub fn new() -> Self {
        let mut builder = tempfile::Builder::new();
        // we change from the default prefix of ".tmp" because our importing code won't import temp files/directories
        builder.prefix("supertag-");
        Self(builder.tempdir().unwrap())
    }
    pub fn path(&self) -> PathBuf {
        self.0.path().canonicalize().unwrap()
    }
}
