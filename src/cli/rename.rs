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
use crate::common::notify::Notifier;
use crate::common::settings::Settings;
use crate::common::types::file_perms::UMask;
use libc::{gid_t, uid_t};
use log::info;
use rusqlite::{Connection, TransactionBehavior};
use std::path::Path;

pub fn rename<P: AsRef<Path>, Q: AsRef<Path>, R: AsRef<Path>, N: Notifier>(
    settings: &Settings,
    conn: &mut Connection,
    mountpoint: R,
    src: P,
    dst: Q,
    uid: uid_t,
    gid: gid_t,
    umask: &UMask,
    notifier: &N,
) -> STagResult<()> {
    info!(
        target: CLI_TAG,
        "Renaming file from {} to {}",
        src.as_ref().display(),
        dst.as_ref().display()
    );

    let relative_src = super::strip_prefix(src.as_ref(), mountpoint.as_ref());
    let relative_dst = super::strip_prefix(dst.as_ref(), mountpoint.as_ref());

    let tx = conn.transaction_with_behavior(TransactionBehavior::Exclusive)?;
    common::fsops::move_or_merge(
        settings,
        &tx,
        relative_src,
        relative_dst,
        uid,
        gid,
        umask,
        notifier,
    )?;
    tx.commit()?;

    flush_path(src.as_ref(), settings);
    flush_path(dst.as_ref(), settings);

    Ok(())
}
