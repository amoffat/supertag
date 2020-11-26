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

#![warn(
    clippy::all,
    clippy::restriction,
    clippy::pedantic,
    clippy::nursery,
    clippy::cargo
)]
#![allow(
    clippy::expect_used,
    clippy::multiple_crate_versions,
    clippy::implicit_return,
    clippy::expect_used,
    clippy::missing_docs_in_private_items,
    clippy::missing_inline_in_public_items,
    clippy::shadow_reuse,
    clippy::similar_names,
    clippy::single_match_else,
    clippy::wildcard_enum_match_arm
)]

use std::error::Error;

use clap::{App, Arg};

use common::constants;
use common::settings::config::HashMapSource;
use common::settings::Settings;
use common::types::file_perms::UMask;
use std::sync::Arc;
use supertag::cli::commands::ArgDefaults;
use supertag::cli::handlers;
use supertag::{cli, common};

fn main() -> Result<(), Box<dyn Error>> {
    let uid = unsafe { libc::getuid() };
    let gid = unsafe { libc::getgid() };
    let umask = UMask::default();

    let defaults = ArgDefaults {
        uid: uid.to_string(),
        gid: gid.to_string(),
        mount_perms: format!("{:o}", umask.dir_perms().mode()),
    };

    let version_str = common::version_str();
    let app = App::new("Supertag")
        .version(&*version_str)
        .author("Andrew Moffat <arwmoffat@gmail.com>")
        .about("Supertag smart filesystem")
        .settings(&[clap::AppSettings::ArgRequiredElseHelp])
        .arg(
            Arg::with_name("verbosity")
                .short("v")
                .multiple(true)
                .help("Sets the level of verbosity"),
        );

    let attached_app = cli::commands::add_subcommands(app, &defaults);
    let matches = attached_app.get_matches();

    let pd =
        Arc::new(directories::ProjectDirs::from("", constants::ORG, constants::APP_NAME).unwrap());

    let mut settings = Settings::new(pd.clone())?;
    let conf_file = settings.base_config_file();

    let mut config_sources: Vec<Box<dyn config::Source + Send + Sync>> =
        vec![Box::new(config::File::from(conf_file))];

    // Here we're setting up the logger two different ways: one way if we're running mount, and
    // another way for all the other subcommands.  We do this for two reasons: 1) only mount should
    // go to the collection log file, and 2) the default log level for non-mount should be silent
    if let Some(args) = matches.subcommand_matches("mount") {
        let maybe_log = match matches.occurrences_of("verbosity") {
            0 => None,
            1 => Some(log::LevelFilter::Info),
            2 => Some(log::LevelFilter::Debug),
            _ => Some(log::LevelFilter::Trace),
        };
        let collection = args.value_of("collection").expect("Collection required!");
        settings.set_collection(collection, false);

        // now let's set up our logger for the mount command.  this has a wrinkle because we only
        // want to log to stdout if we haven't forked, because if we have forked to the background,
        // we don't want the background process spitting output to the terminal while other commands
        // are trying to run
        let mut log_outputs: Vec<fern::Output> = vec![];

        let rotating_log = common::log::RotatingLogger::new(
            settings.log_dir(collection),
            format!("%Y-%m-%d-%H-{}.log", collection),
            6,
            100,
        )?;

        log_outputs.push(From::<Box<dyn log::Log>>::from(Box::new(rotating_log)));
        if args.is_present("foreground") {
            log_outputs.push(std::io::stdout().into());
        }
        if let Some(log_level) = maybe_log {
            supertag::common::log::setup_logger(log_level, log_outputs)?;
        }

        let mut cli_source = HashMapSource(Default::default());
        cli_source.0.insert(
            "mount.uid".to_string(),
            args.value_of("uid")
                .expect("Uid not specified")
                .parse::<i64>()?
                .into(),
        );
        cli_source.0.insert(
            "mount.gid".to_string(),
            args.value_of("gid")
                .expect("Gid not specified")
                .parse::<i64>()?
                .into(),
        );
        cli_source.0.insert(
            "mount.permissions".to_string(),
            args.value_of("permissions")
                .expect("Permissions not specified")
                .into(),
        );

        config_sources.push(Box::new(cli_source));
    } else {
        let maybe_log = match matches.occurrences_of("verbosity") {
            0 => None,
            1 => Some(log::LevelFilter::Info),
            2 => Some(log::LevelFilter::Debug),
            _ => Some(log::LevelFilter::Trace),
        };
        if let Some(log_level) = maybe_log {
            common::log::setup_logger(log_level, vec![std::io::stdout().into()])?;
        }

        // these 3 settings aren't used for anything tag related, but we still need them set as defaults
        // for when settings is deserialized
        let unused_defaults = config::File::from_str(
            r#"
[mount]
uid=0
gid=0
permissions="777""#,
            config::FileFormat::Toml,
        );
        config_sources.push(Box::new(unused_defaults));
    }

    let conf = crate::common::settings::config::build(config_sources, &*pd);
    settings.update_config(conf);

    match matches.subcommand() {
        ("ln", Some(args)) => handlers::ln::handle(args, settings),
        ("mv", Some(args)) => handlers::mv::handle(args, settings),
        ("rm", Some(args)) => handlers::rm::handle(args, settings),
        ("rmdir", Some(args)) => handlers::rmdir::handle(args, settings),
        ("unmount", Some(args)) => handlers::unmount::handle(args, settings),
        ("fstab", Some(args)) => handlers::fstab::handle(args, settings),
        ("mount", Some(args)) => handlers::mount::handle(args, settings),
        _ => Err("Command not found".into()),
    }
}
