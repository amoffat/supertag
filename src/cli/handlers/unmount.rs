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
use clap::ArgMatches;
use log::info;
use std::error::Error;

pub fn handle(args: &ArgMatches, settings: Settings) -> Result<(), Box<dyn Error>> {
    info!(target: TAG, "Running umount");
    let to_unmount = match args.value_of("collection") {
        Some(col) => vec![col.to_owned()],
        None => crate::platform::mounted_collections()?
            .keys()
            .map(|c| c.to_owned())
            .collect(),
    };

    for col in to_unmount {
        let mountpoint = settings.supertag_dir().join(col);
        crate::platform::unmount(&mountpoint)?;
    }
    Ok(())
}
