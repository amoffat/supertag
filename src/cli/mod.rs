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

use std::path::Path;

pub mod commands;
pub mod handlers;
pub mod ln;
pub mod rename;
pub mod rm;
pub mod rmdir;

const CLI_TAG: &str = "cli";

fn strip_prefix<'a>(p: &'a Path, prefix: &Path) -> &'a Path {
    p.strip_prefix(prefix).unwrap_or(p)
}
