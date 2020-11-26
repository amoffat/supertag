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

use super::{OpMode, TestHelper, TestResult};
use std::io::{ErrorKind, Write};

#[test]
fn test_basic_alias() -> TestResult {
    let mut th = TestHelper::new(None);
    th.symlink_mode = OpMode::FINDER;

    let l1 = th.ln(&["t1"])?;

    let linked_path = l1.link_filedir_path(&["t1"], false);
    let resolved = std::fs::canonicalize(linked_path)?;
    assert_eq!(l1.target_path(), resolved);

    Ok(())
}

#[test]
/// Tests that an alias symlink behaves like an alias with the self-healing properties if the source file moves
/// Note, this test does leave a temp file laying around FIXME
fn test_transparent_symlink() -> TestResult {
    let mut th = TestHelper::new(None);
    th.symlink_mode = OpMode::FINDER;

    th.mkdir("t1")?;

    let tf = tempfile::NamedTempFile::new()?;
    let original_name = tf.path().file_name().unwrap();
    supertag::platform::mac::alias::create_alias(
        tf.path(),
        th.mountpoint_path(&["t1"]).join(original_name),
    )?;
    let (_, mut persisted) = tf.keep()?;
    persisted = persisted.canonicalize()?; // for macos

    let mut moved_name = persisted.file_name().unwrap().to_owned();
    moved_name.push("-moved");
    let mut moved = persisted.clone();
    moved.set_file_name(moved_name);
    println!("{} {}", persisted.display(), moved.display());
    std::fs::rename(&persisted, &moved)?;

    assert_ne!(&persisted, &moved);

    let linked_path = th
        .filedir_path(&["t1"])
        .join(persisted.file_name().unwrap());

    let resolved = linked_path.canonicalize()?;
    assert_eq!(&moved, &resolved);

    Ok(())
}

#[test]
/// Tests that we can't create a file that isn't an alias
fn test_alias_bad() -> TestResult {
    let th = TestHelper::new(None);
    let tag_dir = th.mountpoint_path(&["t1"]);
    std::fs::create_dir(tag_dir)?;

    let alias_bytes = b"abc";
    let alias_file = th.mountpoint_path(&["t1"]).join("test_alias");
    let mut h = std::fs::File::create(&alias_file)?;

    match h.write(alias_bytes) {
        Err(e) => match e.kind() {
            ErrorKind::PermissionDenied => {}
            e => panic!("Wrong error {:?}", e),
        },
        Ok(_) => panic!("Should have failed writing"),
    }

    // now make sure it doesn't exist
    let mut found_alias = false;
    for maybe_entry in std::fs::read_dir(th.filedir_path(&["t1"]))? {
        let entry = maybe_entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with("test_alias") {
            found_alias = true;
            break;
        }
    }
    assert!(!found_alias);

    Ok(())
}
