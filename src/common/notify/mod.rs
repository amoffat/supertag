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

use crate::common::types::note::Note;
use std::error::Error;
use std::path::Path;
use std::time::Duration;

pub mod desktop;
pub mod listener;
pub mod uds;

pub trait Notifier: Send {
    type Listener: Listener;

    /// When a user copies the file, instead of symlinking it
    fn bad_copy(&self) -> Result<(), Box<dyn Error>>;

    /// When a user tries to symlink in the root directory, not a tag directory
    fn dragged_to_root(&self) -> Result<(), Box<dyn Error>>;

    /// When a user attempts a regular delete instead of renaming delete
    fn unlink(&self, path: &Path) -> Result<(), Box<dyn Error>>;

    /// When a user attempts to rename a non-empty tag to a tag group
    fn tag_to_tg(&self, tag: &str) -> Result<(), Box<dyn Error>>;

    fn listener(&self) -> Result<Self::Listener, Box<dyn Error>>;
}

pub trait Listener {
    fn marker(&self) -> usize;
    fn wait_for_pred(
        &mut self,
        pred: impl Fn(&Note) -> bool,
        timeout: Duration,
        idx: usize,
    ) -> Option<(Note, usize)>;
    fn wait_for(&mut self, note: &Note, timeout: Duration, marker: usize) -> bool;

    /// The number of notes the listener has seen
    fn note_count(&self) -> usize;
}
