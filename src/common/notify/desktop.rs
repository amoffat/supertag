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

use super::Notifier;
use crate::common::constants;
use crate::common::notify::Listener;
use crate::common::types::note::Note;
use log::info;
use notify_rust::{Notification, Timeout};
use std::cell::RefCell;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub struct DesktopNotifier {
    tag: String,
    icon: Option<PathBuf>,
    last_message: RefCell<Instant>,
}

impl DesktopNotifier {
    pub fn new(icon: Option<PathBuf>) -> Self {
        let tag = "desktop-notification".to_string();
        Self {
            tag,
            icon,
            last_message: RefCell::new(Instant::now()),
        }
    }

    fn send_message(&self, note: Note) -> Result<(), Box<dyn Error>> {
        let elapsed = self.last_message.borrow().elapsed();
        self.last_message.replace(Instant::now());

        if elapsed.as_millis() < 500 {
            return Ok(());
        }

        let mut base_note = Notification::new();

        #[cfg(target_os = "linux")]
        if let Some(icon) = &self.icon {
            base_note.icon(&icon.to_string_lossy());
        }
        base_note
            .summary("Supertag Error")
            .timeout(Timeout::Milliseconds(6000));

        let full_note = match note {
            Note::BadCopy => base_note.body("Cannot copy file into collection, symlink instead"),
            Note::DraggedToRoot => base_note.body("Cannot tag a file in the root collection"),
            Note::Unlink(_) => base_note.body(&*format!(
                "Delete by renaming folder to '{}'",
                constants::UNLINK_NAME
            )),
            Note::TagToTagGroup(_) => {
                base_note.body("Cannot change a non-empty tag to a tag group")
            }
        };

        full_note.show()?;
        Ok(())
    }
}

impl Notifier for DesktopNotifier {
    type Listener = ();

    fn bad_copy(&self) -> Result<(), Box<dyn Error>> {
        info!(target: &self.tag, "bad_copy");
        self.send_message(Note::BadCopy)?;
        Ok(())
    }

    fn dragged_to_root(&self) -> Result<(), Box<dyn Error>> {
        info!(target: &self.tag, "dragged_to_root");
        self.send_message(Note::DraggedToRoot)?;
        Ok(())
    }

    fn unlink(&self, path: &Path) -> Result<(), Box<dyn Error>> {
        info!(target: &self.tag, "unlink");
        self.send_message(Note::Unlink(path.to_owned()))?;
        Ok(())
    }

    fn tag_to_tg(&self, tag: &str) -> Result<(), Box<dyn Error>> {
        info!(target: &self.tag, "tag_to_tg");
        self.send_message(Note::TagToTagGroup(tag.to_owned()))?;
        Ok(())
    }

    fn listener(&self) -> Result<Self::Listener, Box<dyn Error>> {
        Ok(())
    }
}

impl Listener for () {
    fn marker(&self) -> usize {
        unimplemented!()
    }

    fn wait_for_pred(
        &mut self,
        _pred: impl Fn(&Note) -> bool,
        _timeout: Duration,
        _idx: usize,
    ) -> Option<(Note, usize)> {
        unimplemented!()
    }

    fn wait_for(&mut self, _note: &Note, _timeout: Duration, _marker: usize) -> bool {
        unimplemented!()
    }
}
