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
use core_foundation as cf;
use core_foundation::base::{Boolean, TCFType, ToVoid};
use core_foundation::data::CFDataRef;
use core_foundation::error::{CFError, CFErrorRef};
use core_foundation::url::CFURLRef;
use std::ffi::CStr;
use std::path::{Path, PathBuf};
use std::ptr::null;

pub fn resolve_alias<P: AsRef<Path>>(path: P) -> Result<PathBuf, CFError> {
    // TODO does isDirectory need to be true for actual directories?
    let bmark_url = cf::url::CFURL::from_path(path, false).unwrap();

    unsafe {
        let mut err_ref: CFErrorRef = std::ptr::null_mut();
        let alias_data = cf::url::CFURLCreateBookmarkDataFromFile(
            null(),
            bmark_url.to_void() as CFURLRef,
            &mut err_ref,
        );

        if !err_ref.is_null() {
            let err: CFError = TCFType::wrap_under_create_rule(err_ref);
            return Err(err);
        }

        let opts: cf::url::CFURLBookmarkResolutionOptions = 0;
        let is_stale: Boolean = 0;

        let resolved = cf::url::CFURLCreateByResolvingBookmarkData(
            null(),
            alias_data,
            opts,
            null(),
            null(),
            is_stale as *mut Boolean,
            &mut err_ref,
        );

        if !err_ref.is_null() {
            let err: CFError = TCFType::wrap_under_create_rule(err_ref);
            return Err(err);
        }

        let mut buf = vec![0x0; 1024];
        let success = cf::url::CFURLGetFileSystemRepresentation(
            resolved.to_void() as CFURLRef,
            0,
            buf.as_mut_ptr(),
            buf.len() as isize,
        );
        assert_eq!(success, 1);

        let cs = CStr::from_ptr(buf.as_ptr() as *const i8);
        let final_path = PathBuf::from(cs.to_string_lossy().to_string());

        Ok(final_path)
    }
}

pub fn recursive_resolve_alias<P: AsRef<Path>>(path: P) -> Result<PathBuf, CFError> {
    let mut resolved = path.as_ref().to_owned();
    let mut at_least_once = false;
    loop {
        match resolve_alias(&resolved) {
            Ok(path) => {
                resolved = path;
                at_least_once = true;
            }
            Err(e) => {
                if at_least_once {
                    break;
                } else {
                    return Err(e);
                }
            }
        }
    }
    Ok(resolved)
}

/// Creates a Finder alias from src to dst
pub fn create_alias<P: AsRef<Path>, Q: AsRef<Path>>(src: P, dst: Q) -> Result<(), CFError> {
    let src_url = cf::url::CFURL::from_path(src, false).unwrap();

    unsafe {
        let mut err_ref: CFErrorRef = std::ptr::null_mut();
        let bmark_data = cf::url::CFURLCreateBookmarkData(
            null(),
            src_url.to_void() as CFURLRef,
            cf::url::kCFURLBookmarkCreationSuitableForBookmarkFile,
            null(),
            null(),
            &mut err_ref,
        );

        if !err_ref.is_null() {
            let err: CFError = TCFType::wrap_under_create_rule(err_ref);
            return Err(err);
        }

        let dst_url = cf::url::CFURL::from_path(dst, false).unwrap();
        let success = cf::url::CFURLWriteBookmarkDataToFile(
            bmark_data.to_void() as CFDataRef,
            dst_url.to_void() as CFURLRef,
            0,
            &mut err_ref,
        );

        if !err_ref.is_null() {
            let err: CFError = TCFType::wrap_under_create_rule(err_ref);
            return Err(err);
        }
        assert_eq!(success, 1);
    }

    Ok(())
}
