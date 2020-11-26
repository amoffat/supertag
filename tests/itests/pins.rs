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
use std::collections::HashSet;
use std::fs;

// A tag created in a nested position should be *pinned*, or forced to exist even if there are no
// file intersections
#[test]
fn test_pin_tagdir() -> TestResult {
    let th = TestHelper::new(None);

    let tag_dir = th.mountpoint_path(&["t1"]);
    fs::create_dir(tag_dir)?;
    th.assert_parts_exists(&["t1"]);

    let tag_dir = th.mountpoint_path(&["t1", "t2"]);
    fs::create_dir(&tag_dir)?;

    // this tag should exist obviously because we just created it
    th.assert_parts_exists(&["t2"]);

    // pinned, because we created this pin explicitly
    th.assert_parts_exists(&["t1", "t2"]);

    // not pinned, even though both tags exist
    th.assert_parts_not_exists(&["t2", "t1"]);
    Ok(())
}

#[test]
fn test_pin_no_double_tagdir() -> TestResult {
    let th = TestHelper::new(None);

    let tag_dir = th.mountpoint_path(&["t1"]);
    fs::create_dir(tag_dir)?;
    th.assert_parts_exists(&["t1"]);

    // pin t1/t2
    let tag_dir = th.mountpoint_path(&["t1", "t2"]);
    fs::create_dir(&tag_dir)?;

    // now create a tagged file in t1/t2
    th.ln(&["t1", "t2"])?;

    // check that there isn't both a pinned tagdir & and legitimate intersection tagdir
    let mut seen = HashSet::new();
    for entry_err in fs::read_dir(th.mountpoint_path(&["t1"]))? {
        let entry = entry_err?;
        if seen.contains(&entry.path()) {
            panic!("Double entry");
        }
        seen.insert(entry.path());
    }

    Ok(())
}
