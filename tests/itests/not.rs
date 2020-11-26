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

#[test]
fn test_not_tag() -> TestResult {
    let th = TestHelper::new(None);
    let l1 = th.ln(&["t1", "t2", "t3"])?;
    let l2 = th.ln(&["t1", "t3"])?;

    th.assert_path_exists(th.filedir_path(&["t1", "-t2"]));
    th.assert_path_exists(th.filedir_path(&["t1", "-t2", "t3"]));
    th.assert_path_not_exists(l1.link_filedir_path(&["t1", "-t2", "t3"], false));
    th.assert_path_exists(l2.link_filedir_path(&["t1", "-t2", "t3"], false));

    Ok(())
}

#[test]
fn test_not_tagdir() -> TestResult {
    let th = TestHelper::new(None);
    let _l1 = th.ln(&["t1", "t2", "t3"])?;
    let _l2 = th.ln(&["t1", "t3"])?;
    let _l3 = th.ln(&["t1", "t2", "t4"])?;

    th.assert_path_exists(th.filedir_path(&["t1", "-t2"]));
    th.assert_path_exists(th.filedir_path(&["t1", "-t2", "t3"]));
    th.assert_parts_exists(&["t1", "-t2", "t3"]);
    th.assert_parts_not_exists(&["t1", "-t2", "t4"]);

    Ok(())
}

#[test]
fn test_disallow_not_tag_name_cli() -> TestResult {
    let th = TestHelper::new(None);
    _test_disallow_not_tag_name(th)
}

#[test]
fn test_disallow_not_tag_name_manual() -> TestResult {
    let mut th = TestHelper::new(None);
    th.mkdir_mode = OpMode::MANUAL;
    _test_disallow_not_tag_name(th)
}

#[test]
#[cfg(target_os = "macos")]
fn test_disallow_not_tag_name_finder() -> TestResult {
    let mut th = TestHelper::new(None);
    th.mkdir_mode = OpMode::FINDER;
    _test_disallow_not_tag_name(th)
}

fn _test_disallow_not_tag_name(th: TestHelper) -> TestResult {
    th.ln(&["-t1"])?;

    th.assert_parts_not_exists(&["t1"]);
    th.assert_parts_not_exists(&["-t1"]);
    th.assert_parts_not_exists(&["--t1"]);
    Ok(())
}
