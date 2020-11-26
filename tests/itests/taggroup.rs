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
use std::fs;
use std::time::Duration;
use supertag::common::notify::{Listener, Notifier};
use supertag::common::types::note::Note;

#[test]
fn test_rename_tag_group_cli() -> TestResult {
    let th = TestHelper::new(None);
    _test_rename_tag_group(th)
}

#[test]
fn test_rename_tag_group_manual() -> TestResult {
    let mut th = TestHelper::new(None);
    th.mkdir_mode = OpMode::MANUAL;
    th.rename_mode = OpMode::MANUAL;
    _test_rename_tag_group(th)
}

fn _test_rename_tag_group(th: TestHelper) -> TestResult {
    th.mkdir("a_tags+")?;
    th.assert_parts_exists(&["a_tags+"]);
    th.mv(
        &th.mountpoint_path(&["a_tags+"]),
        &th.mountpoint_path(&["b_tags+"]),
    )?;
    th.assert_parts_exists(&["b_tags+"]);
    th.assert_parts_not_exists(&["a_tags+"]);

    Ok(())
}

#[test]
fn test_pin_tag_group() -> TestResult {
    let th = TestHelper::new(None);

    fs::create_dir(&th.mountpoint_path(&["a_tags+"]))?;
    fs::create_dir(&th.mountpoint_path(&["a_tags+", "a1"]))?;

    th.assert_parts_exists(&["a_tags+", "a1"]);
    Ok(())
}

#[test]
fn test_tag_group_basic_cli() -> TestResult {
    let th = TestHelper::new(None);
    _test_tag_group_basic(th)
}

#[test]
fn test_tag_group_basic_manual() -> TestResult {
    let mut th = TestHelper::new(None);
    th.mkdir_mode = OpMode::MANUAL;
    th.rename_mode = OpMode::MANUAL;
    _test_tag_group_basic(th)
}

fn _test_tag_group_basic(th: TestHelper) -> TestResult {
    th.mkdir("a_tags+")?;
    let _l1 = th.ln(&["a1"])?;
    let _l2 = th.ln(&["b1"])?;

    th.mv(
        &th.mountpoint_path(&["a1"]),
        &th.mountpoint_path(&["a_tags+"]),
    )?;

    // TODO ideally we would fill out th.assert_exact which can take an exact layout, and we wouldn't need to test
    // things like this, since it would assert that only a specific layout spec was present
    th.assert_parts_not_exists(&["a_tags+", "a_tags+"]);
    th.assert_parts_not_exists(&["a_tags+", "a1", "a_tags+"]);

    // this should only exist via getattr, not readdir, since the purpose of a tag group is to hide the tag when a
    // directory is listed
    assert!(th.getattr_exists(th.mountpoint_path(&["a1"])));
    assert!(!th.readdir_exists(th.mountpoint_path(&["a1"])));

    assert!(th.getattr_exists(th.mountpoint_path(&["a_tags+", "a1"])));
    th.assert_path_not_exists(th.filedir_path(&["a_tags+"]));
    th.assert_path_exists(th.filedir_path(&["a_tags+", "a1"]));
    assert!(!th.getattr_exists(th.mountpoint_path(&["a_tags+", "b1"])));

    // this was never put into a group
    th.assert_parts_exists(&["b1"]);

    Ok(())
}

#[test]
fn test_tag_group_no_crossover_manual() -> TestResult {
    let mut th = TestHelper::new(None);
    th.mkdir_mode = OpMode::MANUAL;
    _test_tag_group_no_crossover(th)
}

#[test]
fn test_tag_group_no_crossover_cli() -> TestResult {
    let th = TestHelper::new(None);
    _test_tag_group_no_crossover(th)
}

fn _test_tag_group_no_crossover(th: TestHelper) -> TestResult {
    let _l1 = th.ln(&["a1"])?;
    let _l2 = th.ln(&["a2"])?;

    th.mkdir("a_tags+")?;

    th.mv(
        &th.mountpoint_path(&["a1"]),
        &th.mountpoint_path(&["a_tags+"]),
    )?;
    th.mv(
        &th.mountpoint_path(&["a2"]),
        &th.mountpoint_path(&["a_tags+"]),
    )?;

    let _l3 = th.ln(&["b1"])?;
    let _l4 = th.ln(&["b2"])?;

    th.mkdir("b_tags+")?;
    th.mv(
        &th.mountpoint_path(&["b1"]),
        &th.mountpoint_path(&["b_tags+"]),
    )?;
    th.mv(
        &th.mountpoint_path(&["b2"]),
        &th.mountpoint_path(&["b_tags+"]),
    )?;

    th.assert_parts_exists(&["a_tags+", "a1"]);
    th.assert_parts_exists(&["a_tags+", "a2"]);

    th.assert_parts_not_exists(&["a_tags+", "b1"]);
    th.assert_parts_not_exists(&["a_tags+", "b2"]);

    th.assert_parts_exists(&["b_tags+", "b1"]);
    th.assert_parts_exists(&["b_tags+", "b2"]);

    th.assert_parts_not_exists(&["b_tags+", "a1"]);
    th.assert_parts_not_exists(&["b_tags+", "a2"]);

    Ok(())
}

#[test]
fn test_tag_group_gui_move_cli() -> TestResult {
    let th = TestHelper::new(None);
    _test_tag_group_gui_move(th)
}

#[test]
fn test_tag_group_gui_move_manual() -> TestResult {
    let mut th = TestHelper::new(None);
    th.mkdir_mode = OpMode::MANUAL;
    th.rename_mode = OpMode::MANUAL;
    _test_tag_group_gui_move(th)
}

fn _test_tag_group_gui_move(th: TestHelper) -> TestResult {
    th.mkdir("a_tags+")?;
    let _l1 = th.ln(&["a1"])?;
    let _l2 = th.ln(&["b1"])?;

    th.mv(
        &th.mountpoint_path(&["a1"]),
        &th.mountpoint_path(&["a_tags+", "a1"]),
    )?;

    // this should only exist via getattr, not readdir, since the purpose of a tag group is to hide the tag when a
    // directory is listed
    assert!(th.getattr_exists(th.mountpoint_path(&["a1"])));
    assert!(!th.readdir_exists(th.mountpoint_path(&["a1"])));

    assert!(th.getattr_exists(th.mountpoint_path(&["a_tags+", "a1"])));
    th.assert_path_not_exists(th.filedir_path(&["a_tags+"]));
    th.assert_path_exists(th.filedir_path(&["a_tags+", "a1"]));
    assert!(!th.getattr_exists(th.mountpoint_path(&["a_tags+", "b1"])));

    // this was never put into a group
    th.assert_parts_exists(&["b1"]);

    // these just shouldn't exist
    th.assert_parts_not_exists(&["a_tags+", "a_tags+"]);
    th.assert_parts_not_exists(&["a_tags+", "a1", "a_tags+"]);
    Ok(())
}

#[test]
fn test_file_in_tagdir_and_taggroup_cli() -> TestResult {
    let th = TestHelper::new(None);
    _test_file_in_tagdir_and_taggroup(th)
}

#[test]
fn test_file_in_tagdir_and_taggroup_manual() -> TestResult {
    let mut th = TestHelper::new(None);
    th.mkdir_mode = OpMode::MANUAL;
    th.rename_mode = OpMode::MANUAL;
    th.symlink_mode = OpMode::MANUAL;
    _test_file_in_tagdir_and_taggroup(th)
}

fn _test_file_in_tagdir_and_taggroup(th: TestHelper) -> TestResult {
    th.mkdir("a_tags+")?;
    let _l1 = th.ln(&["a1", "b1"])?;

    th.assert_parts_exists(&["b1", "a1"]);

    th.mv(
        &th.mountpoint_path(&["a1"]),
        &th.mountpoint_path(&["a_tags+"]),
    )?;

    th.assert_parts_exists(&["b1", "a_tags+"]);
    Ok(())
}

#[test]
fn test_merge_to_tag_under_taggroup() -> TestResult {
    let th = TestHelper::new(None);
    th.mkdir("b_tags+")?;
    let l1 = th.ln(&["a1", "b1", "b2"])?;

    th.mv(
        &th.mountpoint_path(&["b2"]),
        &th.mountpoint_path(&["a1", "b_tags+"]),
    )?;

    th.assert_parts_exists(&["a1", "b_tags+", "b2"]);
    th.assert_path_exists(l1.link_filedir_path(&["a1", "b_tags+", "b2"], false));
    assert!(th.getattr_exists(l1.link_filedir_path(&["a1", "b_tags+", "b2"], false)));
    th.assert_parts_not_exists(&["b_tags+", "a1"]);

    Ok(())
}

#[test]
fn test_move_empty_tag_should_pin() -> TestResult {
    let th = TestHelper::new(None);
    th.mkdir("a_tags+")?;
    th.mkdir("a1")?;

    th.mv(
        &th.mountpoint_path(&["a1"]),
        &th.mountpoint_path(&["a_tags+"]),
    )?;

    assert!(th.getattr_exists(th.mountpoint_path(&["a1"])));
    th.assert_parts_exists(&["a_tags+", "a1"]);

    Ok(())
}

#[test]
fn test_move_from_old_tg_to_new_tg_cli() -> TestResult {
    let th = TestHelper::new(None);
    _test_move_from_old_tg_to_new_tg(th)
}

#[test]
fn test_move_from_old_tg_to_new_tg_manual() -> TestResult {
    let mut th = TestHelper::new(None);
    th.symlink_mode = OpMode::MANUAL;
    th.mkdir_mode = OpMode::MANUAL;
    th.rename_mode = OpMode::MANUAL;
    _test_move_from_old_tg_to_new_tg(th)
}

fn _test_move_from_old_tg_to_new_tg(th: TestHelper) -> TestResult {
    let _l1 = th.ln(&["a1", "a2"])?;
    th.mkdir("a_tags+")?;
    th.mv(
        &th.mountpoint_path(&["a1"]),
        &th.mountpoint_path(&["a_tags+"]),
    )?;

    th.mkdir("other_tags+")?;
    th.mv(
        &th.mountpoint_path(&["a_tags+", "a1"]),
        &th.mountpoint_path(&["other_tags+"]),
    )?;

    th.assert_parts_exists(&["other_tags+", "a1"]);

    Ok(())
}

#[test]
/// Tests that we can leave off the special taggroup character and it will be fine
fn test_rename_tg_minus_char() -> TestResult {
    let th = TestHelper::new(None);

    th.mkdir("a_tags+")?;
    th.assert_parts_exists(&["a_tags+"]);
    th.mv(
        &th.mountpoint_path(&["a_tags+"]),
        &th.mountpoint_path(&["b_tags"]),
    )?;
    th.assert_parts_exists(&["b_tags+"]);
    th.assert_parts_not_exists(&["a_tags+"]);
    Ok(())
}

#[test]
fn test_delete_tg_cli() -> TestResult {
    let th = TestHelper::new(None);
    _test_delete_tg(th)
}

#[test]
fn test_delete_tg_manual() -> TestResult {
    let mut th = TestHelper::new(None);
    th.rmdir_mode = OpMode::MANUAL;
    _test_delete_tg(th)
}

#[test]
#[cfg(target_os = "macos")]
fn test_delete_tg_finder() -> TestResult {
    let mut th = TestHelper::new(None);
    th.rmdir_mode = OpMode::FINDER;
    _test_delete_tg(th)
}

fn _test_delete_tg(th: TestHelper) -> TestResult {
    th.mkdir("a_tags+")?;
    let _l1 = th.ln(&["a1"])?;

    th.mv(
        &th.mountpoint_path(&["a1"]),
        &th.mountpoint_path(&["a_tags+"]),
    )?;

    th.assert_parts_exists(&["a_tags+"]);
    th.assert_parts_exists(&["a_tags+", "a1"]);

    th.rmdir(&["a_tags+"])?;

    th.assert_parts_not_exists(&["a_tags+"]);
    th.assert_parts_exists(&["a1"]);

    Ok(())
}

#[test]
/// If you delete a tag group that is nested under some tags, we shouldn't remove the tag group totally, only remove
/// from it the tags nested under it
fn test_delete_tg_limited_tags() -> TestResult {
    let th = TestHelper::new(None);
    th.mkdir("a_tags+")?;

    let _l1 = th.ln(&["a1"])?;
    let _l2 = th.ln(&["a2", "a1"])?;
    let _l3 = th.ln(&["b1"])?;

    th.mv(
        &th.mountpoint_path(&["a1"]),
        &th.mountpoint_path(&["a_tags+"]),
    )?;

    th.mv(
        &th.mountpoint_path(&["a2"]),
        &th.mountpoint_path(&["a_tags+"]),
    )?;

    th.rmdir(&["a2", "a_tags+"])?;

    th.assert_parts_exists(&["a_tags+"]);
    th.assert_parts_exists(&["a_tags+", "a2"]);
    th.assert_parts_not_exists(&["a_tags+", "a1"]);

    Ok(())
}

#[test]
fn test_rename_nonempty_tag_to_tg_cli() -> TestResult {
    let th = TestHelper::new(None);
    _test_rename_nonempty_tag_to_tg(th)
}

#[test]
fn test_rename_nonempty_tag_to_tg_manual() -> TestResult {
    let mut th = TestHelper::new(None);
    th.symlink_mode = OpMode::MANUAL;
    th.mkdir_mode = OpMode::MANUAL;
    th.rename_mode = OpMode::MANUAL;
    _test_rename_nonempty_tag_to_tg(th)
}

#[test]
#[cfg(target_os = "macos")]
fn test_rename_nonempty_tag_to_tg_finder() -> TestResult {
    let mut th = TestHelper::new(None);
    th.symlink_mode = OpMode::FINDER;
    th.mkdir_mode = OpMode::FINDER;
    th.rename_mode = OpMode::FINDER;
    _test_rename_nonempty_tag_to_tg(th)
}

/// If you try to rename a tag to a taggroup after it has already been used to tag files, we'll give a notification
fn _test_rename_nonempty_tag_to_tg(th: TestHelper) -> TestResult {
    th.mkdir("a_tags")?;
    let _l1 = th.ln(&["a_tags"])?;

    let mut listener = th
        .notifier
        .lock()
        .listener()
        .expect("Couldn't get listener");
    let idx = listener.marker();

    match th.mv(
        &th.mountpoint_path(&["a_tags"]),
        &th.mountpoint_path(&["a_tags+"]),
    ) {
        Err(_) => {}
        Ok(_) => panic!("Should have had an error"),
    }

    th.assert_note(
        &mut listener,
        idx,
        &[&Note::TagToTagGroup("a_tags".to_string())],
        Duration::from_secs(3),
    );

    th.assert_parts_exists(&["a_tags"]);
    th.assert_parts_not_exists(&["a_tags+"]);

    Ok(())
}

#[test]
fn test_rename_empty_tag_to_tg_cli() -> TestResult {
    let th = TestHelper::new(None);
    _test_rename_empty_tag_to_tg(th)
}

#[test]
fn test_rename_empty_tag_to_tg_manual() -> TestResult {
    let mut th = TestHelper::new(None);
    th.symlink_mode = OpMode::MANUAL;
    th.mkdir_mode = OpMode::MANUAL;
    th.rename_mode = OpMode::MANUAL;
    _test_rename_empty_tag_to_tg(th)
}

#[test]
#[cfg(target_os = "macos")]
fn test_rename_empty_tag_to_tg_finder() -> TestResult {
    let mut th = TestHelper::new(None);
    th.symlink_mode = OpMode::FINDER;
    th.mkdir_mode = OpMode::FINDER;
    th.rename_mode = OpMode::FINDER;
    _test_rename_empty_tag_to_tg(th)
}

/// If you try to rename a tag to a taggroup after it has already been used to tag files, we'll give a notification
fn _test_rename_empty_tag_to_tg(th: TestHelper) -> TestResult {
    th.mkdir("a_tags")?;

    th.mv(
        &th.mountpoint_path(&["a_tags"]),
        &th.mountpoint_path(&["a_tags+"]),
    )?;

    th.assert_parts_exists(&["a_tags+"]);
    th.assert_parts_not_exists(&["a_tags"]);

    Ok(())
}

#[test]
fn test_nested_tag_group() -> TestResult {
    let th = TestHelper::new(None);
    th.mkdir("a_tags+")?;
    th.mkdir("b_tags+")?;

    let _l1 = th.ln(&["a", "b"])?;
    let _l2 = th.ln(&["a", "b"])?;

    th.mv(
        &th.mountpoint_path(&["a"]),
        &th.mountpoint_path(&["a_tags+"]),
    )?;
    th.mv(
        &th.mountpoint_path(&["b"]),
        &th.mountpoint_path(&["b_tags+"]),
    )?;

    th.assert_parts_exists(&["a_tags+", "a", "b_tags+", "b"]);
    th.assert_parts_exists(&["b_tags+", "b", "a_tags+", "a"]);
    Ok(())
}

#[test]
fn test_tag_group_multiple_cli() -> TestResult {
    let th = TestHelper::new(None);
    _test_tag_group_multiple(th)
}

#[test]
fn test_tag_group_multiple_manual() -> TestResult {
    let mut th = TestHelper::new(None);
    th.mkdir_mode = OpMode::MANUAL;
    th.rename_mode = OpMode::MANUAL;
    _test_tag_group_multiple(th)
}

/// Tests that a tag can live in multiple tag groups at once
fn _test_tag_group_multiple(th: TestHelper) -> TestResult {
    th.mkdir("a_tags+")?;
    th.mkdir("b_tags+")?;

    let _l1 = th.ln(&["ab"])?;

    th.mv(
        &th.mountpoint_path(&["ab"]),
        &th.mountpoint_path(&["a_tags+"]),
    )?;
    th.mv(
        &th.mountpoint_path(&["a_tags+", "ab"]),
        &th.mountpoint_path(&["b_tags+"]),
    )?;

    th.assert_parts_exists(&["a_tags+", "ab"]);
    th.assert_parts_exists(&["b_tags+", "ab"]);

    // this shouldn't happen because "ab" would be in both tag groups, and /ab/ab wouldn't make sense
    th.assert_parts_not_exists(&["b_tags+", "ab", "a_tags+"]);
    th.assert_parts_not_exists(&["b_tags+", "a_tags+"]);
    th.assert_parts_not_exists(&["a_tags+", "ab", "b_tags+"]);
    th.assert_parts_not_exists(&["a_tags+", "b_tags+"]);

    Ok(())
}

#[test]
fn test_tag_group_size() -> TestResult {
    let th = TestHelper::new(None);

    th.mkdir("a_tags+")?;
    th.assert_size(&["a_tags+"], 0);
    let _l1 = th.ln(&["t1"])?;
    th.mv(
        th.mountpoint_path(&["t1"]),
        th.mountpoint_path(&["a_tags+"]),
    )?;

    th.assert_size(&["a_tags+"], 1);
    let _l2 = th.ln(&["t1"])?;

    // FIXME? linking to a tag doesn't flush the tag groups it is a part of
    th.sleep_readdir_cache();
    th.assert_size(&["a_tags+"], 2);

    Ok(())
}

#[test]
fn test_tag_group_nested_size_cli() -> TestResult {
    let th = TestHelper::new(None);
    _test_tag_group_nested_size(th)
}

#[test]
fn test_tag_group_nested_size_manual() -> TestResult {
    let mut th = TestHelper::new(None);
    th.symlink_mode = OpMode::MANUAL;
    th.mkdir_mode = OpMode::MANUAL;
    th.rename_mode = OpMode::MANUAL;
    _test_tag_group_nested_size(th)
}

fn _test_tag_group_nested_size(th: TestHelper) -> TestResult {
    th.mkdir("a_tags+")?;
    let _l1 = th.ln(&["a1", "a2", "a3"])?;
    let _l2 = th.ln(&["a2", "a3", "a4"])?;
    let _l3 = th.ln(&["a3", "a4", "a5"])?;

    for i in 1..=5 {
        let tag = format!("a{}", i);
        th.mv(
            th.mountpoint_path(&[&tag]),
            th.mountpoint_path(&["a_tags+"]),
        )?;
    }

    th.assert_size(&["a_tags+"], 3);
    th.assert_size(&["a_tags+", "a3", "a_tags+"], 3);
    th.assert_size(&["a_tags+", "a4", "a_tags+"], 2);
    th.assert_size(&["a_tags+", "a2", "a_tags+"], 2);
    th.assert_size(&["a_tags+", "a5", "a_tags+"], 1);
    th.assert_size(&["a_tags+", "a1", "a_tags+"], 1);

    Ok(())
}
