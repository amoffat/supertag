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
use super::TAG;
use crate::common::notify::uds::UDSNotifier;
use crate::common::settings::Settings;
use crate::common::types::file_perms::UMask;
use crate::sql;
use clap::ArgMatches;
use log::info;
use std::error::Error;

pub fn handle(args: &ArgMatches, mut settings: Settings) -> Result<(), Box<dyn Error>> {
    info!(target: TAG, "Running mv");
    let src = args.value_of("src").expect("src is required!");
    let dst = args.value_of("dst").expect("dst is required!");

    // FIXME come in from cli
    let umask = UMask::default();
    let uid = unsafe { libc::getuid() };
    let gid = unsafe { libc::getgid() };

    let col = settings.resolve_collection(src)?;
    let mut conn = sql::db_for_collection(&settings, &col)?;

    let notifier_socket = settings.notify_socket_file(&col);
    let notifier = UDSNotifier::new(notifier_socket, false)?;

    crate::rename(
        &settings,
        &mut conn,
        settings.mountpoint(&col),
        src,
        dst,
        uid,
        gid,
        &umask,
        &notifier,
    )?;
    Ok(())
}
