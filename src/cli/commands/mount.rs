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
use super::ArgDefaults;
use clap::{Arg, SubCommand};

type ValidatorResult = Result<(), String>;

fn id_validator(v: String) -> ValidatorResult {
    let _ = v
        .parse::<u32>()
        .map_err(|_| format!("{} is not a valid id", v))?;
    Ok(())
}

fn perm_validator(v: String) -> ValidatorResult {
    u32::from_str_radix(&v, 8).map_err(|_| format!("{} is not a valid octal number", v))?;
    Ok(())
}

pub(super) fn add_subcommands<'a, 'b>(
    app: clap::App<'a, 'b>,
    defaults: &'a ArgDefaults,
) -> clap::App<'a, 'b> {
    app.subcommand(
        SubCommand::with_name("unmount")
            .about("Unmounts one or all collections")
            .arg(
                Arg::with_name("collection")
                    .help("Supertag collection name, eg 'media_files'.  This will be the name of our mounted drive.")
                    .takes_value(true),
            ),
    ).subcommand(
        SubCommand::with_name("mount")
            .about("Mounts a Supertag collection")
            .arg(
                Arg::with_name("collection")
                    .help("Supertag collection name, eg 'media_files'.  This will be the name of our mounted drive.")
                    .required(true)
                    .takes_value(true),
            )
            .arg(
                Arg::with_name("foreground")
                    .help("Don't run in the background as a daemon.")
                    .short("-f")
                    .long("--foreground"),
            )
            .arg(
                Arg::with_name("uid")
                    .help("The UID of the mounted directory.  By default, the process owner is used.")
                    .default_value(&defaults.uid)
                    .validator(id_validator)
                    .long("--uid"),
            )
            .arg(
                Arg::with_name("gid")
                    .help("The GID of the mounted directory.  By default, the process group is used.")
                    .default_value(&defaults.gid)
                    .validator(id_validator)
                    .long("--gid"),
            )
            .arg(
                Arg::with_name("permissions")
                    .help("The octal permissions of the mounted directory.  By default, the process umask's is considered.")
                    .default_value(&defaults.mount_perms)
                    .validator(perm_validator)
                    .long("--permissions"),
            )
    )
}
