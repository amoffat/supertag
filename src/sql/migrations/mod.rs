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
use log::debug;
use rusqlite::{params, OptionalExtension, Transaction, TransactionBehavior, NO_PARAMS};
use rusqlite::{Connection, Result as SqliteResult};

mod m0;
type MigrationFunction = Box<dyn Fn(&Transaction) -> SqliteResult<()>>;

const TAG: &str = "migrations";

pub fn migrate(conn: &mut Connection, app_version: &str) -> SqliteResult<()> {
    let maybe_table: Option<String> = conn
        .query_row(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='supertag_meta'",
            NO_PARAMS,
            |row| Ok(row.get(0)?),
        )
        .optional()?;

    // no tables? create
    if maybe_table.is_none() {
        debug!(target: TAG, "Running initial migration");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Exclusive)?;
        m0::migrate(&tx)?;
        let _res = tx.commit();
    }

    let migration_version: i64 = conn.query_row(
        "SELECT migration_version FROM supertag_meta",
        NO_PARAMS,
        |row| Ok(row.get(0)?),
    )?;
    debug!(
        target: TAG,
        "Currently on database version {}", migration_version
    );

    #[allow(unused_mut)]
    let mut migrations: Vec<MigrationFunction> = vec![];
    //migrations.push(m1::migrate);

    for (i, mig) in migrations
        .iter()
        .skip(migration_version as usize)
        .enumerate()
    {
        debug!(
            target: TAG,
            "Running migration {}",
            (i as i64) + migration_version
        );
        let tx = conn.transaction_with_behavior(TransactionBehavior::Exclusive)?;
        mig(&tx)?;
        let _res = tx.execute(
            "UPDATE supertag_meta SET migration_version=?1",
            params![(i as i64) + migration_version],
        )?;
        tx.commit()?;
    }

    let _res = conn.execute(
        "UPDATE supertag_meta SET supertag_version=?1",
        params![app_version],
    )?;

    Ok(())
}
