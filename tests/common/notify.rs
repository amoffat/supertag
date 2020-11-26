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

use log::{debug, info, warn};
use std::error::Error;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use supertag::common::notify::{Listener, Notifier};
use supertag::common::types::note::Note;

const TAG: &str = "test_notify";

pub struct TestNotifier {
    pub notes: Arc<Mutex<Vec<Note>>>,
}

impl TestNotifier {
    pub fn new() -> Self {
        Self {
            notes: Arc::new(Mutex::new(vec![])),
        }
    }

    pub fn get_notes(&self) -> MutexGuard<Vec<Note>> {
        self.notes.lock().unwrap()
    }

    pub fn last(&self) -> Option<Note> {
        self.notes.lock().unwrap().last().map(Note::to_owned)
    }
}

impl Notifier for TestNotifier {
    type Listener = TestListener;

    fn bad_copy(&self) -> Result<(), Box<dyn Error>> {
        info!(target: TAG, "bad_copy");
        self.notes.lock().unwrap().push(Note::BadCopy);
        Ok(())
    }

    fn dragged_to_root(&self) -> Result<(), Box<dyn Error>> {
        info!(target: TAG, "dragged_to_root");
        self.notes.lock().unwrap().push(Note::DraggedToRoot);
        Ok(())
    }

    fn unlink(&self, path: &Path) -> Result<(), Box<dyn Error>> {
        info!(target: TAG, "unlink");
        self.notes
            .lock()
            .unwrap()
            .push(Note::Unlink(path.to_owned()));
        Ok(())
    }

    fn tag_to_tg(&self, tag: &str) -> Result<(), Box<dyn Error>> {
        info!(target: TAG, "tag_to_tg");
        self.notes
            .lock()
            .unwrap()
            .push(Note::TagToTagGroup(tag.to_owned()));
        Ok(())
    }

    fn listener(&self) -> Result<Self::Listener, Box<dyn Error>> {
        Ok(Self::Listener::new(self.notes.clone()))
    }
}

pub struct TestListener {
    notes: Arc<Mutex<Vec<Note>>>,
}

impl TestListener {
    pub fn new(notes: Arc<Mutex<Vec<Note>>>) -> Self {
        Self { notes }
    }
}

impl Listener for TestListener {
    fn marker(&self) -> usize {
        self.notes.lock().unwrap().len()
    }

    fn wait_for_pred(
        &mut self,
        _pred: impl Fn(&Note) -> bool,
        _timeout: Duration,
        _idx: usize,
    ) -> Option<(Note, usize)> {
        None
    }

    fn wait_for(&mut self, note: &Note, timeout: Duration, idx: usize) -> bool {
        let start = std::time::Instant::now();
        loop {
            {
                let notes = self.notes.lock().unwrap();
                debug!(
                    target: TAG,
                    "Looking for note {:?} in {} notes, starting at {}",
                    note,
                    notes.len(),
                    idx
                );
                for cand in notes.iter().skip(idx) {
                    debug!(target: TAG, "Checking {:?}", cand);
                    if note == cand {
                        debug!(target: TAG, "Found note!");
                        return true;
                    }
                }
            }
            let elapsed = std::time::Instant::now() - start;
            if elapsed > timeout {
                warn!(target: TAG, "Timeout looking for note {:?}", note);
                return false;
            }

            std::thread::sleep(std::time::Duration::from_millis(1000));
        }
    }
}
