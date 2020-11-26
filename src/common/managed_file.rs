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

use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

/// Converts some original path into a unlikely-to-collide subdirectory path, based on chunks of
/// a hash of the original path
pub fn subdir_path<P: AsRef<Path>>(orig_path: P) -> (PathBuf, String) {
    // yes i know md5 isn't *cryptographically* secure, but we're just using it as a content hash
    // for directory structure, so it's fine.
    let digest = md5::compute(orig_path.as_ref().as_os_str().as_bytes());
    let chunk_size = 1;

    let path = digest
        .chunks(chunk_size)
        .map(|c| {
            let mut s = String::with_capacity(chunk_size * 2);
            for byte in c {
                s.push_str(&format!("{:02x}", byte));
            }
            s
        })
        .collect::<Vec<_>>()
        .join(&std::path::MAIN_SEPARATOR.to_string())
        .into();

    (path, format!("{:x}", digest))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subdir_path() {
        let (path, hash) = subdir_path("/tmp/abc.txt");
        assert_eq!(
            path.display().to_string(),
            "88/2a/46/06/3f/a0/7f/5a/06/2c/e0/75/57/40/8b/7b"
        );
        assert_eq!(hash, "882a46063fa07f5a062ce07557408b7b");
    }
}
