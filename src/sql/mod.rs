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

use rusqlite::{params, Connection, Row, ToSql, Transaction, NO_PARAMS};
use rusqlite::{OptionalExtension, Result};

use crate::common::types::file_perms::{Permissions, UMask};
use crate::common::types::{DeviceFile, TagCollectible, TagType, UtcDt};
use libc::{gid_t, mode_t, uid_t};
use log::{debug, error, info, trace, warn};
use std::collections::HashSet;
use std::path::Path;

pub mod migrations;
pub mod tpool;
pub mod types;

use crate::common::settings::Settings;
use std::borrow::Cow;
use types::*;

pub const SQL_TAG: &str = "sql";
pub const MAX_CONN: u32 = 50;

// libsqlite on ubuntu LTS 18.04 doesn't have UPSERT, which was added in 3.24.0 (2018-06-04).
// https://www.sqlite.org/lang_UPSERT.html

// You'll see casting back and forth between u64 and i64.  This is because sqlite only natively
// supports i64.  Casting will change the interpretation of the bytes on u64 -> i64 overflow, but
// the bytes stay the same, so casting back to u64 gives us the original value, so it's fine.
// https://github.com/jgallagher/rusqlite/issues/250

/// Returns a correct connection with a very permissive contention handler
pub fn get_conn<P: AsRef<Path>>(db_path: P) -> Result<Connection> {
    trace!(target: SQL_TAG, "Opening {:?}", db_path.as_ref());
    let conn = Connection::open(&db_path)?;
    trace!(target: SQL_TAG, "Opened {:?}", db_path.as_ref());

    trace!(target: SQL_TAG, "Enabling foreign keys");
    // so we get cascading deletes in our relationship tables
    conn.execute("PRAGMA foreign_keys = 1", NO_PARAMS)?;
    trace!(target: SQL_TAG, "Installing busy handler");
    conn.busy_handler(Some(|num| -> bool {
        if num >= MAX_CONN as i32 {
            error!(target: SQL_TAG, "Timed out waiting for connection lock");
            false
        } else {
            warn!(
                target: SQL_TAG,
                "Sqlite database contention!  Tried {} times to acquire lock.  Trying again soon...",
                num + 1
            );
            std::thread::sleep(std::time::Duration::from_millis(100));
            true
        }
    }))?;
    Ok(conn)
}

pub fn db_for_collection(settings: &Settings, collection: &str) -> Result<Connection> {
    debug!(
        target: SQL_TAG,
        "Acquiring db connection for collection {}", collection
    );
    let db_file = settings.db_file(&collection);
    let conn = crate::sql::get_conn(&db_file)?;

    debug!(
        target: SQL_TAG,
        "Got db connection for collection {}", collection
    );
    Ok(conn)
}

fn float_to_utcdt(val: f64) -> UtcDt {
    let secs = val.trunc() as i64;
    let nsecs: u32 = (val.fract() * 1e+9) as u32;

    let utc_dt = chrono::NaiveDateTime::from_timestamp(secs, nsecs);
    chrono::DateTime::from_utc(utc_dt, chrono::Utc)
}

fn to_tag(row: &Row) -> Result<Tag> {
    let tag = Tag {
        id: row.get(0)?,
        name: row.get(1)?,
        mtime: float_to_utcdt(row.get(2)?),
        uid: row.get(3)?,
        gid: row.get(4)?,
        permissions: Permissions::from(row.get::<usize, mode_t>(5)?),
        num_files: row.get(6)?,
    };
    Ok(tag)
}

fn to_taggedfile(row: &Row) -> Result<TaggedFile> {
    let tf = TaggedFile {
        id: row.get(0)?,
        inode: row.get::<usize, i64>(1)? as u64,
        device: row.get::<usize, i64>(2)? as u64,
        path: row.get(3)?,
        primary_tag: row.get(4)?,
        mtime: float_to_utcdt(row.get(5)?),
        uid: row.get(6)?,
        gid: row.get(7)?,
        permissions: Permissions::from(row.get::<usize, mode_t>(8)?),
        alias_file: row.get(9)?,
    };
    Ok(tf)
}

fn to_tag_group(row: &Row) -> Result<TagGroup> {
    let tag_ids_str = row.get(6).ok().unwrap_or("".to_string());
    let tag_ids = tag_ids_str
        .split(",")
        .filter_map(|tag_id| tag_id.parse::<i64>().ok())
        .collect();

    let tg = TagGroup {
        id: row.get(0)?,
        name: row.get(1)?,
        mtime: float_to_utcdt(row.get(2)?),
        uid: row.get(3)?,
        gid: row.get(4)?,
        permissions: Permissions::from(row.get::<usize, mode_t>(5)?),
        tag_ids,
        num_files: 0,
    };
    Ok(tg)
}

pub fn get_now_secs() -> f64 {
    let now = std::time::SystemTime::now();
    let unix_ts = now.duration_since(std::time::UNIX_EPOCH).unwrap();
    unix_ts.as_secs_f64()
}

pub fn resolve_tag_ids(conn: &Connection, tag_ids: &[i64]) -> Result<Vec<String>> {
    let query = format!(
        "
SELECT tag_name
FROM tags
WHERE id IN ({})
",
        tag_ids
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<String>>()
            .join(",")
    );
    conn.prepare(&query)?
        .query_map(NO_PARAMS, |row| Ok(row.get(0)?))?
        .collect()
}

/// Ensures a tag exists in the database. The return value is the authoritative name. This can differ from the name
/// we pass in, in the case of a plugin-generated tag, where we've renamed the tag after the plugin generated it.
pub fn ensure_tag(
    tx: &Transaction,
    tag: &str,
    uid: uid_t,
    gid: gid_t,
    permissions: &Permissions,
    now: f64,
) -> Result<(String, i64)> {
    debug!(target: SQL_TAG, "Ensuring tag {} exists", tag);

    // we'll use this as the default existing tag for the following scenarios:
    // 1) we're creating a tag that doesn't come from a plugin, in which case we'll use this query.
    // 2) we're creating a tag *from* a plugin, but the plugin doesn't find an existing tag with that plugin_id, BUT
    //    it doesn't mean that a tag with that name *without* a plugin doesn't exist, so we need to fall back to this
    //    query
    let maybe_default_tag = tx
        .query_row(
            "SELECT id, tag_name FROM tags WHERE tag_name=?1",
            params![tag],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let maybe_tag: Option<(i64, String)> = maybe_default_tag;

    if let Some((existing_tag_id, existing_tag_name)) = maybe_tag {
        update_tag_mtime(tx, &existing_tag_name, now)?;

        Ok((existing_tag_name, existing_tag_id))
    } else {
        debug!(target: SQL_TAG, "Tag doesn't exist, creating");
        tx.execute(
            "INSERT INTO tags (
            tag_name,
            ts,
            mtime,
            uid,
            gid,
            permissions
        ) VALUES (
            ?1,
            ?5,
            ?5,
            ?2,
            ?3,
            ?4
        )",
            params![tag, uid, gid, permissions, now],
        )?;

        let tag_id = get_tag_id(tx, tag)?.expect("No tag id?");

        // creating a new tag should update the root timestamp because all tags live at the root
        update_root_mtime(tx, now)?;
        Ok((tag.to_owned(), tag_id))
    }
}

pub fn ensure_tag_group(
    tx: &Transaction,
    name: &str,
    uid: uid_t,
    gid: gid_t,
    permissions: &Permissions,
    now: f64,
) -> Result<()> {
    info!(target: SQL_TAG, "Ensuring tag group {} exists", name);

    let maybe_tag_group: Option<i64> = tx
        .query_row(
            "SELECT id FROM tag_groups WHERE name=?1",
            params![name],
            |row| row.get(0),
        )
        .optional()?;

    if let Some(_tg_id) = maybe_tag_group {
        update_tag_group_mtime(tx, name, now)?;
    } else {
        tx.execute(
            "INSERT INTO tag_groups (
            name,
            ts,
            mtime,
            uid,
            gid,
            permissions
        ) VALUES (
            ?1,
            ?5,
            ?5,
            ?2,
            ?3,
            ?4
        )",
            params![name, uid, gid, permissions, now],
        )?;

        // creating a new tag should update the root timestamp because all tags live at the root
        update_root_mtime(tx, now)?;
    }

    Ok(())
}

pub fn update_tag_mtime(tx: &Transaction, tag: &str, now: f64) -> Result<()> {
    debug!(target: SQL_TAG, "Updating tag mtime for {} to {}", tag, now);
    tx.execute(
        "UPDATE tags SET mtime=?2 WHERE tag_name=?1",
        params![tag, now],
    )?;
    Ok(())
}

pub fn update_tag_group_mtime(tx: &Transaction, name: &str, now: f64) -> Result<()> {
    debug!(
        target: SQL_TAG,
        "Updating tag group mtime {} to {}", name, now
    );
    tx.execute(
        "UPDATE tag_groups SET mtime=?2 WHERE name=?1",
        params![name, now],
    )?;
    Ok(())
}

/// Adds a tag to a device/inode pair
pub fn link_file_to_tag(
    tx: &Transaction,
    device: u64,
    inode: u64,
    tag: &str,
    uid: uid_t,
    gid: gid_t,
    permissions: &Permissions,
    now: f64,
) -> Result<()> {
    let maybe_row: Option<(i64, i64)> = tx
        .query_row(
            "SELECT file_tag.file_id, file_tag.tag_id FROM file_tag
        JOIN files ON files.id = file_tag.file_id
        JOIN tags ON tags.id = file_tag.tag_id
        WHERE
            files.inode = ?1
            AND files.device = ?2
            AND tags.tag_name = ?3",
            params![inode as i64, device as i64, tag],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;

    if let Some((_file_id, _tag_id)) = maybe_row {
        // FIXME what to do here? anything?
        warn!(target: SQL_TAG, "File-tag already exists, skipping");
    } else {
        let updated_tags = tx.execute(
            "UPDATE tags SET num_files = num_files+1 WHERE tag_name=?",
            params![tag],
        )?;
        trace!(
            target: SQL_TAG,
            "Updated {} tags with num_files",
            updated_tags
        );

        tx.execute(
            "INSERT INTO file_tag (
                file_id,
                tag_id,
                ts,
                mtime,
                uid,
                gid,
                permissions
            ) VALUES (
                (SELECT id FROM files WHERE device = ?1 AND inode = ?2),
                (SELECT id FROM tags WHERE tag_name = ?3),
                ?7,
                ?7,
                ?4,
                ?5,
                ?6
         )",
            params![device as i64, inode as i64, tag, uid, gid, permissions, now],
        )?;
    }
    update_tag_mtime(tx, tag, now)?;
    update_root_mtime(tx, now)?;
    Ok(())
}

pub fn get_num_files(conn: &Connection, tags: &[TagType]) -> Result<usize> {
    Ok(files_tagged_with(conn, tags)?.len())
}

pub fn get_tag(conn: &Connection, tag: &str) -> Result<Option<Tag>> {
    info!(target: SQL_TAG, "Getting tag {}", tag);
    let query = "
SELECT
    id,
    tag_name,
    mtime,
    uid,
    gid,
    permissions,
    num_files
FROM tags
WHERE
    tag_name=?1
";
    trace!(target: SQL_TAG, "{}", query);
    conn.query_row(query, params![tag], to_tag).optional()
}

pub fn get_tags_in_tag_group(conn: &Connection, name: &str) -> Result<Vec<Tag>> {
    debug!(target: SQL_TAG, "Getting tags in tag group {}", name);
    let query = "
SELECT
    t.id,
    t.tag_name,
    t.mtime,
    t.uid,
    t.gid,
    t.permissions,
    t.num_files
FROM tags AS t
JOIN tag_group_tag AS tgt
    ON tgt.tag_id=t.id
JOIN tag_groups AS tg
    ON tgt.tg_id=tg.id
WHERE
    tg.name=?1
    ";
    conn.prepare(query)?
        .query_map(params![name], to_tag)?
        .collect()
}

pub fn get_tag_group(conn: &Connection, name: &str) -> Result<Option<TagGroup>> {
    debug!(target: SQL_TAG, "Getting tag group by name {}", name);

    conn.query_row(
        "
    SELECT
        tg.id,
        tg.name,
        tg.mtime,
        tg.uid,
        tg.gid,
        tg.permissions,
        GROUP_CONCAT(tgt.tag_id, ',')
    FROM tag_groups AS tg
    LEFT JOIN tag_group_tag AS tgt ON tgt.tg_id=tg.id
    WHERE tg.name=?1
    GROUP BY tg.id
    ",
        params![name],
        to_tag_group,
    )
    .optional()
}

pub fn get_tag_by_id(conn: &Connection, id: i64) -> Result<Option<Tag>> {
    debug!(target: SQL_TAG, "Getting tag by id {}", id);
    let query = "
    SELECT
        id,
        tag_name,
        mtime,
        uid,
        gid,
        permissions,
        num_files
    FROM tags where id=?1";
    trace!(target: SQL_TAG, "{}", query);
    conn.query_row(query, params![id], to_tag).optional()
}

pub fn get_tag_group_by_id(conn: &Connection, id: i64) -> Result<Option<TagGroup>> {
    debug!(target: SQL_TAG, "Getting tag group by id {}", id);
    let query = "
    SELECT
        tg.id,
        tg.name,
        tg.mtime,
        tg.uid,
        tg.gid,
        tg.permissions,
        GROUP_CONCAT(tgt.tag_id, ',')
    FROM tag_groups AS tg
    LEFT JOIN tag_group_tag AS tgt ON tgt.tg_id=tg.id
    WHERE tgt.tg_id=?1
    GROUP BY tg.id";
    trace!(target: SQL_TAG, "{}", query);
    conn.query_row(query, params![id], to_tag_group).optional()
}

pub fn get_all_tags(conn: &Connection) -> Result<Vec<Tag>> {
    info!(target: SQL_TAG, "Getting all tags");
    let query = "
    SELECT
        tags.id,
        tags.tag_name,
        tags.mtime,
        tags.uid,
        tags.gid,
        tags.permissions,
        tags.num_files
    FROM tags
    ORDER BY tag_name";
    trace!(target: SQL_TAG, "{}", query);
    conn.prepare(query)?.query_map(NO_PARAMS, to_tag)?.collect()
}

/// Returns all of the tag groups
pub fn get_all_tag_groups(conn: &Connection) -> Result<Vec<TagGroup>> {
    info!(target: SQL_TAG, "Getting all tag groups");
    let query = "SELECT
        tg.id,
        tg.name,
        tg.mtime,
        tg.uid,
        tg.gid,
        tg.permissions,
        GROUP_CONCAT(tgt.tag_id, ',')
    FROM tag_groups AS tg
    LEFT JOIN tag_group_tag AS tgt ON tgt.tg_id=tg.id
    GROUP BY tg.id
    ORDER BY name";

    trace!(target: SQL_TAG, "{}", query);

    conn.prepare(&query)?
        .query_map(NO_PARAMS, to_tag_group)?
        .collect()
}

/// Returns `true` if the tag named `name` exists
pub fn tag_exists(conn: &Connection, name: &str) -> Result<bool> {
    Ok(conn
        .query_row(
            "SELECT 1 FROM tags WHERE tag_name=?1",
            params![name],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

pub fn tag_group_exists(conn: &Connection, name: &str) -> Result<bool> {
    Ok(conn
        .query_row(
            "SELECT 1 FROM tag_groups WHERE name=?1",
            params![name],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

/// For a single top-level tag group, find all of the unique files it is an umbrella over
pub fn num_files_for_tag_group(conn: &Connection, tg: &str) -> Result<i64> {
    let tags = get_tags_in_tag_group(conn, tg)?;

    let ids = tags
        .iter()
        .map(|tag| tag.id.to_string())
        .collect::<Vec<String>>()
        .join(",");

    let query = format!(
        "
SELECT
    COUNT(DISTINCT ft.file_id)
FROM file_tag AS ft
JOIN files AS f
    ON f.id=ft.file_id
WHERE
    ft.tag_id IN ({})
    ",
        ids
    );
    let count = conn.query_row(&query, NO_PARAMS, |row| row.get(0))?;
    Ok(count)
}

/// For a given tag intersection, yield the number of *unique* files managed underneath
pub fn num_files_for_intersection(conn: &Connection, tags: &[TagType]) -> Result<i64> {
    let mut all_but_last = tags.iter().collect_regular();
    if let Some(last_tag) = all_but_last.pop() {
        let itags = intersect_tag(conn, &all_but_last.as_slice(), true)?;

        if let TagType::Regular(last_tag_name) = last_tag {
            match itags.iter().find(|tag| tag.name == last_tag_name) {
                Some(found) => Ok(found.num_files),
                None => Ok(0),
            }
        } else {
            Ok(0)
        }
    } else {
        Ok(0)
    }
}

/// Return all the tag groups that could exist at the intersection of `tags`
pub fn tag_group_intersections(conn: &Connection, tags: &[TagType]) -> Result<Vec<TagGroup>> {
    debug!(
        target: SQL_TAG,
        "Getting tag group intersections for {:?}", tags
    );

    let itags = intersect_tag(conn, tags, true)?;
    let sum_tag_files = num_files_for_intersection(conn, tags)?;
    debug!(
        target: SQL_TAG,
        "Using {} as the total files for the tag group", sum_tag_files
    );
    let mut tag_groups = HashSet::new();

    let tag_ids: Vec<i64> = itags.iter().map(|t| t.id).collect();

    for mut tg in tag_groups_for_tags(conn, tag_ids.as_slice())? {
        tg.num_files = sum_tag_files;
        tag_groups.insert(tg);
    }

    Ok(tag_groups.into_iter().collect())
}

/// Finds all tags that intersect with the tags of the files tagged with `tags`.
/// `exclude_provided` will keep `tags` out of the resulting Vec.  This is useful for getting the
/// subdirectories of a path, where `tags` represents that path, and we don't want `tags` listed as
/// subdirectories of itself.
pub fn intersect_tag(
    conn: &Connection,
    tags: &[TagType],
    exclude_provided: bool,
) -> Result<Vec<Tag>> {
    debug!(target: SQL_TAG, "Getting tag intersections for {:?}", tags);

    // short circuit here if we just want all the tags
    if tags.is_empty() {
        return get_all_tags(conn);
    }

    let outer_tmpl = "SELECT
        tags.id,
        tags.tag_name,
        MAX(file_tag.mtime) as mtime,
        tags.uid,
        tags.gid,
        tags.permissions,
        COUNT(file_tag.tag_id)
    FROM tags
    JOIN file_tag ON tags.id=file_tag.tag_id
    WHERE
        file_tag.file_id IN";

    let mut all_params: Vec<Box<dyn ToSql>> = vec![];

    let (subquery, params) = intersection_subquery(conn, tags, 0)?;
    all_params.extend(params);
    let mut query = format!("{} {}", outer_tmpl, subquery);

    if exclude_provided {
        let regular_tags = tags.iter().collect_regular_names();
        let exclude_params = make_params(regular_tags.len(), all_params.len());

        for t in regular_tags {
            all_params.push(Box::new(t.to_string()));
        }
        let outer_where = format!("AND tags.tag_name NOT IN ({})", exclude_params);
        query = format!("{} {}", query, outer_where)
    }

    query = format!("{} GROUP BY tags.id ORDER BY tags.tag_name", query);

    trace!(target: SQL_TAG, "{}", query);
    let isect_tags: Vec<Tag> = conn
        .prepare(&query)?
        .query_map(all_params, to_tag)?
        .collect::<Result<Vec<Tag>>>()?;

    // because our main query is selecting all *tags* based on *file ids*, we may have included tags that don't make
    // sense.  for example, if the tags in question are "b_tags+", it is possible to show tag "a1", which isn't grouped
    // under "b_tags", because "b1" and "a1" might tag the same file, and "b1" is grouped under "b_tags+".  this only
    // really happens in cases where our last tag is a tag group, because otherwise, a tag group is paired with
    // (by immediately preceeding) a regular tag
    if let Some(TagType::Group(last_group)) = tags.last() {
        // evaluate the tag group into the tags it represents
        let tag_groups = tag_names_for_tag_group(conn, last_group)?;
        let mut pruned_tags: Vec<Tag> = vec![];

        // we iterate instead of doing some set difference, because this preserves order from our sql results
        for itag in isect_tags {
            if tag_groups.contains(&itag.name) {
                pruned_tags.push(itag);
            }
        }
        Ok(pruned_tags)
    } else {
        Ok(isect_tags)
    }
}

pub fn add_tag_to_group(
    tx: &Transaction,
    tag: &str,
    tag_group: &str,
    uid: uid_t,
    gid: gid_t,
    permissions: &Permissions,
    now: f64,
) -> Result<()> {
    info!(
        target: SQL_TAG,
        "Adding tag {} to tag group {}", tag, tag_group
    );

    let query = "
INSERT OR IGNORE INTO tag_group_tag (
    tg_id,
    tag_id,
    ts,
    mtime,
    uid,
    gid,
    permissions
) VALUES (
    (SELECT id FROM tag_groups WHERE name=?1),
    (SELECT id FROM tags WHERE tag_name=?2),
    ?3,
    ?3,
    ?4,
    ?5,
    ?6
)";

    trace!(target: SQL_TAG, "{}", query);
    tx.execute(&query, params![tag_group, tag, now, uid, gid, permissions])?;
    Ok(())
}

/// For a `tag_id`, return all of the tag groups it is a part of
pub fn tag_groups_for_tag(conn: &Connection, tag_id: i64) -> Result<Vec<TagGroup>> {
    debug!(target: SQL_TAG, "Getting tag groups for tag id {}", tag_id);

    let query = "
    SELECT
        tg.id,
        tg.name,
        tg.mtime,
        tg.uid,
        tg.gid,
        tg.permissions,
        GROUP_CONCAT(tgt.tag_id, ',')
    FROM tag_groups AS tg
    LEFT JOIN tag_group_tag AS tgt ON tgt.tg_id=tg.id
    WHERE tgt.tag_id=?1
    GROUP BY tg.id
";

    trace!(target: SQL_TAG, "{}", query);

    conn.prepare(&query)?
        .query_map(params![tag_id], to_tag_group)?
        .collect()
}

/// For a `tag_id`, return all of the tag groups it is a part of
pub fn tag_groups_for_tags(conn: &Connection, tag_ids: &[i64]) -> Result<Vec<TagGroup>> {
    info!(
        target: SQL_TAG,
        "Getting tag groups for tag ids {:?}", tag_ids
    );
    if tag_ids.is_empty() {
        Ok(vec![])
    } else {
        let query_tmpl = "SELECT
        tg.id,
        tg.name,
        tg.mtime,
        tg.uid,
        tg.gid,
        tg.permissions,
        GROUP_CONCAT(tgt.tag_id, ',')
    FROM tag_groups AS tg
    LEFT JOIN tag_group_tag AS tgt ON tgt.tg_id=tg.id
    WHERE tgt.tag_id IN ";
        let group_by = "GROUP BY tg.id";

        let in_params = make_params(tag_ids.len(), 0);
        let query = format!("{} ({}) {}", query_tmpl, in_params, group_by);

        trace!(target: SQL_TAG, "{}", query);
        trace!(target: SQL_TAG, "Params: {:?}", tag_ids);

        let res = conn
            .prepare(&query)?
            .query_map(tag_ids, to_tag_group)?
            .collect::<Result<Vec<TagGroup>>>()?;
        debug!(target: SQL_TAG, "Got {} tag groups", res.len());
        Ok(res)
    }
}

pub fn contains_file<P>(conn: &Connection, tags: &[TagType], pred: P) -> Result<Option<TaggedFile>>
where
    P: FnMut(&TaggedFile) -> bool,
{
    debug!(
        target: SQL_TAG,
        "Using predicate to find file in {:?}", tags
    );
    let ifiles = files_tagged_with(conn, tags)?;
    Ok(ifiles.into_iter().find(pred))
}

/// Finds all files that intersect with all of the provided `tags`
pub fn files_tagged_with(conn: &Connection, tags: &[TagType]) -> Result<Vec<TaggedFile>> {
    // FIXME need GROUP to account for null rows
    let outer_tmpl = "
SELECT
    files.id,
    inode,
    device,
    path,
    primary_tag,
    MAX(file_tag.mtime) as mtime,
    file_tag.uid,
    file_tag.gid,
    file_tag.permissions,
    alias_file
FROM files
JOIN file_tag ON file_tag.file_id=files.id
JOIN tags ON file_tag.tag_id=tags.id
WHERE
    file_tag.file_id IN";

    let mut all_params: Vec<Box<dyn ToSql>> = vec![];
    let (subquery, params) = intersection_subquery(conn, tags, 0)?;
    all_params.extend(params);

    let query = format!(
        "{outer} {subquery} GROUP BY files.id ORDER BY primary_tag",
        outer = outer_tmpl,
        subquery = subquery
    );

    trace!(target: SQL_TAG, "{}", query);
    conn.prepare(&query)?
        .query_map(all_params, to_taggedfile)?
        .collect()
}

/// A convenience method that builds a string of sqlite placeholders
fn make_params(num: usize, offset: usize) -> String {
    let mut param_offset = offset + 1;
    let mut params = vec![];
    for _ in 0..num {
        params.push(format!("?{}", param_offset));
        param_offset += 1;
    }
    params.join(",")
}

/// Constructs a correct subquery from tags.  The resulting subquery
/// should be used in a query that ends in "WHERE file_tag.file_id IN {}"
/// The basic idea is that, for regular tags, ie "t1", "t2", etc, we want an INTERSECTion of all file ids tagged with
/// those tags.  For tag groups, ie "t_tags+", we want an INTERSECTion of all files tagged with all tags in the tag
/// groups.  And for NOT tags, ie "-t3", we want to construct an EXCEPT query that excepts the INTERSECTion of all
/// NOT tags.
fn intersection_subquery(
    conn: &Connection,
    tags: &[TagType],
    offset: i32,
) -> Result<(String, Vec<Box<dyn ToSql>>)> {
    debug!(
        target: SQL_TAG,
        "Constructing intersection query from tags {:?} at offset {}", tags, offset
    );

    // first let's separate our intersects from our excepts
    let mut excepts: Vec<Cow<str>> = Vec::new();
    let mut intersects: Vec<Cow<str>> = Vec::new();
    for tag in tags {
        match tag {
            TagType::Regular(name) => intersects.push(Cow::from(name)),
            TagType::Negation(name) => excepts.push(Cow::from(name)),
            TagType::Group(_name) => {}
            _ => {}
        }
    }

    // if our last tag is a tag group, it means we need to consider all of the possible tags underneath it.  otherwise
    // we are essentially ignoring tag groups, since, if a tag group exists in the path and *isn't* the last group, it
    // is immediately followed by a regular tag.  so in that case, we just consider the regular tag and ignore the
    // group altogether.
    let mut groups: Vec<Cow<str>> = Vec::new();
    match tags.last() {
        Some(TagType::Group(last_group)) => {
            // evaluate the tag group into the tags it represents
            let tag_groups = tag_names_for_tag_group(conn, last_group)?;
            groups.extend(tag_groups.into_iter().map(Cow::from))
        }
        _ => {}
    }

    debug!(
        target: SQL_TAG,
        "Exceptions: {:?}, intersections: {:?}", excepts, intersects
    );

    let mut params: Vec<Box<dyn ToSql>> = vec![];

    let group_tmpl = "
SELECT
    file_tag.file_id
FROM file_tag
JOIN tags 
    ON tags.id=file_tag.tag_id
WHERE
    tags.tag_name IN";

    let intersect_tmpl = "
SELECT
    file_tag.file_id
FROM file_tag
JOIN tags 
    ON tags.id=file_tag.tag_id
WHERE
    tags.tag_name=";

    let mut param_offset = offset;

    // now let's intersect all our intersects
    let mut intersect_subqueries: Vec<String> = Vec::new();
    for _ in 0..intersects.len() {
        intersect_subqueries.push(format!("{}?{}", intersect_tmpl, param_offset + 1));
        param_offset += 1;
    }

    // and finally our groups
    let mut group_subqueries: Vec<String> = Vec::new();
    if !groups.is_empty() {
        let group_params = make_params(groups.len(), param_offset as usize);
        let group_subquery = format!("{} ({})", group_tmpl, group_params);
        group_subqueries.push(group_subquery);
        param_offset += groups.len() as i32;
    }

    // and intersect all our excepts
    let mut except_subqueries: Vec<String> = Vec::new();
    for _ in 0..excepts.len() {
        except_subqueries.push(format!("{}?{}", intersect_tmpl, param_offset + 1));
        param_offset += 1;
    }

    let query = if except_subqueries.is_empty() {
        format!(
            "({})",
            intersect_subqueries
                .into_iter()
                .chain(group_subqueries.into_iter())
                .collect::<Vec<_>>()
                .join(" INTERSECT "),
        )
    } else {
        if intersect_subqueries.is_empty() {
            "()".to_string()
        } else {
            format!(
                "(SELECT * FROM ({}) EXCEPT SELECT * FROM ({}))",
                intersect_subqueries
                    .into_iter()
                    .chain(group_subqueries.into_iter())
                    .collect::<Vec<_>>()
                    .join(" INTERSECT "),
                except_subqueries.join(" INTERSECT ")
            )
        }
    };

    for tag in intersects
        .into_iter()
        .chain(groups.into_iter())
        .chain(excepts.into_iter())
    {
        params.push(Box::new(tag.into_owned()));
    }

    Ok((query, params))
}

pub fn add_file(
    tx: &Transaction,
    device_id: u64,
    inode: u64,
    path: &str,
    primary_tag: &str,
    tags: &[&str],
    uid: uid_t,
    gid: gid_t,
    umask: &UMask,
    now: f64,
    alias_file: Option<&str>,
) -> Result<Vec<TaggedFile>> {
    info!(target: SQL_TAG, "Adding file {:?} to tags {:?}", path, tags);

    let query1 = "
INSERT OR IGNORE INTO files (
    device,
    inode,
    path,
    primary_tag,
    ts,
    mtime,
    alias_file
) VALUES (
    ?1,
    ?2,
    ?3,
    ?4,
    ?5,
    ?5,
    ?6
)";
    trace!(target: SQL_TAG, "{}", query1);

    let inserted = tx.execute(
        query1,
        params![
            device_id as i64,
            inode as i64,
            path,
            primary_tag,
            now,
            alias_file
        ],
    )?;
    debug!(
        target: SQL_TAG,
        "Inserted new file record for {}: {}",
        primary_tag,
        inserted > 0
    );

    let mut tagged = Vec::new();
    for &tag in tags {
        debug!(target: SQL_TAG, "Linking to tag {}", tag);

        // auth = authoritative
        let (auth_tag, _) = ensure_tag(tx, tag, uid, gid, &umask.dir_perms(), now)?;
        debug!(target: SQL_TAG, "Resolving tag {} to {}", tag, auth_tag);

        link_file_to_tag(
            tx,
            device_id,
            inode,
            &auth_tag,
            uid,
            gid,
            &umask.file_perms(),
            now,
        )?;

        let tf = TaggedFile {
            id: 0,
            inode,
            device: device_id,
            path: path.into(),
            primary_tag: primary_tag.into(),
            mtime: float_to_utcdt(now),
            uid,
            gid,
            permissions: umask.file_perms().clone(),
            alias_file: alias_file.map(ToOwned::to_owned),
        };

        tagged.push(tf);
    }

    update_root_mtime(tx, now)?;
    Ok(tagged)
}

pub fn purge_devicefile(tx: &Transaction, df: &DeviceFile, now: f64) -> Result<()> {
    info!(target: SQL_TAG, "Purging {:?}", df);

    // update tag count
    let query = "
UPDATE
    tags
SET
    num_files=num_files-1
WHERE
    id IN (
    SELECT
        tag_id
    FROM
        file_tag
    JOIN files
        ON files.id=file_tag.file_id
    WHERE
        files.device=?1
        AND files.inode=?2
)
";
    tx.execute(query, params![df.device as i64, df.inode as i64])?;

    tx.execute(
        "DELETE FROM files WHERE device=?1 AND inode=?2",
        params![df.device as i64, df.inode as i64],
    )?;
    update_root_mtime(tx, now)?;
    Ok(())
}

pub fn purge_path(tx: &Transaction, path: &str, now: f64) -> Result<()> {
    info!(target: SQL_TAG, "Purging {}", path);

    let query = "
UPDATE
    tags
SET
    num_files=num_files-1
WHERE
    id IN (
    SELECT
        tag_id
    FROM
        file_tag
    JOIN files
        ON files.id=file_tag.file_id
    WHERE
        files.path=?1
)
";
    tx.execute(query, params![path])?;

    tx.execute("DELETE FROM files WHERE path=?", params![path])?;
    update_root_mtime(tx, now)?;
    Ok(())
}

pub fn remove_devicefile(
    tx: &Transaction,
    device_file: &DeviceFile,
    tags: &[&str],
    now: f64,
) -> Result<Vec<i64>> {
    info!(
        target: SQL_TAG,
        "Removing inode {} from tags {:?}", device_file.inode, tags
    );
    let file_id: i64 = tx.query_row(
        "SELECT id FROM files WHERE device=?1 AND inode=?2",
        params![device_file.device as i64, device_file.inode as i64],
        |row| row.get(0),
    )?;

    let mut all_removed_ids = vec![];
    for &tag in tags {
        let query1 = "
SELECT rowid
FROM file_tag
WHERE
    file_id=?1
    AND tag_id=(SELECT id FROM tags WHERE tag_name=?2)
";
        let removed_ids = tx
            .prepare(query1)?
            .query_map(params![file_id, tag], |row| Ok(row.get(0)?))?
            .collect::<Result<Vec<i64>>>()?;
        all_removed_ids.extend(&removed_ids);

        tx.execute(
            "DELETE FROM file_tag
                WHERE
                    file_id=?1
                    AND tag_id=(SELECT id FROM tags WHERE tag_name=?2)
            ",
            params![file_id, tag],
        )?;

        if !removed_ids.is_empty() {
            tx.execute(
                "UPDATE tags SET num_files = num_files-?1 WHERE tag_name=?2",
                params![removed_ids.len() as i64, tag],
            )?;
        }
    }
    update_root_mtime(tx, now)?;
    Ok(all_removed_ids)
}

pub fn remove_links(
    tx: &Transaction,
    primary_tag: &str,
    tags: &[TagType],
    now: f64,
) -> Result<Vec<i64>> {
    info!(
        target: SQL_TAG,
        "Removing symlink primary tag {} from tags {:?}", primary_tag, tags
    );

    let mut all_removed_ids = vec![];
    let maybe_tf = contains_file(tx, tags, |tf| &tf.primary_tag == primary_tag)?;
    if let Some(tf) = maybe_tf {
        for tag in tags.iter().collect_regular_names() {
            let query1 = "
SELECT rowid
FROM file_tag
WHERE
    file_id=?1
    AND tag_id=(SELECT id FROM tags WHERE tag_name=?2)
            ";
            let removed_ids = tx
                .prepare(query1)?
                .query_map(params![tf.id, tag], |row| Ok(row.get(0)?))?
                .collect::<Result<Vec<i64>>>()?;
            all_removed_ids.extend(&removed_ids);

            let query2 = "
DELETE FROM file_tag
WHERE
    file_id=?1
    AND tag_id=(SELECT id FROM tags WHERE tag_name=?2)
            ";
            trace!(target: SQL_TAG, "{}", query2);
            let changed = tx.execute(query2, params![tf.id, tag])?;
            debug!(target: SQL_TAG, "Changed {} rows", changed);

            if changed > 0 {
                debug!(
                    target: SQL_TAG,
                    "Updating {} num_files by -{}", tag, changed
                );
                tx.execute(
                    "UPDATE tags SET num_files = num_files-?1 WHERE tag_name=?2",
                    params![changed as i64, tag],
                )?;
            }
        }
        update_root_mtime(tx, now)?;
    } else {
        warn!(
            target: SQL_TAG,
            "Couldn't find symlink to remove for {} at {:?}", primary_tag, tags
        );
    }

    Ok(all_removed_ids)
}

pub fn get_root_mtime(conn: &Connection) -> Result<UtcDt> {
    Ok(conn
        .query_row("SELECT root_mtime FROM supertag_meta", NO_PARAMS, |row| {
            Ok(float_to_utcdt(row.get(0)?))
        })
        .optional()?
        .unwrap_or_else(|| chrono::Utc::now()))
}

fn update_root_mtime(tx: &Transaction, now: f64) -> Result<usize> {
    debug!(target: SQL_TAG, "Updating root mtime to {}", now);
    tx.execute("UPDATE supertag_meta SET root_mtime=?1", params![now])
}

pub fn get_tag_id(conn: &Connection, tag: &str) -> Result<Option<i64>> {
    debug!(target: SQL_TAG, "Getting tag id for {}", tag);
    conn.query_row(
        "SELECT id FROM tags WHERE tag_name=?1",
        params![tag],
        |row| Ok(row.get(0)?),
    )
    .optional()
}

pub fn get_tag_group_id(conn: &Connection, group: &str) -> Result<Option<i64>> {
    debug!(target: SQL_TAG, "Getting group tag id for {}", group);
    conn.query_row(
        "SELECT id FROM tag_groups WHERE name=?1",
        params![group],
        |row| Ok(row.get(0)?),
    )
    .optional()
}

pub fn remove_taggroup(tx: &Transaction, group: &str) -> Result<()> {
    info!(target: SQL_TAG, "Deleting tag group {}", group);
    let query = "DELETE FROM tag_groups WHERE name=?1";
    tx.execute(&query, params![group])?;
    Ok(())
}

pub fn remove_taggroup_from_itersection(
    tx: &Transaction,
    group: &str,
    intersect: &[TagType],
) -> Result<()> {
    info!(
        target: SQL_TAG,
        "Deleting tag group {} from tag intersection {:?}", group, intersect
    );

    let tg_id = get_tag_group_id(tx, group)?.ok_or(rusqlite::Error::QueryReturnedNoRows)?;
    let isect_tags = intersect_tag(tx, intersect, true)?;
    for chunk in isect_tags.chunks(500) {
        let ids = chunk
            .iter()
            .map(|t| t.id.to_string())
            .collect::<Vec<String>>()
            .join(",");

        let query = format!(
            "
            DELETE FROM tag_group_tag
            WHERE tag_id IN ({})
            AND tg_id=?1",
            ids
        );
        trace!(target: SQL_TAG, "{}", query);

        tx.execute(&query, params![tg_id as i64])?;
    }

    Ok(())
}

pub fn remove_tag_from_intersection(
    tx: &Transaction,
    tag: &str,
    intersect: &[TagType],
    now: f64,
) -> Result<Vec<TaggedFile>> {
    info!(
        target: SQL_TAG,
        "Deleting tag {} from tag intersection {:?}", tag, intersect
    );
    let mut total_removed = 0;
    let files = files_tagged_with(tx, intersect)?;
    let tag_id = get_tag_id(tx, tag)?.ok_or(rusqlite::Error::QueryReturnedNoRows)?;

    // let's do our deletes in chunks so we don't blow up sqlite
    for chunk in files.chunks(500) {
        let ids = chunk
            .iter()
            .map(|f| f.id.to_string())
            .collect::<Vec<String>>()
            .join(",");

        let query = format!(
            "
            DELETE FROM file_tag
            WHERE file_id IN ({})
            AND tag_id=?1",
            ids
        );
        trace!(target: SQL_TAG, "{}", query);

        let removed = tx.execute(&query, params![tag_id])?;
        total_removed += removed;

        tx.execute(
            "UPDATE tags SET num_files = num_files-?1 WHERE id=?2",
            params![removed as i64, tag_id],
        )?;
    }
    update_root_mtime(tx, now)?;
    debug!(
        target: SQL_TAG,
        "Removed {} file associations", total_removed
    );

    Ok(files)
}

/// Removes a tag from the database and cascades the delete to all file-tag associations.
pub fn remove_tag(tx: &Transaction, tag: &str, now: f64, immediate: bool) -> Result<()> {
    info!(
        target: SQL_TAG,
        "Deleting tag {}, immediate: {}", tag, immediate
    );

    // TODO is immediate required anymore?
    if immediate {
        let query1 = "DELETE FROM tags WHERE tag_name=?1";
        trace!(target: SQL_TAG, "{}", query1);
        let num_tags = tx.execute(query1, params![tag])?;
        debug!(target: SQL_TAG, "Deleted {} tags", num_tags);
    } else {
        let query1 = "UPDATE tags SET rm_time=?2 WHERE tag_name=?1";
        trace!(target: SQL_TAG, "{}", query1);
        let num_tags = tx.execute(query1, params![tag, now])?;
        debug!(target: SQL_TAG, "Updated {} tags", num_tags);

        let query2 = "
DELETE FROM file_tag
WHERE file_tag.tag_id=(
    SELECT id
    FROM tags
    WHERE tag_name=?1
)
    ";
        trace!(target: SQL_TAG, "{}", query2);
        let num_links = tx.execute(query2, params![tag])?;
        debug!(target: SQL_TAG, "Updated {} links", num_links);
    }

    update_root_mtime(tx, now)?;
    Ok(())
}

/// Renames a tag
pub fn rename_tag(tx: &Transaction, old_tag: &str, new_tag: &str, now: f64) -> Result<()> {
    info!(target: SQL_TAG, "Renaming tag {} to {}", old_tag, new_tag);
    tx.execute(
        "UPDATE tags SET tag_name=?1 WHERE tag_name=?2",
        params![new_tag, old_tag],
    )?;

    update_tag_mtime(tx, new_tag, now)?;
    update_root_mtime(tx, now)?;
    Ok(())
}

pub fn rename_tag_group(tx: &Transaction, old_name: &str, new_name: &str, now: f64) -> Result<()> {
    info!(
        target: SQL_TAG,
        "Renaming tag group {} to {}", old_name, new_name
    );
    tx.execute(
        "UPDATE tag_groups SET name=?1 WHERE name=?2",
        params![new_name, old_name],
    )?;
    update_tag_group_mtime(tx, new_name, now)?;
    update_root_mtime(tx, now)?;
    Ok(())
}

/// Takes everything tagged with the intersection of `src_tags` and removes the last src_tag from it, retagging all
/// of those files with every tag in `dst_tags`
pub fn merge_tags(
    tx: &Transaction,
    src_tag: &str,
    src_tags: &[TagType],
    dst_tags: &[&str],
    now: f64,
) -> Result<()> {
    info!(
        target: SQL_TAG,
        "Merging tag intersection {:?} into {:?}", src_tags, dst_tags
    );

    let removed = remove_tag_from_intersection(tx, src_tag, src_tags, now)?;
    debug!(
        target: SQL_TAG,
        "Deleted-to-be-moved {} files",
        removed.len()
    );

    // now for each destination tag, we'll copy our file_tag associations and replace the tag_id
    // field with our dst_tag tag id.  we'll also update the different timestamps to reflect that
    // these files were merged now
    for &new_tag in dst_tags {
        for tf in &removed {
            tx.execute(
                "INSERT OR IGNORE INTO file_tag (
                    file_id,
                    tag_id,
                    ts,
                    mtime,
                    uid,
                    gid,
                    permissions
                )
                VALUES (
                    ?1,
                    (SELECT id from tags WHERE tag_name=?2),
                    ?3,
                    ?4,
                    ?5,
                    ?6,
                    ?7
                )
            ",
                params![tf.id, new_tag, now, now, tf.uid, tf.gid, tf.permissions],
            )?;
        }

        update_tag_mtime(tx, new_tag, now)?;
    }

    update_root_mtime(tx, now)?;
    Ok(())
}

/// Renames a tag file
pub fn rename_file(
    tx: &Transaction,
    device_file: &DeviceFile,
    new_name: &str,
    now: f64,
) -> Result<()> {
    info!(
        target: SQL_TAG,
        "Renaming file {:?} to {}", device_file, new_name
    );
    tx.execute(
        "UPDATE files SET
        primary_tag=?1,
        mtime=?4
        WHERE device=?2 AND inode=?3",
        params![
            new_name,
            device_file.device as i64,
            device_file.inode as i64,
            now
        ],
    )?;
    // TODO update the mtimes of all tags that contain this file
    update_root_mtime(tx, now)?;
    Ok(())
}

pub fn pin_tags(
    tx: &Transaction,
    tags: &[TagType],
    uid: uid_t,
    gid: gid_t,
    permissions: &Permissions,
    now: f64,
) -> Result<()> {
    info!(target: SQL_TAG, "Pinning {:?}", tags);

    let mut pin_ids = vec![];
    for tt in tags {
        match tt {
            TagType::Regular(tag) => {
                ensure_tag(tx, tag, uid, gid, permissions, now)?;
                let tag_id = get_tag_id(tx, tag)?.ok_or(rusqlite::Error::QueryReturnedNoRows)?;
                pin_ids.push(format!("t{}", tag_id));
            }
            TagType::Group(group) => {
                ensure_tag_group(tx, group, uid, gid, permissions, now)?;
                let group_id =
                    get_tag_group_id(tx, group)?.ok_or(rusqlite::Error::QueryReturnedNoRows)?;
                pin_ids.push(format!("g{}", group_id));
            }
            _ => {
                // FIXME make this nicer
                panic!("Can't pin anything except regular or group tags");
            }
        }
    }

    // FIXME this should be where the caller is
    // here we're determining if we need to put a tag into a tag group.  this can be the case if we're manually creating
    // a tag inside of a tag group dir in a file browser.  in that case, the mkdir path might look something like
    // `something/else/+tag_group/tag`, in which case we want `tag` to be put into `+tag_group`.
    if tags.len() >= 2 {
        let last = &tags[tags.len() - 1];
        let second_to_last = &tags[tags.len() - 2];
        if let (TagType::Regular(tag), TagType::Group(group)) = (last, second_to_last) {
            debug!(
                target: SQL_TAG,
                "{} is being pinned under tag group {}, grouping", tag, group
            );
            add_tag_to_group(tx, tag, group, uid, gid, permissions, now)?;
        }
    }

    let mut joined_pin_ids: String = pin_ids.join("/");
    joined_pin_ids.push_str("/");
    trace!(target: SQL_TAG, "Pin insertion params: {}", &joined_pin_ids);

    tx.execute(
        "INSERT INTO pins (tag_ids) VALUES (?1)",
        params![joined_pin_ids],
    )?;
    Ok(())
}

pub fn is_pinned(conn: &Connection, tags: &[TagType]) -> Result<bool> {
    let maybe_joined_tag_ids = build_pintag_prefix(conn, tags)?;
    if let Some(joined_tag_ids) = maybe_joined_tag_ids {
        Ok(conn
            .query_row(
                "SELECT 1 FROM pins WHERE tag_ids LIKE ?1",
                params![joined_tag_ids],
                |_| Ok(true),
            )
            .optional()?
            .is_some())
    } else {
        Ok(false)
    }
}

fn build_pintag_record(conn: &Connection, tags: &[TagType]) -> Result<Option<String>> {
    let mut joined_tag_ids = "".to_string();
    let mut pin_ids = vec![];
    for tt in tags {
        match tt {
            TagType::Regular(tag) => {
                let maybe_tag_id = get_tag_id(conn, tag)?;
                match maybe_tag_id {
                    Some(tag_id) => pin_ids.push(format!("t{}", tag_id)),
                    None => return Ok(None),
                }
            }
            TagType::Group(group) => {
                let maybe_group_id = get_tag_group_id(conn, group)?;
                match maybe_group_id {
                    Some(group_id) => pin_ids.push(format!("g{}", group_id)),
                    None => return Ok(None),
                }
            }
            _ => {}
        }
    }

    // even though we've used forward slashes to separate our tag ids in `pin_tags`, we can't use
    // those here without a lot of fancy quoting.  we'll use another token character, the
    // underscore, instead, and everything works fine
    joined_tag_ids.extend(pin_ids.join("/").chars());
    joined_tag_ids.push_str("/");
    Ok(Some(joined_tag_ids))
}

fn build_pintag_prefix(conn: &Connection, tags: &[TagType]) -> Result<Option<String>> {
    debug!(target: SQL_TAG, "Building pin prefix query for {:?}", tags);
    let mut joined_tag_ids = String::new();
    let mut pin_ids = vec![];
    for tt in tags {
        match tt {
            TagType::Regular(tag) => {
                let maybe_tag_id = get_tag_id(conn, tag)?;
                match maybe_tag_id {
                    Some(tag_id) => pin_ids.push(format!("t{}", tag_id)),
                    None => return Ok(None),
                }
            }
            TagType::Group(group) => {
                let maybe_group_id = get_tag_group_id(conn, group)?;
                match maybe_group_id {
                    Some(group_id) => pin_ids.push(format!("g{}", group_id)),
                    None => return Ok(None),
                }
            }
            _ => {}
        }
    }

    joined_tag_ids.extend(pin_ids.join("/").chars());
    joined_tag_ids.push_str("/%");

    trace!(target: SQL_TAG, "Pin prefix query: {}", joined_tag_ids);
    Ok(Some(joined_tag_ids))
}

/// For a given tag path, find all other tags that are pinned underneath.  This is required when
/// we're doing a readdir (ls) on a directory, and another tag directory, which is empty but pinned,
/// should be listed
pub fn pinned_subdirs(conn: &Connection, tags: &[TagType]) -> Result<Vec<TagOrTagGroup>> {
    debug!(target: SQL_TAG, "Looking for pinned subdirs for {:?}", tags);
    let maybe_joined_tag_ids = build_pintag_prefix(conn, tags)?;

    let mut records = vec![];

    if let Some(joined_tag_ids) = maybe_joined_tag_ids {
        // find all of the pin entries that start with our `tags` prefix
        let all_tag_ids: Vec<String> = conn
            .prepare("SELECT tag_ids FROM pins WHERE tag_ids LIKE ?1")?
            .query_map(params![joined_tag_ids], |row: &Row| -> Result<String> {
                Ok(row.get(0)?)
            })?
            .collect::<Result<Vec<String>>>()?;

        debug!(
            target: SQL_TAG,
            "Got {} potential pin results",
            all_tag_ids.len()
        );
        let parse_id = |chunk: &str| {
            chunk
                .chars()
                .skip(1)
                .collect::<String>()
                .parse::<i64>()
                .ok()
        };

        if let Some(strip_prefix) = build_pintag_record(conn, tags)? {
            // for each pin entry, we need to parse out the *first* subdirectory in the entry, after the `tags`, and turn
            // that into either a tag group or a tag.  we're essentially building the collection of all immediate pinned
            // descendants of `tags`, and each of those entries will either be a tag group or a tag
            for ref tag_id_str in all_tag_ids {
                // parse our record either a tag or a tag group.  we use a substring slice because we
                // we only want to deal with the tags that are not in the `tags` that we passed into this function
                if let Some(chunk) = tag_id_str[strip_prefix.len()..].split("/").nth(0) {
                    trace!(
                        target: SQL_TAG,
                        "Looking to parse {} for valid pin subdirs",
                        chunk
                    );

                    match chunk.chars().nth(0) {
                        Some('g') => {
                            if let Some(group_id) = parse_id(chunk) {
                                match get_tag_group_by_id(conn, group_id)? {
                                    Some(group) => records.push(TagOrTagGroup::Group(group)),
                                    _ => {}
                                }
                            }
                        }
                        Some('t') => {
                            if let Some(tag_id) = parse_id(chunk) {
                                match get_tag_by_id(conn, tag_id)? {
                                    Some(tag) => records.push(TagOrTagGroup::Tag(tag)),
                                    _ => {}
                                }
                            }
                        }
                        Some(_) | None => {}
                    }
                }
            }
        }
    }
    Ok(records)
}

pub fn tag_names_for_tag_group(conn: &Connection, group: &str) -> Result<HashSet<String>> {
    let query = "SELECT
            tags.tag_name
        FROM tags
        JOIN tag_group_tag AS tgt ON tgt.tag_id=tags.id
        JOIN tag_groups AS tg ON tg.id=tgt.tg_id
        WHERE tg.name=?1";
    conn.prepare(&query)?
        .query_map(params![group], |row| row.get(0))?
        .collect()
}

pub fn tag_is_in_group(conn: &Connection, group: &str, tag: &str) -> Result<bool> {
    for tag_in_group in tag_names_for_tag_group(conn, group)? {
        if tag == &tag_in_group {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_float_to_dt_and_back() {
        let now = get_now_secs();
        let dt = float_to_utcdt(now);
        assert_eq!(now as i64, dt.timestamp());
    }
}
