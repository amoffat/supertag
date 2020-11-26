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

use super::{mtime, mtime_pause, OpMode, TestHelper, TestResult};

#[test]
fn test_tagdir_modify_time_cli() -> TestResult {
    let th = TestHelper::new(None);
    _test_tagdir_modify_time(th)
}

#[test]
fn test_tagdir_modify_time_manual() -> TestResult {
    let mut th = TestHelper::new(None);
    th.symlink_mode = OpMode::MANUAL;
    _test_tagdir_modify_time(th)
}

#[test]
#[cfg(target_os = "macos")]
fn test_tagdir_modify_time_finder() -> TestResult {
    let mut th = TestHelper::new(None);
    th.symlink_mode = OpMode::FINDER;
    _test_tagdir_modify_time(th)
}

/// Tests that our tag directory mtimes get updated when a new file is tagged to them
fn _test_tagdir_modify_time(th: TestHelper) -> TestResult {
    let t1 = th.mountpoint_path(&["t1"]);
    let t1_bar = th.filedir_path(&["t1"]);
    let t2 = th.mountpoint_path(&["t2"]);
    let t2_bar = th.filedir_path(&["t2"]);

    // let's link a file and then get some initial timestamps
    let _ = th.ln(&["t1", "t2"])?;

    let mtime_t1_1 = mtime(&t1);
    let mtime_t1_bar_1 = mtime(&t1_bar);
    let mtime_t2_1 = mtime(&t2);
    let mtime_t2_bar_1 = mtime(&t2_bar);

    mtime_pause();

    // now let's link a new file, but to only one tag directory, and get some new timestamps
    let _ = th.ln(&["t1"])?;

    let mtime_t1_2 = mtime(&t1);
    let mtime_t1_bar_2 = mtime(&t1_bar);
    let mtime_t2_2 = mtime(&t2);
    let mtime_t2_bar_2 = mtime(&t2_bar);

    // the mtime of t1 should have advanced now, because we added linked2 to it
    assert!(
        mtime_t1_2 > mtime_t1_1,
        "tagdir timestamp {:?} wasn't greater than {:?}",
        mtime_t1_2,
        mtime_t1_1
    );
    assert!(
        mtime_t1_bar_2 > mtime_t1_bar_1,
        "filedir timestamp {:?} wasn't greater than {:?}",
        mtime_t1_bar_2,
        mtime_t1_bar_1
    );

    // but mtime of t2 should stay the same, because no knew files have been tagged by it
    assert_eq!(mtime_t2_1, mtime_t2_2);
    assert_eq!(mtime_t2_bar_1, mtime_t2_bar_2);
    assert_eq!(mtime_t2_1, mtime_t2_bar_1);

    Ok(())
}

#[test]
fn test_different_mtimes_for_tag_cli() -> TestResult {
    let th = TestHelper::new(None);
    _test_different_mtimes_for_tag(th)
}

#[test]
fn test_different_mtimes_for_tag_manual() -> TestResult {
    let mut th = TestHelper::new(None);
    th.symlink_mode = OpMode::MANUAL;
    _test_different_mtimes_for_tag(th)
}

#[test]
#[cfg(target_os = "macos")]
fn test_different_mtimes_for_tag_finder() -> TestResult {
    let mut th = TestHelper::new(None);
    th.symlink_mode = OpMode::FINDER;
    _test_different_mtimes_for_tag(th)
}

fn _test_different_mtimes_for_tag(th: TestHelper) -> TestResult {
    let t2t1 = th.mountpoint_path(&["t2", "t1"]);
    let t3t2t1 = th.mountpoint_path(&["t3", "t2", "t1"]);

    let _linked1 = th.ln(&["t1", "t2", "t3"])?;
    mtime_pause();
    let _linked2 = th.ln(&["t1", "t2"])?;

    let mtime_t3t2t1_1 = mtime(&t3t2t1);
    let mtime_t2t1_1 = mtime(&t2t1);
    let mtime_t1_1 = mtime(th.mountpoint_path(&["t1"]));

    // the /t2/t1 tagdir should have a greater mtime than /t3/t2/t1 because a file was linked to
    // it later
    assert!(
        mtime_t2t1_1 > mtime_t3t2t1_1,
        "{:?} wasn't greater than {:?}",
        mtime_t2t1_1,
        mtime_t3t2t1_1
    );

    // however, /t2/t1 should have the same mtime as the top level /t1, because the top level should
    // always be the latest update
    assert_eq!(
        mtime_t2t1_1, mtime_t1_1,
        "/t2/t1 mtime does not equal /t1 mtime"
    );

    Ok(())
}

#[test]
fn test_new_tag_mountdir_mtime_cli() -> TestResult {
    let th = TestHelper::new(None);
    _test_new_tag_mountdir_mtime(th)
}

#[test]
fn test_new_tag_mountdir_mtime_manual() -> TestResult {
    let mut th = TestHelper::new(None);
    th.symlink_mode = OpMode::MANUAL;
    _test_new_tag_mountdir_mtime(th)
}

#[test]
#[cfg(target_os = "macos")]
fn test_new_tag_mountdir_mtime_finder() -> TestResult {
    let mut th = TestHelper::new(None);
    th.symlink_mode = OpMode::FINDER;
    _test_new_tag_mountdir_mtime(th)
}

/// Tests that creating a new tagdir and tagging files updates the mountpoint's mtime
fn _test_new_tag_mountdir_mtime(th: TestHelper) -> TestResult {
    let base_1 = mtime(&th.real_mountpoint());

    mtime_pause();
    let _ = th.ln(&["t1"])?;
    let base_2 = mtime(&th.real_mountpoint());

    assert!(
        base_2 > base_1,
        "{:?} wasn't greater than {:?}",
        base_2,
        base_1
    );

    mtime_pause();
    let _ = th.ln(&["t2"])?;
    let base_3 = mtime(&th.real_mountpoint());

    assert!(
        base_3 > base_2,
        "{:?} wasn't greater than {:?}",
        base_3,
        base_2
    );

    mtime_pause();
    // a nested tag should still update the base, since all the tags are stored at the base
    let _ = th.ln(&["t1", "t2"])?;
    let base_4 = mtime(&th.real_mountpoint());

    assert!(
        base_4 > base_3,
        "{:?} wasn't greater than {:?}",
        base_4,
        base_3
    );

    Ok(())
}

#[test]
fn test_rmdir_mountdir_mtime_cli() -> TestResult {
    let th = TestHelper::new(None);
    _test_rmdir_mountdir_mtime(th)
}

#[test]
fn test_rmdir_mountdir_mtime_manual() -> TestResult {
    let mut th = TestHelper::new(None);
    th.symlink_mode = OpMode::MANUAL;
    _test_rmdir_mountdir_mtime(th)
}

#[test]
#[cfg(target_os = "macos")]
fn test_rmdir_mountdir_mtime_finder() -> TestResult {
    let mut th = TestHelper::new(None);
    th.symlink_mode = OpMode::FINDER;
    _test_rmdir_mountdir_mtime(th)
}

/// Tests that removing a directory updates the mtime
fn _test_rmdir_mountdir_mtime(th: TestHelper) -> TestResult {
    let _linked1 = th.ln(&["t1", "t2"])?;

    let base_1 = mtime(&th.real_mountpoint());

    mtime_pause();
    th.rmdir(&["t1", "t2"])?;
    let base_2 = mtime(&th.real_mountpoint());

    assert!(
        base_2 > base_1,
        "{:?} wasn't greater than {:?}",
        base_2,
        base_1
    );

    Ok(())
}
