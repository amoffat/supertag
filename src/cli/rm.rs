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
use super::CLI_TAG;
use crate::common;
use crate::common::err::STagResult;
use crate::common::fsops::flush_tags;
use crate::common::settings::Settings;
use log::{debug, info};
use rusqlite::{Connection, TransactionBehavior};
use std::path::Path;

pub fn rm<P1: AsRef<Path>, P2: AsRef<Path>>(
    settings: &Settings,
    conn: &mut Connection,
    file: P1,
    mountpoint: P2,
) -> STagResult<()> {
    let file = file.as_ref();
    info!(target: CLI_TAG, "Removing file {:?}", file);

    let relpath = super::strip_prefix(file.as_ref(), mountpoint.as_ref());

    // this will remove our file from the database
    let tx = conn.transaction_with_behavior(TransactionBehavior::Exclusive)?;
    common::fsops::rm(settings, &tx, relpath)?;
    tx.commit()?;

    // but now we need to communicate to supertag that we want to clear the entry from its caches.
    // we do this by removing the file, but appending a special char, so that when supertag sees this
    // path in the unlink handler, it will know that we just want it cleared from the caches
    let sync_file = settings.suffix_sync_char(file)?;
    debug!(
        target: CLI_TAG,
        "Sending readdir cache sync for {:?}", sync_file
    );
    let _ = std::fs::metadata(sync_file);

    // this flushes all of the tags that contain the file removed. this is necessary because these tags
    // may exist in the readdir cache with the wrong size/num_files count now
    flush_tags(relpath, settings, mountpoint);

    Ok(())
}
