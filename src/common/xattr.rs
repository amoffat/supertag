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

use log::{debug, info};
use std::collections::HashMap;
use std::path::Path;

/// Renames a file and preserves xattrs
pub fn rename<P: AsRef<Path>, Q: AsRef<Path>>(from: P, to: Q) -> std::io::Result<()> {
    info!(
        "Renaming {} to {} while preserving xattrs",
        from.as_ref().display(),
        to.as_ref().display()
    );
    let mut xattr_map = HashMap::new();

    for xa in xattr::list(&from)? {
        if let Some(val) = xattr::get(from.as_ref(), &xa)? {
            debug!("got xattr {:?} with values {:?}", xa, val);
            xattr_map.insert(xa.clone(), val.clone());
        }
    }

    std::fs::rename(&from, &to)?;

    for (k, v) in xattr_map {
        debug!("setting xattr {:?} with values {:?}", k, v);
        xattr::set(&to, &k, v.as_slice())?;
    }

    Ok(())
}
