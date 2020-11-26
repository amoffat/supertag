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

//! This is the entrypoint for the commandline interface to access the supertag ops

#![warn(
    clippy::all,
    clippy::restriction,
    clippy::pedantic,
    clippy::nursery,
    clippy::cargo
)]
#![allow(
    clippy::option_expect_used,
    clippy::multiple_crate_versions,
    clippy::implicit_return,
    clippy::result_expect_used,
    clippy::missing_docs_in_private_items,
    clippy::missing_inline_in_public_items,
    clippy::shadow_reuse,
    clippy::similar_names,
    clippy::single_match_else,
    clippy::wildcard_enum_match_arm
)]

pub mod cli;
pub mod common;
pub mod fuse;
pub mod platform;
pub mod sql;

pub use cli::ln::ln;
pub use cli::rename::rename;
pub use cli::rm::rm;
pub use cli::rmdir::rmdir;
