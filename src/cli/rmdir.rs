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
use crate::common::fsops::flush_path;
use crate::common::settings::Settings;
use log::info;
use rusqlite::{Connection, TransactionBehavior};
use std::path::Path;

pub fn rmdir<P1: AsRef<Path>, P2: AsRef<Path>>(
    settings: &Settings,
    conn: &mut Connection,
    mountpoint: P1,
    path: P2,
) -> STagResult<()> {
    let path = path.as_ref();
    info!(target: CLI_TAG, "Removing directory {:?}", path);

    let relpath = super::strip_prefix(path.as_ref(), mountpoint.as_ref());

    let tx = conn.transaction_with_behavior(TransactionBehavior::Exclusive)?;
    common::fsops::rmdir(settings, &tx, relpath)?;
    tx.commit()?;

    flush_path(path, settings);

    Ok(())
}
