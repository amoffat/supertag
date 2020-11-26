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
use supertag::common::err::STagError;

#[test]
fn test_nested_tag_merge_cli() -> TestResult {
    let th = TestHelper::new(None);
    _test_nested_tag_merge(th)
}

#[test]
fn test_nested_tag_merge_manual() -> TestResult {
    let mut th = TestHelper::new(None);
    th.rename_mode = OpMode::MANUAL;
    _test_nested_tag_merge(th)
}

#[test]
#[cfg(target_os = "macos")]
fn test_nested_tag_merge_finder() -> TestResult {
    let mut th = TestHelper::new(None);
    th.rename_mode = OpMode::FINDER;
    _test_nested_tag_merge(th)
}

fn _test_nested_tag_merge(th: TestHelper) -> TestResult {
    let l1 = th.ln(&["t1", "t2"])?;
    let _l2 = th.ln(&["t3", "t4"])?;
    let l3 = th.ln(&["t2"])?;

    th.mv(
        &th.mountpoint_path(&["t2", "t1"]),
        &th.mountpoint_path(&["t3", "t4"]),
    )?;

    th.assert_path_not_exists(l1.link_filedir_path(&["t1"], false));
    th.assert_path_exists(l1.link_filedir_path(&["t3"], false));
    th.assert_path_exists(l1.link_filedir_path(&["t3", "t4"], false));
    th.assert_path_exists(l1.link_filedir_path(&["t4"], false));

    // this proves that we only merged the t1/t2 intersection, but since l3 wasn't in t1, it should
    // be left alone
    th.assert_path_exists(l3.link_filedir_path(&["t2"], false));
    th.assert_path_not_exists(l3.link_filedir_path(&["t3", "t4"], false));

    // even though we've moved all of the tagged files out of t1, we've made a conscious decision
    // not to delete that tag, because there are difficult cache flushing problems, and also because
    // it might not be obvious that a merge results in a delete sometimes (but not always).
    th.assert_parts_exists(&["t1"]);

    // t2 should still exist because l3 is still tagged with it
    th.assert_parts_exists(&["t2"]);
    Ok(())
}

#[test]
fn test_move_tag_merge_cli() -> TestResult {
    let th = TestHelper::new(None);
    _test_move_tag_merge(th)
}
#[test]
fn test_move_tag_merge_manual() -> TestResult {
    let mut th = TestHelper::new(None);
    th.rename_mode = OpMode::MANUAL;
    _test_move_tag_merge(th)
}

#[test]
#[cfg(target_os = "macos")]
fn test_move_tag_merge_finder() -> TestResult {
    let mut th = TestHelper::new(None);
    th.rename_mode = OpMode::FINDER;
    _test_move_tag_merge(th)
}

fn _test_move_tag_merge(th: TestHelper) -> TestResult {
    let l1 = th.ln(&["t1"])?;
    let _l2 = th.ln(&["t2"])?;
    let l3 = th.ln(&["t1", "t2", "t3"])?;

    th.mv(&th.mountpoint_path(&["t1"]), &th.mountpoint_path(&["t2"]))?;

    // ensure t2 still there
    th.assert_parts_exists(&["t2"]);

    // check that l1 is now found under tag t2, demonstrating a merge
    th.assert_path_exists(l1.link_filedir_path(&["t2"], false));

    // t1 should still exist, even though l1 shouldn't exist under it anymore
    th.assert_parts_exists(&["t1"]);
    th.assert_path_not_exists(l1.link_filedir_path(&["t1"], false));

    // this is an important one. it catches a bug where we were removing too many file_tag associations
    th.assert_path_exists(l3.link_filedir_path(&["t3", "t2"], false));

    th.assert_parts_not_exists(&["t1", "t2"]);
    th.assert_parts_not_exists(&["t2", "t1"]);

    Ok(())
}

#[test]
fn test_move_file_misname_cli() -> TestResult {
    let th = TestHelper::new(None);
    _test_move_file_already_exists(th)
}

#[test]
fn test_move_file_misname_manual() -> TestResult {
    let mut th = TestHelper::new(None);
    th.rename_mode = OpMode::MANUAL;
    _test_move_file_already_exists(th)
}

#[test]
#[cfg(target_os = "macos")]
fn test_move_file_misname_finder() -> TestResult {
    let mut th = TestHelper::new(None);
    th.rename_mode = OpMode::FINDER;
    _test_move_file_already_exists(th)
}

/// Tests that a file renamed to also include the special characters doesn't duplicate those
/// special characters.  For example,
//FIXME does this actually do what the comment says?
fn _test_move_file_misname_exists(th: TestHelper) -> TestResult {
    let l1 = th.ln(&["t1"])?;

    let new_dst_filename = l1.new_link_filename("fudge", false);
    let new_dst_path = th.filedir_path(&["t1"]).join(&new_dst_filename);

    th.mv(&l1.link_filedir_path(&["t1"], false), &new_dst_path)?;

    th.assert_path_exists(&new_dst_path);

    Ok(())
}

#[test]
fn test_move_file_already_exists_cli() -> TestResult {
    let th = TestHelper::new(None);
    _test_move_file_already_exists(th)
}

#[test]
fn test_move_file_already_exists_manual() -> TestResult {
    let mut th = TestHelper::new(None);
    th.rename_mode = OpMode::MANUAL;
    _test_move_file_already_exists(th)
}

#[test]
#[cfg(target_os = "macos")]
fn test_move_file_already_exists_finder() -> TestResult {
    let mut th = TestHelper::new(None);
    th.symlink_mode = OpMode::FINDER;
    th.rename_mode = OpMode::FINDER;
    _test_move_file_already_exists(th)
}

fn _test_move_file_already_exists(th: TestHelper) -> TestResult {
    let l1 = th.ln(&["t1"])?;

    let new_dst_filename = l1.new_link_filename("fudge", false);
    let new_dst_path = th.filedir_path(&["t1"]).join(&new_dst_filename);

    th.mv(&l1.link_filedir_path(&["t1"], false), &new_dst_path)?;

    th.assert_path_exists(&new_dst_path);

    Ok(())
}

#[test]
fn test_move_merge_nested_same_name_cli() -> TestResult {
    let th = TestHelper::new(None);
    _test_move_merge_nested_same_name(th)
}

#[test]
fn test_move_merge_nested_same_name_manual() -> TestResult {
    let mut th = TestHelper::new(None);
    th.rename_mode = OpMode::MANUAL;
    _test_move_merge_nested_same_name(th)
}

#[test]
#[cfg(target_os = "macos")]
fn test_move_merge_nested_same_name_finder() -> TestResult {
    let mut th = TestHelper::new(None);
    th.symlink_mode = OpMode::FINDER;
    th.rename_mode = OpMode::FINDER;
    _test_move_merge_nested_same_name(th)
}

// this tests what some file browsers do when you drag/drop-move a tag directory.  they don't
// always do what the cli does, which would be: `mv /t1 /t2`, they do something more like
// `mv /t1 /t2/t1`.  and this can cause supertag to not do a real merge if we're not careful
fn _test_move_merge_nested_same_name(th: TestHelper) -> TestResult {
    let l1 = th.ln(&["t1"])?;
    let _l2 = th.ln(&["t2"])?;

    th.mv(
        &th.mountpoint_path(&["t1"]),
        &th.mountpoint_path(&["t2", "t1"]),
    )?;

    th.assert_path_not_exists(l1.link_filedir_path(&["t1"], false));
    // FIXME do we need to test the merge now?

    Ok(())
}

#[test]
fn test_rename_file_cli() -> TestResult {
    let th = TestHelper::new(None);
    _test_rename_file(th)
}

#[test]
fn test_rename_file_manual() -> TestResult {
    let mut th = TestHelper::new(None);
    th.rename_mode = OpMode::MANUAL;
    _test_rename_file(th)
}

#[test]
#[cfg(target_os = "macos")]
fn test_rename_file_finder() -> TestResult {
    let mut th = TestHelper::new(None);
    th.symlink_mode = OpMode::FINDER;
    th.rename_mode = OpMode::FINDER;
    _test_rename_file(th)
}

fn _test_rename_file(th: TestHelper) -> TestResult {
    let linked = th.ln(&["t1", "t2"])?;

    let new_filename = "some_new_name";
    let dst = th.filedir_path(&["t1"]).join(new_filename);
    th.mv(&linked.link_filedir_path(&["t1"], false), &dst)?;

    // check that our new file exists
    let new_dst_filename = linked.new_link_filename(new_filename, false);
    let new_dst_path = th.filedir_path(&["t1"]).join(&new_dst_filename);
    th.assert_path_exists(new_dst_path);

    // and that our old file doesn't
    th.assert_path_not_exists(linked.link_filedir_path(&["t1"], false));

    Ok(())
}

#[test]
fn test_rename_collision_cli() -> TestResult {
    let th = TestHelper::new(None);
    _test_rename_collision(th)
}

#[test]
fn test_rename_collision_manual() -> TestResult {
    let mut th = TestHelper::new(None);
    th.symlink_mode = OpMode::MANUAL;
    th.rename_mode = OpMode::MANUAL;
    _test_rename_collision(th)
}

#[test]
#[cfg(target_os = "macos")]
fn test_rename_collision_finder() -> TestResult {
    let mut th = TestHelper::new(None);
    th.symlink_mode = OpMode::FINDER;
    th.rename_mode = OpMode::FINDER;
    _test_rename_collision(th)
}

/// Tests that we switch over to "fully-qualified" naming when there exists a file name collision in a tagdir
fn _test_rename_collision(th: TestHelper) -> TestResult {
    let l1 = th.ln(&["t1"])?;
    let l2 = th.ln(&["t1"])?;

    // rename l1 to be the same name as l2
    let collision_name = supertag::common::get_filename(l2.tmp.path())?;
    let new_dst_path = l1.new_link_path(&["t1"], collision_name, false);
    th.mv(&l1.link_filedir_path(&["t1"], false), &new_dst_path)?;

    // l2's simple name should be gone now
    th.assert_path_not_exists(l2.link_filedir_path(&["t1"], false));
    // replaced with only the fully qualified name
    th.assert_path_exists(l2.link_filedir_path(&["t1"], true));
    // and the new l1 simple name should also not exist
    th.assert_path_not_exists(&new_dst_path);
    // replaced with only the fully qualified name
    th.assert_path_exists(l1.new_link_path(&["t1"], collision_name, true));

    Ok(())
}

#[test]
fn test_rename_tag_cli() -> TestResult {
    let th = TestHelper::new(None);
    _test_rename_tag(th)
}

#[test]
fn test_rename_tag_manual() -> TestResult {
    let mut th = TestHelper::new(None);
    th.rename_mode = OpMode::MANUAL;
    _test_rename_tag(th)
}

#[test]
#[cfg(target_os = "macos")]
fn test_rename_tag_finder() -> TestResult {
    let mut th = TestHelper::new(None);
    th.symlink_mode = OpMode::FINDER;
    th.rename_mode = OpMode::FINDER;
    _test_rename_tag(th)
}

/// Tests that we can rename a tagdir.  Very basic.
fn _test_rename_tag(th: TestHelper) -> TestResult {
    let _linked = th.ln(&["t1"])?;
    th.mv(
        &th.mountpoint_path(&["t1"]),
        &th.mountpoint_path(&["new_t1"]),
    )?;

    th.assert_parts_not_exists(&["t1"]);
    th.assert_parts_exists(&["new_t1"]);

    Ok(())
}

// tests that a bogus tag name isn't allowed
#[test]
fn test_rename_tag_invalid_name() -> TestResult {
    let th = TestHelper::new(None);
    let _linked = th.ln(&["t1"])?;

    for invalid in &[&th.settings.get_config().symbols.filedir_str] {
        let src = th.mountpoint_path(&["t1"]);
        let dst = th.mountpoint_path(&[invalid]);

        let res = th.mv(&src, &dst);

        match res {
            Err(STagError::BadTag(_tag)) => (),
            Ok(_) => panic!("Should have raised"),
            Err(e) => Err(e)?,
        }
    }

    th.assert_parts_exists(&["t1"]);

    Ok(())
}
