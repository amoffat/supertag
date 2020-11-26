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
use crate::common::settings::Settings;
use crate::sql;
use clap::{values_t, ArgMatches};
use log::info;
use std::error::Error;

pub fn handle(args: &ArgMatches, mut settings: Settings) -> Result<(), Box<dyn Error>> {
    info!(target: TAG, "Running rmdir");

    let paths = values_t!(args.values_of("path"), String).expect("path is required!");

    let col = settings.resolve_collection(paths.get(0).expect("Need one path"))?;
    let mut conn = sql::db_for_collection(&settings, &col)?;

    for path in paths {
        crate::rmdir(&settings, &mut conn, settings.mountpoint(&col), path)?;
    }
    Ok(())
}
