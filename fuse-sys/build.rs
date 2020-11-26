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

extern crate pkg_config;

use std::path::Path;

#[cfg(not(target_os = "macos"))]
static LIBFUSE_NAME: &str = "fuse";

#[cfg(target_os = "macos")]
static LIBFUSE_NAME: &str = "osxfuse";

fn main() {
    let out_dir: std::path::PathBuf = std::env::var("OUT_DIR").unwrap().into();

    // this will also print out the appropriate "cargo:rustc" meta commands to stdout
    let fuse_lib = pkg_config::Config::new()
        .atleast_version("2.6.0")
        .probe(LIBFUSE_NAME)
        .expect(&format!("Invalid version of {}", LIBFUSE_NAME));

    // Tell cargo to invalidate the built crate whenever the wrapper changes
    println!("cargo:rerun-if-changed=wrapper.h");

    // pull our cflags from pkg_config
    let mut cflags = vec![];
    for (key, maybe_val) in fuse_lib.defines {
        let entry = match maybe_val {
            Some(val) => format!("-D{}={}", key, val),
            None => format!("-D{}", key),
        };
        cflags.push(entry);
    }

    let mut include_paths = vec![];
    for path in fuse_lib.include_paths {
        include_paths.push(format!("-I{}", path.display()));
    }

    let bindings = bindgen::Builder::default()
        .header("wrapper.h")
        // Tell cargo to invalidate the built crate whenever any of the
        // included header files changed.
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        .clang_arg(format!("-DFUSE_USE_VERSION={}", 26))
        .clang_args(cflags)
        .clang_args(include_paths)
        .generate()
        .expect("Unable to generate bindings");

    let fuse_binding = Path::new("fuse_bindings.rs");
    bindings
        .write_to_file(out_dir.join(fuse_binding))
        .expect("Couldn't write bindings!");
}
