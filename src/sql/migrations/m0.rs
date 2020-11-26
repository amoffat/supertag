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
use rusqlite::Result as SqliteResult;
use rusqlite::{Transaction, NO_PARAMS};

pub fn migrate(tx: &Transaction) -> SqliteResult<()> {
    // our metadata table for future proofing
    tx.execute(
        "CREATE TABLE IF NOT EXISTS supertag_meta (
            migration_version INTEGER NOT NULL DEFAULT 0,
            supertag_version TEXT NOT NULL,
            root_mtime FLOAT NOT NULL
        )",
        NO_PARAMS,
    )?;

    tx.execute(
        "INSERT INTO supertag_meta
        (migration_version, supertag_version, root_mtime)
        VALUES (0, '0.0.0', (select (julianday('now') - 2440587.5)*86400.0))",
        NO_PARAMS,
    )?;

    // this table contains references to files as they exist on the file system.  `primary_tag`,
    // by default, is the original file name, but it can be renamed without affecting the source
    // file
    tx.execute(
        "CREATE TABLE IF NOT EXISTS files (
            id INTEGER PRIMARY KEY NOT NULL,
            device INTEGER NOT NULL,
            inode INTEGER NOT NULL,
            path TEXT NOT NULL UNIQUE,
            primary_tag TEXT NOT NULL,
            ts FLOAT NOT NULL,
            mtime FLOAT NOT NULL,
            alias_file TEXT,
            UNIQUE (device, inode)
        )",
        NO_PARAMS,
    )?;

    // tags are the entities that manifest as directories
    tx.execute(
        "CREATE TABLE IF NOT EXISTS tags (
            id INTEGER PRIMARY KEY NOT NULL,
            tag_name TEXT NOT NULL UNIQUE,
            ts FLOAT NOT NULL,
            mtime FLOAT NOT NULL,
            uid INTEGER NOT NULL,
            gid INTEGER NOT NULL,
            permissions INTEGER NOT NULL,
            num_files INTEGER NOT NULL DEFAULT 0
        )",
        NO_PARAMS,
    )?;

    tx.execute(
        "CREATE TABLE IF NOT EXISTS file_tag (
            file_id INTEGER NOT NULL,
            tag_id INTEGER NOT NULL,
            ts FLOAT NOT NULL,
            mtime FLOAT NOT NULL,
            uid INTEGER NOT NULL,
            gid INTEGER NOT NULL,
            permissions INTEGER NOT NULL,
            PRIMARY KEY (file_id, tag_id),
            FOREIGN KEY (file_id) REFERENCES files (id) ON DELETE CASCADE,
            FOREIGN KEY (tag_id) REFERENCES tags (id) ON DELETE CASCADE
        )",
        NO_PARAMS,
    )?;

    // I thought of a few different ways to implement pins, and a sqlite3 full-text search table
    // is what I settled on.  Essentially we need a way to represent a hierarchical tag path that
    // can be searched by prefix.  For example, if I pin directory `t1/t2/t3/`, that means that
    // the following directories are considered pinned:
    //
    //    * t1/
    //    * t1/t2/
    //    * t1/t2/t3/
    //
    // but not:
    //
    //    * t2/
    //    * t2/t3/
    //    * t3/
    //    * etc
    //
    // Essentially what we want is some kind of prefix tree, where prefix searches are better than
    // linear.  Using FTS5 with a prefix index seems to get us this.  Unfortunately, we can't use prefix searches
    // because they are not totally supported in
    // sqlite 3.19.3 2017-06-27 16:48:08 2b0954060fe10d6de6d479287dd88890f1bef6cc1beca11bc6cdb79f72e2377b
    // which is what the MacOS I'm testing on is using.
    tx.execute(
        "CREATE TABLE IF NOT EXISTS pins (tag_ids TEXT NOT NULL)",
        NO_PARAMS,
    )?;

    tx.execute(
        "CREATE TABLE IF NOT EXISTS tag_groups (
            id INTEGER PRIMARY KEY NOT NULL,
            name TEXT NOT NULL UNIQUE,
            ts FLOAT NOT NULL,
            mtime FLOAT NOT NULL,
            uid INTEGER NOT NULL,
            gid INTEGER NOT NULL,
            permissions INTEGER NOT NULL
        )",
        NO_PARAMS,
    )?;

    tx.execute(
        "CREATE TABLE IF NOT EXISTS tag_group_tag (
            tg_id INTEGER NOT NULL,
            tag_id INTEGER NOT NULL,
            ts FLOAT NOT NULL,
            mtime FLOAT NOT NULL,
            uid INTEGER NOT NULL,
            gid INTEGER NOT NULL,
            permissions INTEGER NOT NULL,
            PRIMARY KEY (tg_id, tag_id),
            FOREIGN KEY (tg_id) REFERENCES tag_groups (id) ON DELETE CASCADE,
            FOREIGN KEY (tag_id) REFERENCES tags (id) ON DELETE CASCADE
        )",
        NO_PARAMS,
    )?;

    Ok(())
}
