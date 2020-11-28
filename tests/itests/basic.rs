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
use std::io::ErrorKind;
#[cfg(target_os = "linux")]
#[cfg(target_os = "macos")]
use std::os::macos::fs::MetadataExt;
use std::rc::Rc;
use supertag::common::err::STagError;
use supertag::common::types::file_perms::UMask;
use tempfile::NamedTempFile;

#[test]
fn test_basic_tagging_cli() -> TestResult {
    let th = TestHelper::new(None);
    _test_basic_tagging(th)
}

#[test]
fn test_basic_tagging_manual() -> TestResult {
    let mut th = TestHelper::new(None);
    th.symlink_mode = OpMode::MANUAL;
    _test_basic_tagging(th)
}

#[test]
#[cfg(target_os = "macos")]
fn test_basic_tagging_finder() -> TestResult {
    let mut th = TestHelper::new(None);
    th.symlink_mode = OpMode::FINDER;
    _test_basic_tagging(th)
}

// tests that some basic file linking works
fn _test_basic_tagging(th: TestHelper) -> TestResult {
    let linked = th.ln(&["t1"])?;

    th.assert_parts_exists(&["t1"]);
    th.assert_path_exists(linked.link_filedir_path(&["t1"], false));
    th.assert_count(&["t1"], 1);

    // this was intended to catch macos which, in a non-test environment, seems to emit bad_copy
    // notes even on legitimate symlinks. however, we haven't been able to make this assertion
    // fail on mac
    th.assert_no_note();
    Ok(())
}

#[test]
fn test_duplicate_names_cli() -> TestResult {
    let th = TestHelper::new(None);
    _test_duplicate_names(th)
}

#[test]
fn test_duplicate_names_manual() -> TestResult {
    let mut th = TestHelper::new(None);
    th.symlink_mode = OpMode::MANUAL;
    _test_duplicate_names(th)
}

#[test]
#[cfg(target_os = "macos")]
fn test_duplicate_names_finder() -> TestResult {
    let mut th = TestHelper::new(None);
    th.symlink_mode = OpMode::FINDER;
    _test_duplicate_names(th)
}

fn _test_duplicate_names(th: TestHelper) -> TestResult {
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

    // as long as only one file exists, there is no collision with the name, and the short name is used
    let linked1 = th.ln_with_tempfile(to_link1.clone(), &["t1"])?;
    th.assert_path_exists(linked1.link_filedir_path(&["t1"], false));

    // although, the long version should always be visible via gettattr
    assert!(th.getattr_exists(linked1.link_filedir_path(&["t1"], true)));
    assert!(!th.readdir_exists(linked1.link_filedir_path(&["t1"], true)));

    // we *have* to sleep here for the test to pass.  it is not intuitive, but there is a very good reason.  it boils
    // down to the fact that there is no good time to clear an alias from the alias cache.  at one point, we were doing
    // it on release (close) of the file, but that turned out not to work because macos sets xattrs later, which are
    // necessary for finder aliases to work, after the file is closed.  but if we've removed the file from the opcache,
    // macos sees that the file doesn't exist (via getattr) and declines to setxattr, causing broken finder aliases.
    // however, we do also need to clear out the aliases at some point, otherwise this test will never pass.  the reason
    // the test would never pass is because macos would refuse to create the second alias file (linked2) because it
    // would see (via getattr) an existing alias file in the same place we want to create this one.  so obviously we
    // need to both not clear out the alias, so that finder does setxattr, but simultaneously clear out the alias,
    // so that we can create colliding files for this test.  the solution is to use a time-based lru cache for the
    // alias cache, and in this test, do this sleep.  this is reasonable because we expect macos to do its finder
    // setxattr business in under a second as part of its creation process, but then after that, we consider the
    // alias cache entry as removed, so that we can create other files with the same name
    std::thread::sleep(std::time::Duration::from_millis(1000));

    // but as soon as two files with the same name exist, we must use the device-file version of the filename to
    // distinguish between them
    let linked2 = th.ln_with_tempfile(to_link2.clone(), &["t1"])?;
    th.assert_path_exists(linked1.link_filedir_path(&["t1"], true));
    th.assert_path_exists(linked2.link_filedir_path(&["t1"], true));
    th.assert_path_not_exists(linked1.link_filedir_path(&["t1"], false));
    th.assert_path_not_exists(linked2.link_filedir_path(&["t1"], false));

    Ok(())
}

#[test]
fn test_disable_recursive_symlinks_cli() -> TestResult {
    let th = TestHelper::new(None);
    let _l1 = th.ln(&["a1"])?;
    let _l2 = th.ln(&["a2"])?;

    let mut cmd_conn = th.fresh_conn();
    let notifier = th.notifier.lock();
    match supertag::ln(
        &th.settings,
        &mut cmd_conn,
        th.real_mountpoint(),
        vec![&th.mountpoint_path(&["a2"])],
        &th.mountpoint_path(&["a1", "a2"]),
        th.uid,
        th.gid,
        &UMask::default(),
        &*notifier,
    ) {
        Err(STagError::RecursiveLink(src)) if src == th.mountpoint_path(&["a2"]) => Ok(()),
        Err(e) => panic!("Had wrong error {}", e),
        Ok(()) => panic!("Should have had error"),
    }
}

#[test]
fn test_disable_recursive_symlinks_manual() -> TestResult {
    let mut th = TestHelper::new(None);
    th.symlink_mode = OpMode::MANUAL;
    let _l1 = th.ln(&["a1"])?;
    let _l2 = th.ln(&["a2"])?;

    match std::os::unix::fs::symlink(
        th.mountpoint_path(&["a2"]),
        th.mountpoint_path(&["a1", "a2"]),
    ) {
        Err(e) => match e.kind() {
            ErrorKind::Other => Ok(()),
            _ => panic!("Wrong error {:?}", e),
        },
        _ => panic!("Should have had error"),
    }
}

#[test]
fn test_tag_intersection_cli() -> TestResult {
    let th = TestHelper::new(None);
    _test_tag_intersection(th)
}

#[test]
fn test_tag_intersection_manual() -> TestResult {
    let mut th = TestHelper::new(None);
    th.symlink_mode = OpMode::MANUAL;
    _test_tag_intersection(th)
}

#[test]
#[cfg(target_os = "macos")]
fn test_tag_intersection_finder() -> TestResult {
    let mut th = TestHelper::new(None);
    th.symlink_mode = OpMode::FINDER;
    _test_tag_intersection(th)
}

// tests that a file made with more than one tag lives in multiple tag directories, at the
// intersection
fn _test_tag_intersection(th: TestHelper) -> TestResult {
    let linked = th.ln(&["t1", "t2"])?;

    // check that the tag directories exist
    th.assert_parts_exists(&["t1"]);
    th.assert_parts_exists(&["t2"]);

    // and that each tag has each of the others in the subdirs
    th.assert_parts_exists(&["t1", "t2"]);
    th.assert_parts_exists(&["t2", "t1"]);

    th.assert_path_exists(linked.link_filedir_path(&["t1"], false));
    th.assert_path_exists(linked.link_filedir_path(&["t2"], false));
    th.assert_path_exists(linked.link_filedir_path(&["t1", "t2"], false));
    th.assert_path_exists(linked.link_filedir_path(&["t2", "t1"], false));

    th.assert_count(&["t1"], 1);
    th.assert_count(&["t2"], 1);
    th.assert_count(&["t1", "t2"], 1);

    Ok(())
}

#[test]
fn test_funky_name() -> TestResult {
    let th = TestHelper::new(None);

    let mut builder = tempfile::Builder::new();
    builder
        .prefix(r"[!supertag-testfile")
        .suffix(".tmp")
        .rand_bytes(8);
    let linked = th.ln_with_tempfile(Rc::new(builder.tempfile()?), &["t1"])?;

    th.assert_parts_exists(&["t1"]);
    th.assert_path_exists(linked.link_filedir_path(&["t1"], false));

    Ok(())
}

// tests that files tagged with some intersecting tags, and some non-intersecting tags, creates the
// appropriate directories
#[test]
fn test_tag_difference() -> TestResult {
    let th = TestHelper::new(None);

    let linked1 = th.ln(&["t1"])?;

    let linked2 = th.ln(&["t2"])?;
    let linked3 = th.ln(&["t1", "t2"])?;
    let linked4 = th.ln(&["t1", "t3"])?;

    // and that each tag has each of the others in the subdirs
    th.assert_parts_exists(&["t1", "t2"]);
    th.assert_parts_exists(&["t1", "t3"]);
    th.assert_parts_exists(&["t2", "t1"]);
    th.assert_parts_exists(&["t3", "t1"]);

    // these two share no common files though
    th.assert_parts_not_exists(&["t2", "t3"]);
    th.assert_parts_not_exists(&["t3", "t2"]);

    th.assert_path_exists(linked1.link_filedir_path(&["t1"], false));
    th.assert_path_not_exists(linked1.link_filedir_path(&["t2"], false));

    th.assert_path_exists(linked2.link_filedir_path(&["t2"], false));
    th.assert_path_not_exists(linked2.link_filedir_path(&["t1"], false));

    th.assert_path_exists(linked3.link_filedir_path(&["t2"], false));
    th.assert_path_exists(linked3.link_filedir_path(&["t1"], false));
    th.assert_path_exists(linked3.link_filedir_path(&["t1", "t2"], false));
    th.assert_path_exists(linked3.link_filedir_path(&["t2", "t1"], false));
    th.assert_path_not_exists(linked3.link_filedir_path(&["t3"], false));
    th.assert_path_not_exists(linked3.link_filedir_path(&["t1", "t3"], false));

    th.assert_path_exists(linked4.link_filedir_path(&["t1"], false));
    th.assert_path_exists(linked4.link_filedir_path(&["t3"], false));
    th.assert_path_exists(linked4.link_filedir_path(&["t1", "t3"], false));
    th.assert_path_exists(linked4.link_filedir_path(&["t3", "t1"], false));
    th.assert_path_not_exists(linked4.link_filedir_path(&["t2"], false));
    th.assert_path_not_exists(linked4.link_filedir_path(&["t1", "t2"], false));

    th.assert_count(&["t1"], 3);
    th.assert_count(&["t2"], 2);
    th.assert_count(&["t3"], 1);

    th.assert_count(&["t1", "t2"], 1);
    th.assert_count(&["t1", "t3"], 1);

    Ok(())
}

#[test]
fn test_filedir() -> TestResult {
    let th = TestHelper::new(None);

    let _linked = th.ln(&["t1", "t2"])?;

    // make sure the filedir exists in the tag directories
    th.assert_path_exists(th.filedir_path(&["t1"]));
    th.assert_path_exists(th.filedir_path(&["t1", "t2"]));

    Ok(())
}

#[test]
fn test_mkdir_tag() -> TestResult {
    let th = TestHelper::new(None);
    let tag_dir = th.mountpoint_path(&["t1"]);
    std::fs::create_dir(tag_dir)?;
    th.assert_parts_exists(&["t1"]);
    Ok(())
}

#[test]
fn test_same_file_cli() -> TestResult {
    let th = TestHelper::new(None);
    _test_same_file(th)
}

#[test]
fn test_same_file_manual() -> TestResult {
    let mut th = TestHelper::new(None);
    th.symlink_mode = OpMode::MANUAL;
    _test_same_file(th)
}

#[test]
#[cfg(target_os = "macos")]
fn test_same_file_finder() -> TestResult {
    let mut th = TestHelper::new(None);
    th.symlink_mode = OpMode::FINDER;
    _test_same_file(th)
}

/// Tests that two links made two separate times to the same file do indeed point to the same file.  Only really useful
/// for Finder links, but we'll do it for all just in case
fn _test_same_file(th: TestHelper) -> TestResult {
    let temp = Rc::new(NamedTempFile::new()?);

    let l1 = th.ln_with_tempfile(temp.clone(), &["t1"])?;
    std::thread::sleep(std::time::Duration::from_millis(1000));
    let l2 = th.ln_with_tempfile(temp.clone(), &["t1"])?;

    // we're using !inodify to prove that they're the same file because there should be no collisions
    th.assert_path_exists(l1.link_filedir_path(&["t1"], false));
    th.assert_path_exists(l2.link_filedir_path(&["t1"], false));

    Ok(())
}

#[test]
fn test_no_filedir() -> TestResult {
    let th = TestHelper::new(None);
    th.mkdir("t1")?;

    let num1 = std::fs::read_dir(th.mountpoint_path(&["t1"]))?.count();
    assert_eq!(num1, 0);

    th.ln(&["t1"])?;

    let num2 = std::fs::read_dir(th.mountpoint_path(&["t1"]))?.count();
    assert_eq!(num2, 1);

    Ok(())
}

#[test]
fn test_recursive_link_cli() -> TestResult {
    let th = TestHelper::new(None);
    _test_recursive_link(th)
}

#[test]
fn test_recursive_link_manual() -> TestResult {
    let mut th = TestHelper::new(None);
    th.symlink_mode = OpMode::MANUAL;
    _test_recursive_link(th)
}

#[test]
#[cfg(target_os = "macos")]
fn test_recursive_link_finder() -> TestResult {
    let mut th = TestHelper::new(None);
    th.symlink_mode = OpMode::FINDER;
    _test_recursive_link(th)
}

/// Tests that if you create a symlink to an existing symlink in supertag, the new symlink points to the original file,
/// not the old symlink
fn _test_recursive_link(th: TestHelper) -> TestResult {
    let l1 = th.ln(&["t1"])?;

    // now we'll make a new link to this existing link in supertag
    th.ln_with_file(&l1.link_filedir_path(&["t1"], false), &["t2"])?;

    let l2_path = th
        .filedir_path(&["t2"])
        .join(l1.target_path().file_name().unwrap().to_str().unwrap());

    // and we'll prove that the new link points to the original file, not the first link
    let recursive = l2_path.canonicalize()?;
    assert_eq!(l1.target_path(), recursive);

    Ok(())
}

#[test]
/// Tests that linking directly to a tag's filedir works the same as linking to the tag
fn test_ln_to_filedir_manual() -> TestResult {
    let th = TestHelper::new(None);
    th.mkdir("t1")?;

    let src = tempfile::NamedTempFile::new()?;
    let dst = th
        .filedir_path(&["t1"])
        .join(src.path().file_name().unwrap());

    std::os::unix::fs::symlink(src.path(), &dst)?;

    th.assert_path_exists(&dst);

    Ok(())
}

#[test]
/// Tests that linking directly to a tag's filedir works the same as linking to the tag
fn test_ln_to_filedir_cli() -> TestResult {
    let th = TestHelper::new(None);
    th.mkdir("t1")?;

    let src = tempfile::NamedTempFile::new()?;
    let dst = th
        .filedir_path(&["t1"])
        .join(src.path().file_name().unwrap());

    th.ln_cli(src.path(), &dst)?;

    th.assert_path_exists(&dst);

    Ok(())
}

#[test]
#[cfg(target_os = "macos")]
fn test_ln_to_filedir_finder() -> TestResult {
    let th = TestHelper::new(None);
    th.mkdir("t1")?;

    let src = tempfile::NamedTempFile::new()?;
    let dst = th
        .filedir_path(&["t1"])
        .join(src.path().file_name().unwrap());

    supertag::platform::mac::alias::create_alias(src.path(), &dst)?;

    th.assert_path_exists(&dst);
    Ok(())
}

#[test]
fn test_cli_filedir() -> TestResult {
    let th = TestHelper::new(None);
    let _l1 = th.ln(&["t1", "t2", "t3"])?;

    let syms = th.settings.get_config().symbols;
    let files1 = th.ls(&["t1", &syms.filedir_cli_str])?;
    let files2 = th.ls(&["t1", &syms.filedir_str])?;

    assert_eq!(files1, files2);
    Ok(())
}
