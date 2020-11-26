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

use super::{TestHelper, TestResult};
use crate::common::OpMode;
use std::time::Duration;
use supertag::common::err::STagError;
use supertag::common::notify::{Listener, Notifier};
use supertag::common::types::note::Note;

#[test]
/// Tests that copying a file (non-alias) launches a notification
fn test_bad_copy() -> TestResult {
    let th = TestHelper::new(None);
    th.mkdir("t1")?;

    let to_copy = tempfile::NamedTempFile::new()?;

    let mut listener = th
        .notifier
        .lock()
        .listener()
        .expect("Couldn't get listener");
    let idx = listener.marker();

    let res = std::fs::copy(
        to_copy.path(),
        th.mountpoint_path(&["t1"]).join(
            to_copy
                .path()
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string(),
        ),
    );

    match res {
        // we use NotFound here because on macos, a failure is only detected when we try to do something like chmod
        // on the created file. not sure why macos even tries to do the chmod, but it's where the failure occurs
        #[cfg(target_os = "macos")]
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        #[cfg(target_os = "linux")]
        Err(e) if e.kind() == std::io::ErrorKind::Other => {}
        Err(e) => panic!("Wrong error: {:?}", e),
        Ok(_) => panic!("Should have had an error"),
    }

    th.assert_note(
        &mut listener,
        idx,
        &[&Note::BadCopy],
        Duration::from_secs(3),
    );

    Ok(())
}

#[test]
fn test_dragged_to_root_cli() -> TestResult {
    let th = TestHelper::new(None);
    _test_dragged_to_root(th)
}

#[test]
fn test_dragged_to_root_manual() -> TestResult {
    let mut th = TestHelper::new(None);
    th.symlink_mode = OpMode::MANUAL;
    _test_dragged_to_root(th)
}

#[test]
#[cfg(target_os = "macos")]
fn test_dragged_to_root_finder() -> TestResult {
    let mut th = TestHelper::new(None);
    th.symlink_mode = OpMode::FINDER;
    _test_dragged_to_root(th)
}

/// Tests that creating a symlink in the root of the filesystem is not allowed and also creates a notification message.
/// This often happens on macos during drag and drop, and missing the tag folder.
fn _test_dragged_to_root(th: TestHelper) -> TestResult {
    let mut listener = th
        .notifier
        .lock()
        .listener()
        .expect("Couldn't get listener");
    let idx = listener.marker();

    match th.ln(&[]) {
        Err(STagError::IOError(_)) if th.symlink_mode == OpMode::MANUAL => {}
        Err(STagError::InvalidPath(_)) if th.symlink_mode == OpMode::CLI => {}
        #[cfg(target_os = "macos")]
        Ok(_) if th.symlink_mode == OpMode::FINDER => {}
        Err(e) => panic!("Wrong error {:?}", e),
        Ok(_) => panic!("Should have had an error"),
    }

    th.assert_note(
        &mut listener,
        idx,
        &[&Note::DraggedToRoot],
        Duration::from_secs(3),
    );

    Ok(())
}

#[test]
/// Tests that if you try to unlink a directory, you get an error and a notification
fn test_non_rm_delete_dir() -> TestResult {
    let th = TestHelper::new(None);
    th.mkdir("t1")?;

    let target = th.mountpoint_path(&["t1"]);

    let mut listener = th
        .notifier
        .lock()
        .listener()
        .expect("Couldn't get listener");
    let idx = listener.marker();

    match std::fs::remove_dir_all(&target) {
        Err(e) if e.kind() == std::io::ErrorKind::Other => {}
        Err(e) => panic!("Wrong error {:?}", e),
        Ok(_) => panic!("Should have had an error"),
    }

    th.assert_note(
        &mut listener,
        idx,
        &[&Note::Unlink(target)],
        Duration::from_secs(3),
    );

    Ok(())
}
