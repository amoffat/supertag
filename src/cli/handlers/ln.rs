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
use crate::common::notify::desktop::DesktopNotifier;
use crate::common::settings::Settings;
use crate::common::types::file_perms::UMask;
use crate::sql;
use clap::{values_t, ArgMatches};
use log::info;
use std::error::Error;
use std::path::{Path, PathBuf};

pub fn handle(args: &ArgMatches, mut settings: Settings) -> Result<(), Box<dyn Error>> {
    info!(target: TAG, "Running ln");
    let files = values_t!(args.values_of("file"), String).expect("file is required!");
    let files = files.iter().map(Path::new).collect();

    // FIXME make a cli arg
    let umask = UMask::default();
    let uid = unsafe { libc::getuid() };
    let gid = unsafe { libc::getgid() };

    let tag_path: PathBuf = args.value_of("path").expect("path is required!").into();

    let col = settings.resolve_collection(&tag_path)?;
    let mut conn = sql::db_for_collection(&settings, &col)?;
    let mountpoint = settings.mountpoint(&col);

    let notifier = DesktopNotifier::new(settings.notification_icon());

    crate::ln(
        &settings,
        &mut conn,
        &mountpoint,
        files,
        &tag_path,
        uid,
        gid,
        &umask,
        &notifier,
    )?;
    Ok(())
}
