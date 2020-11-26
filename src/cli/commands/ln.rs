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
        SubCommand::with_name("ln")
            .about("Links a file(s) to a tag directory")
            .arg(
                Arg::with_name("file")
                    .required(true)
                    .help("The file(s) to tag. It can be a relative path.")
                    .min_values(1)
                    .takes_value(true),
            )
            .arg(
                Arg::with_name("path")
                    .long_help(
                        r#"
The tags to link to the file, in path form.  The tags don't have to exist yet for this to work.

You can use a relative tag path:
    movies/action/marvel/superhero/avengers
    
Or an absolute tag path:
    /mnt/supertag/myfiles/movies/action/marvel/superhero/avengers
    
Note, however, if you use a relative tag path, the collection used by default will be the oldest mounted
collection.
"#
                            .trim(),
                    )
                    .required(true)
                    .takes_value(true),
            ),
    )
}
