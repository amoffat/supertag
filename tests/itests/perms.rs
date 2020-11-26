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
#[cfg(target_os = "linux")]
use crate::common::OpMode;
use nix::sys::stat::stat;
use supertag::common::types::file_perms::{Permissions, UMask};

#[test]
fn test_mountdir_perms() -> TestResult {
    {
        let test_config = r#"
[mount]
permissions = "775"
uid = 456
gid = 789
"#;
        let th = TestHelper::new(Some(test_config));
        let stat = nix::sys::stat::stat(&th.real_mountpoint())?;
        let perms = Permissions::from(stat.st_mode);

        assert_eq!(th.settings.get_config().mount.permissions, perms);
        assert_eq!(perms, Permissions::from(0o775));
        assert_eq!(stat.st_uid, 456);
        assert_eq!(stat.st_gid, 789);
    }

    {
        let test_config = r#"
[mount]
permissions = "644"
uid = 567
gid = 890
"#;
        let th = TestHelper::new(Some(test_config));
        let st = stat(&th.real_mountpoint())?;
        let perms = Permissions::from(st.st_mode);

        assert_eq!(th.settings.get_config().mount.permissions, perms);
        assert_eq!(perms, Permissions::from(0o644));
        assert_eq!(st.st_uid, 567);
        assert_eq!(st.st_gid, 890);
    }
    Ok(())
}

#[test]
fn test_tag_perms_cli() -> TestResult {
    let th = TestHelper::new(None);
    _test_tag_perms(th)
}

// because of https://github.com/osxfuse/osxfuse/issues/225, the directory permissions of the
// tag directory will be 777 on macos
#[cfg(target_os = "linux")]
#[test]
fn test_tag_perms_manual() -> TestResult {
    let mut th = TestHelper::new(None);
    th.mkdir_mode = OpMode::MANUAL;
    _test_tag_perms(th)
}

fn _test_tag_perms(th: TestHelper) -> TestResult {
    let tag_path = th.mkdir("t1")?;
    let st = stat(&tag_path)?;
    let perms = Permissions::from(st.st_mode);

    assert_eq!(perms, UMask::default().dir_perms());

    let filedir = th.filedir_path(&["t1"]);
    let st = stat(&filedir)?;
    let filedir_perms = Permissions::from(st.st_mode);
    assert_eq!(filedir_perms, UMask::default().dir_perms());

    Ok(())
}
