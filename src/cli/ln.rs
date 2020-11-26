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
use crate::common::get_filename;
use crate::common::notify::Notifier;
use crate::common::settings::Settings;
use crate::common::types::file_perms::UMask;
use libc::{gid_t, uid_t};
use log::info;
use rusqlite::{Connection, TransactionBehavior};
use std::path::{Path, PathBuf};

pub fn ln<P: AsRef<Path>, N: Notifier>(
    settings: &Settings,
    conn: &mut Connection,
    mountpoint: P,
    files: Vec<&Path>,
    tag_path: &Path,
    uid: uid_t,
    gid: gid_t,
    umask: &UMask,
    notifier: &N,
) -> STagResult<()> {
    let rel_tagpath = super::strip_prefix(tag_path, mountpoint.as_ref());

    // we have to do this outside of a transaction, because it will call the fuse handler getattr if we're attempting
    // to create a symlink to a supertag file, resulting a db connection deadlock
    let abs_files: Vec<PathBuf> = files
        .into_iter()
        .map(std::fs::canonicalize)
        .collect::<std::io::Result<Vec<PathBuf>>>()?;

    info!(
        target: CLI_TAG,
        "Linking files {:?} to {:?}", abs_files, rel_tagpath
    );

    let tx = conn.transaction_with_behavior(TransactionBehavior::Exclusive)?;
    for target in abs_files {
        let primary_tag = get_filename(&target)?;
        common::fsops::ln(
            settings,
            &tx,
            &target,
            rel_tagpath,
            &primary_tag,
            uid,
            gid,
            umask,
            None,
            notifier,
        )?;
    }
    tx.commit()?;

    // now that we've created a link, we need to send a signal (via stat) to flush the readdir
    // cache for the tag directory, so that the tag directory's mtime reports correctly
    flush_tags(rel_tagpath, settings, mountpoint);

    Ok(())
}
