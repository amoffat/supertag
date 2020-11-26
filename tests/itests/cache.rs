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
use crate::common::{TestHelper, TestResult};
use rusqlite::{params, NO_PARAMS};
use std::time::{Duration, UNIX_EPOCH};

#[test]
fn test_tag_cache() -> TestResult {
    let th = TestHelper::new(None);
    let _ = th.ln(&["t1"])?;

    th.assert_size(&["t1"], 1);
    let conn = th.fresh_conn();
    conn.execute("UPDATE tags SET num_files=num_files+1", NO_PARAMS)?;

    th.assert_size(&["t1"], 1);
    th.sleep_readdir_cache();
    th.assert_size(&["t1"], 2);
    Ok(())
}

#[test]
fn test_taggroup_cache() -> TestResult {
    let th = TestHelper::new(None);
    th.mkdir("a_tags+")?;
    let _ = th.ln(&["t1"])?;
    th.mv(
        th.mountpoint_path(&["t1"]),
        th.mountpoint_path(&["a_tags+"]),
    )?;

    let get_mtime = || {
        th.mountpoint_path(&["a_tags+"])
            .metadata()
            .unwrap()
            .modified()
            .unwrap()
    };
    let mtime = get_mtime();

    let updated_mtime = mtime
        .checked_add(Duration::from_secs(10))
        .unwrap()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs_f64();

    let conn = th.fresh_conn();
    conn.execute("UPDATE tag_groups SET mtime=?1", params![updated_mtime])?;

    let still_mtime = get_mtime();
    assert_eq!(still_mtime, mtime);

    th.sleep_readdir_cache();
    let new_mtime = get_mtime();

    assert!(
        new_mtime > mtime,
        "{:?} wasn't greater than {:?}",
        new_mtime,
        mtime
    );
    Ok(())
}

#[test]
fn test_file_cache() -> TestResult {
    let th = TestHelper::new(None);
    let l1 = th.ln(&["t1"])?;

    let get_mtime = || {
        l1.link_filedir_path(&["t1"], false)
            .symlink_metadata()
            .unwrap()
            .modified()
            .unwrap()
    };
    let mtime = get_mtime();

    let updated_mtime = mtime
        .checked_add(Duration::from_secs(10))
        .unwrap()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs_f64();
    let conn = th.fresh_conn();

    conn.execute("UPDATE file_tag SET mtime=?1", params![updated_mtime])?;
    let still_mtime = get_mtime();
    assert_eq!(still_mtime, mtime);

    th.sleep_readdir_cache();
    let new_mtime = get_mtime();

    assert!(
        new_mtime > mtime,
        "{:?} wasn't greater than {:?}",
        new_mtime,
        mtime
    );
    Ok(())
}
