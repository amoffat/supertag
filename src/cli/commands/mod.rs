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
mod fstab;
mod ln;
mod mount;
mod mv;
mod rm;
mod rmdir;

pub struct ArgDefaults {
    pub uid: String,
    pub gid: String,
    pub mount_perms: String,
}

pub fn add_subcommands<'a, 'b>(
    app: clap::App<'a, 'b>,
    defaults: &'a ArgDefaults,
) -> clap::App<'a, 'b> {
    let mut attached = app;
    attached = mv::add_subcommands(attached);
    attached = ln::add_subcommands(attached);
    attached = mount::add_subcommands(attached, defaults);
    attached = rmdir::add_subcommands(attached);
    attached = rm::add_subcommands(attached);
    attached = fstab::add_subcommands(attached);
    attached
}
