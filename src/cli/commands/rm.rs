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
use clap::{Arg, SubCommand};

pub(super) fn add_subcommands<'a, 'b>(app: clap::App<'a, 'b>) -> clap::App<'a, 'b> {
    app.subcommand(
        SubCommand::with_name("rm")
            .about("Removes the last tag in a path from a specific file.")
            .arg(
                Arg::with_name("file")
                    .help("The file path to remove from the tags in the path")
                    .required(true)
                    .takes_value(true),
            ),
    )
}
