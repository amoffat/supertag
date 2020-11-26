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
use crate::platform;
use clap::ArgMatches;
use log::info;
use std::error::Error;

pub fn handle(_args: &ArgMatches, settings: Settings) -> Result<(), Box<dyn Error>> {
    info!(target: TAG, "Running fstab");
    println!("Collections:");

    let all_cols = platform::all_collections(&settings)?;
    let mounted_cols = platform::mounted_collections()?;
    let maybe_pc = platform::primary_collection(&settings)?;
    for col in all_cols {
        let is_pc = match &maybe_pc {
            Some(pc) => pc == &col,
            None => false,
        };

        let maybe_mnt = mounted_cols.get(&col);
        let note = if is_pc { "* " } else { "" };
        let mnt = match maybe_mnt {
            Some(mnt) => format!(" => {}", mnt),
            None => "".to_string(),
        };
        println!("  {}{}{}", note, col, mnt);
    }
    Ok(())
}
