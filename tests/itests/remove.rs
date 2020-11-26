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
use crate::common::{make_unlink_name, OpMode};
use std::rc::Rc;

#[test]
fn test_remove_symlink_single_tag_cli() -> TestResult {
    let th = TestHelper::new(None);
    _test_remove_symlink_single_tag(th)
}

#[test]
fn test_remove_symlink_single_tag_manual() -> TestResult {
    let mut th = TestHelper::new(None);
    th.rm_mode = OpMode::MANUAL;
    th.rmdir_mode = OpMode::MANUAL;
    _test_remove_symlink_single_tag(th)
}

/// Removes a symlink from a single tag directory, very basic
fn _test_remove_symlink_single_tag(th: TestHelper) -> TestResult {
    let linked = th.ln(&["t1"])?;
    th.assert_count(&["t1"], 1);
    let supertag_path = linked.link_filedir_path(&["t1"], false);

    th.assert_path_exists(&supertag_path);
    th.rm(&supertag_path)?;
    th.assert_path_not_exists(&supertag_path);
    th.assert_count(&["t1"], 0);

    Ok(())
}

#[test]
fn test_remove_symlink_multitag_cli() -> TestResult {
    let th = TestHelper::new(None);
    _test_remove_symlink_multitag(th)
}

#[test]
fn test_remove_symlink_multitag_manual() -> TestResult {
    let mut th = TestHelper::new(None);
    th.rm_mode = OpMode::MANUAL;
    th.rmdir_mode = OpMode::MANUAL;
    _test_remove_symlink_multitag(th)
}

/// Tests that removing a single file from one tagdir doesn't remove it from another
fn _test_remove_symlink_multitag(th: TestHelper) -> TestResult {
    let linked = th.ln(&["t1", "t2"])?;
    let t1_path = linked.link_filedir_path(&["t1"], false);
    let t2_path = linked.link_filedir_path(&["t2"], false);

    th.assert_count(&["t1"], 1);
    th.assert_count(&["t2"], 1);
    th.assert_count(&["t1", "t2"], 1);

    th.assert_path_exists(&t1_path);
    th.assert_path_exists(&t2_path);

    th.rm(&t1_path)?;

    th.assert_path_not_exists(&t1_path);
    th.assert_path_exists(&t2_path);

    th.assert_count(&["t1"], 0);
    th.assert_count(&["t2"], 1);

    Ok(())
}

#[test]
fn test_rm_tagdir_cli() -> TestResult {
    let th = TestHelper::new(None);
    _test_rm_tagdir(th)
}

#[test]
fn test_rm_tagdir_manual() -> TestResult {
    let mut th = TestHelper::new(None);
    th.symlink_mode = OpMode::MANUAL;
    th.rm_mode = OpMode::MANUAL;
    th.rmdir_mode = OpMode::MANUAL;
    _test_rm_tagdir(th)
}

#[test]
#[cfg(target_os = "macos")]
fn test_rm_tagdir_finder() -> TestResult {
    let mut th = TestHelper::new(None);
    th.symlink_mode = OpMode::FINDER;
    th.rm_mode = OpMode::FINDER;
    th.rmdir_mode = OpMode::FINDER;
    _test_rm_tagdir(th)
}

/// Tests removing a leaf tag removes the tag from all of the files in the path intersection
fn _test_rm_tagdir(th: TestHelper) -> TestResult {
    // two files, with the same tags, so removing one doesn't alter the tag hierarchy
    let linked1 = th.ln(&["t1", "t2", "t3", "t4"])?;
    let _linked2 = th.ln(&["t1", "t2", "t3", "t4"])?;

    th.assert_count(&["t1", "t2", "t4"], 2);

    th.rmdir(&["t1", "t2", "t4"])?;

    th.mountpoint_path(&["t1"]).metadata().unwrap();
    th.assert_parts_exists(&["t1"]);

    // these intersections should still exist
    th.assert_parts_exists(&["t1", "t2", "t3"]);
    th.assert_path_exists(linked1.link_filedir_path(&["t1", "t2", "t3"], false));
    th.assert_path_exists(linked1.link_filedir_path(&["t1"], false));

    // but not these
    th.assert_parts_not_exists(&["t1", "t4"]);
    th.assert_parts_not_exists(&["t4", "t1", "t4"]);
    th.assert_path_not_exists(linked1.link_filedir_path(&["t2", "t3", "t4"], false));

    th.assert_count(&["t1", "t2", "t3"], 2);
    th.assert_count(&["t3", "t4"], 0);
    th.assert_count(&["t4"], 0);
    th.assert_count(&["t1", "t2", "t4"], 0);

    Ok(())
}

#[test]
fn test_top_level_rm_tagdir_cli() -> TestResult {
    let th = TestHelper::new(None);
    _test_top_level_rm_tagdir(th)
}

#[test]
fn test_top_level_rm_tagdir_manual() -> TestResult {
    let mut th = TestHelper::new(None);
    th.rm_mode = OpMode::MANUAL;
    th.rmdir_mode = OpMode::MANUAL;
    _test_top_level_rm_tagdir(th)
}

fn _test_top_level_rm_tagdir(th: TestHelper) -> TestResult {
    let _linked1 = th.ln(&["t1", "t2", "t3", "t4"])?;

    th.assert_parts_exists(&["t1"]);

    th.rmdir(&["t1"])?;

    th.assert_parts_not_exists(&["t1"]);

    Ok(())
}

#[test]
fn test_delete_tag_cascade_cli() -> TestResult {
    let th = TestHelper::new(None);
    _test_delete_tag_cascade(th)
}

#[test]
fn test_delete_tag_cascade_manual() -> TestResult {
    let mut th = TestHelper::new(None);
    th.rmdir_mode = OpMode::MANUAL;
    _test_delete_tag_cascade(th)
}

fn _test_delete_tag_cascade(th: TestHelper) -> TestResult {
    let l1 = th.ln(&["t1"])?;
    th.rmdir(&["t1"])?;

    // bring back the t1 directory
    let _l2 = th.ln(&["t1"])?;

    // and check that the old linked file doesn't exist there, proving that the file association
    // was cascade-deleted
    th.assert_path_not_exists(l1.link_filedir_path(&["t1"], false));
    Ok(())
}

#[test]
fn test_remove_collision_cli() -> TestResult {
    let th = TestHelper::new(None);
    _test_remove_collision(th)
}

#[test]
fn test_remove_collision_manual() -> TestResult {
    let mut th = TestHelper::new(None);
    th.rmdir_mode = OpMode::MANUAL;
    _test_remove_collision(th)
}

fn _test_remove_collision(th: TestHelper) -> TestResult {
    let td1 = tempfile::TempDir::new()?;
    let td2 = tempfile::TempDir::new()?;
    let mut b1 = tempfile::Builder::new();
    let to_link1 = Rc::new(
        b1.prefix("collision")
            .rand_bytes(0)
            .tempfile_in(td1.path())?,
    );
    let mut b2 = tempfile::Builder::new();
    let to_link2 = Rc::new(
        b2.prefix("collision")
            .rand_bytes(0)
            .tempfile_in(td2.path())?,
    );

    let linked1 = th.ln_with_tempfile(to_link1.clone(), &["t1"])?;
    let linked2 = th.ln_with_tempfile(to_link2.clone(), &["t1"])?;
    // colliding paths have the full inodify name
    th.assert_path_exists(linked1.link_filedir_path(&["t1"], true));
    th.assert_path_exists(linked2.link_filedir_path(&["t1"], true));

    // but if we remove one...
    th.rm(&linked1.link_filedir_path(&["t1"], true))?;

    // and confirm it is deleted
    th.assert_path_not_exists(linked1.link_filedir_path(&["t1"], true));
    // we can't do the commented out check because even though it says `linked1`, it will be checking for the name
    // "collision" which actually *does* exist now (from `linked2`), so this check will fail
    //th.assert_path_not_exists(linked1.link_filedir_path(&["t1"], false));

    // then the remaining file should go back to a regular name
    th.assert_path_exists(linked2.link_filedir_path(&["t1"], false));

    // the device-file version should *always* exist via getattr, but not readdir
    assert!(th.getattr_exists(linked2.link_filedir_path(&["t1"], true)));
    assert!(!th.readdir_exists(linked2.link_filedir_path(&["t1"], true)));
    Ok(())
}

#[test]
fn test_remove_and_relink_file_cli() -> TestResult {
    let th = TestHelper::new(None);
    _test_remove_and_relink_file(th)
}

#[test]
fn test_remove_and_relink_file_manual() -> TestResult {
    let mut th = TestHelper::new(None);
    th.symlink_mode = OpMode::MANUAL;
    th.rm_mode = OpMode::MANUAL;
    _test_remove_and_relink_file(th)
}

#[test]
#[cfg(target_os = "macos")]
fn test_remove_and_relink_file_finder() -> TestResult {
    let mut th = TestHelper::new(None);
    th.symlink_mode = OpMode::FINDER;
    th.rm_mode = OpMode::FINDER;
    _test_remove_and_relink_file(th)
}

/// Tests that removing a symlink and then relinking it works fine
fn _test_remove_and_relink_file(th: TestHelper) -> TestResult {
    let l1 = th.ln(&["t1"])?;
    th.assert_path_exists(l1.link_filedir_path(&["t1"], false));

    th.rm(&l1.link_filedir_path(&["t1"], false))?;
    th.assert_path_not_exists(l1.link_filedir_path(&["t1"], false));

    th.ln_with_file(&l1.target_path(), &["t1"])?;
    th.assert_path_exists(l1.link_filedir_path(&["t1"], false));

    Ok(())
}

#[test]
fn test_remove_and_relink_tag_cli() -> TestResult {
    let th = TestHelper::new(None);
    _test_remove_and_relink_tag(th)
}

#[test]
fn test_remove_and_relink_tag_manual() -> TestResult {
    let mut th = TestHelper::new(None);
    th.symlink_mode = OpMode::MANUAL;
    th.rmdir_mode = OpMode::MANUAL;
    _test_remove_and_relink_tag(th)
}

#[test]
#[cfg(target_os = "macos")]
fn test_remove_and_relink_tag_finder() -> TestResult {
    let mut th = TestHelper::new(None);
    th.symlink_mode = OpMode::FINDER;
    th.rmdir_mode = OpMode::FINDER;
    _test_remove_and_relink_tag(th)
}

/// Tests that a tag removed and then re-created has its rm_time reset
fn _test_remove_and_relink_tag(th: TestHelper) -> TestResult {
    let _l1 = th.ln(&["t1"])?;

    th.rmdir(&["t1"])?;

    th.assert_parts_not_exists(&["t1"]);

    // bring back the t1 directory
    let _l2 = th.ln(&["t1"])?;
    th.assert_parts_exists(&["t1"]);

    th.rmdir(&["t1"])?;

    th.assert_parts_not_exists(&["t1"]);

    // bring it back with a mkdir
    th.mkdir("t1")?;
    th.assert_parts_exists(&["t1"]);

    Ok(())
}

#[test]
fn test_unlinked_stays_unlinked_cli() -> TestResult {
    let th = TestHelper::new(None);
    _test_unlinked_stays_unlinked(th)
}

#[test]
fn test_unlinked_stays_unlinked_manual() -> TestResult {
    let mut th = TestHelper::new(None);
    th.symlink_mode = OpMode::MANUAL;
    th.rmdir_mode = OpMode::MANUAL;
    _test_unlinked_stays_unlinked(th)
}

#[test]
#[cfg(target_os = "macos")]
fn test_unlinked_stays_unlinked_finder() -> TestResult {
    let mut th = TestHelper::new(None);
    th.symlink_mode = OpMode::FINDER;
    th.rmdir_mode = OpMode::FINDER;
    _test_unlinked_stays_unlinked(th)
}

/// Tests that a file association to a tag stays unlinked when the tag is deleted even if the file becomes linked to a
/// new tag
fn _test_unlinked_stays_unlinked(th: TestHelper) -> TestResult {
    let l1 = th.ln(&["t1"])?;
    th.assert_path_exists(l1.link_filedir_path(&["t1"], false));

    th.rmdir(&["t1"])?;

    th.assert_path_not_exists(l1.link_filedir_path(&["t1"], false));
    th.assert_parts_not_exists(&["t1"]);

    th.ln_with_file(&l1.target_path(), &["t2"])?;
    th.assert_path_exists(l1.link_filedir_path(&["t2"], false));
    th.assert_parts_not_exists(&["t2", "t1"]);

    Ok(())
}

#[test]
fn test_remove_tag_immediately_cli() -> TestResult {
    let th = TestHelper::new(None);
    _test_remove_tag_immediately(th)
}

#[test]
fn test_remove_tag_immediately_manual() -> TestResult {
    let mut th = TestHelper::new(None);
    th.symlink_mode = OpMode::MANUAL;
    th.rmdir_mode = OpMode::MANUAL;
    _test_remove_tag_immediately(th)
}

#[test]
#[cfg(target_os = "macos")]
fn test_remove_tag_immediately_finder() -> TestResult {
    let mut th = TestHelper::new(None);
    th.symlink_mode = OpMode::FINDER;
    th.rmdir_mode = OpMode::FINDER;
    _test_remove_tag_immediately(th)
}

/// Tests that when we're removing a top-level tag, instead of doing a pending delete (with rm_time), it does an
/// immediate delete, so that a file browser GUI reflects the changes immediately
fn _test_remove_tag_immediately(th: TestHelper) -> TestResult {
    let _l1 = th.ln(&["t1"])?;
    th.rmdir(&["t1"])?;
    th.assert_parts_not_exists(&["t1"]);
    Ok(())
}

#[test]
fn test_only_remove_leaf_symlink_cli() -> TestResult {
    let th = TestHelper::new(None);
    _test_only_remove_leaf_symlink(th)
}

#[test]
fn test_only_remove_leaf_symlink_manual() -> TestResult {
    let mut th = TestHelper::new(None);
    th.symlink_mode = OpMode::MANUAL;
    th.rm_mode = OpMode::MANUAL;
    _test_only_remove_leaf_symlink(th)
}

#[test]
#[cfg(target_os = "macos")]
fn test_only_remove_leaf_symlink_finder() -> TestResult {
    let mut th = TestHelper::new(None);
    th.symlink_mode = OpMode::FINDER;
    th.rm_mode = OpMode::FINDER;
    _test_only_remove_leaf_symlink(th)
}

/// Tests that removing a symlink from multiple tags only removes it from the last tag, not all of the tags
fn _test_only_remove_leaf_symlink(th: TestHelper) -> TestResult {
    let l1 = th.ln(&["t1", "t2", "t3"])?;

    th.assert_path_exists(l1.link_filedir_path(&["t1", "t3", "t2"], false));
    th.rm(&l1.link_filedir_path(&["t1", "t3", "t2"], false))?;

    th.assert_path_not_exists(l1.link_filedir_path(&["t1", "t3", "t2"], false));
    th.assert_path_exists(l1.link_filedir_path(&["t1", "t3"], false));
    th.assert_path_exists(l1.link_filedir_path(&["t3", "t1"], false));
    th.assert_path_exists(l1.link_filedir_path(&["t1"], false));
    th.assert_path_exists(l1.link_filedir_path(&["t3"], false));
    Ok(())
}

#[ignore]
#[test]
/// Tests that a file deleted through a gui succeeds. This doesn't actually work as a test, because the exists()/stat
/// call fails by the time we call it, but it works in practice
fn test_rename_delete_stat() -> TestResult {
    let mut th = TestHelper::new(None);
    th.rmdir_mode = OpMode::MANUAL;
    th.mkdir("t1")?;
    th.rmdir(&["t1"])?;

    let check = make_unlink_name(th.mountpoint_path(&["t1"]));
    println!("\n{:?}\n", check);
    assert!(check.exists());

    Ok(())
}
