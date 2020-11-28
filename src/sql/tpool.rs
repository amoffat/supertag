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

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;
use std::thread::ThreadId;

use crate::sql;
use log::trace;
use parking_lot::{Mutex, RwLock};
use rusqlite::Connection;
use std::path::PathBuf;

// the pool is shared amongst threads, hence Arc
// we want it to be safe & fast, and most of our access is Read, so RwLock
// Arc for Hashmap value because it is shared, though not outside of this thread
// Mutex because the RefCell has interior mutability, and different threads could (in theory) have a reference to the same value
// RefCell because creating a transaction requires a mutable &Connection
type ConnMap = Arc<RwLock<HashMap<ThreadId, Arc<Mutex<RefCell<Connection>>>>>>;

const TAG: &str = "db_thread_pool";

/// This structure lazily creates unique database connections for the current thread that we're in.
/// These connections are re-used and have strict thread-affinity.
pub struct ThreadConnPool {
    pool: ConnMap,
    db_path: PathBuf,
}

impl ThreadConnPool {
    pub fn new(db_path: PathBuf) -> Self {
        Self {
            pool: Arc::new(RwLock::new(HashMap::new())),
            db_path,
        }
    }

    pub fn raw_conn(&self) -> Connection {
        sql::get_conn(&self.db_path).expect("Couldn't create db connection")
    }

    pub fn get_conn(&self) -> Arc<Mutex<RefCell<Connection>>> {
        let tid = std::thread::current().id();
        trace!(target: TAG, "Attempting to get a db connection");

        // let's look for a connection for our current thread
        trace!(target: TAG, "Acquiring read lock...");
        let read_guard = self.pool.read();
        trace!(target: TAG, "Got read lock");

        match read_guard.get(&tid) {
            // we have one already?  just clone the Arc
            Some(val) => {
                trace!(target: TAG, "Found an existing db connection");
                Arc::clone(val)
            }
            // no connection?  we need to create one.
            None => {
                trace!(target: TAG, "No existing db connection, creating");

                // this may seem like a race condition, because we're dropping the read lock, then
                // acquiring the write lock, and in theory, another thread may have changed the
                // hash map.  this doesn't matter however, because another thread is only ever
                // inserting a new Connection object *for its own thread id*, so there's no way
                // there will be a collision
                trace!(target: TAG, "Dropping read lock");
                drop(read_guard);

                trace!(target: TAG, "Creating db connection");
                let new_raw_conn = self.raw_conn();
                trace!(target: TAG, "Created db connection");

                let new_conn = Arc::new(Mutex::new(RefCell::new(new_raw_conn)));
                trace!(target: TAG, "Acquiring write lock...");
                let mut write_guard = self.pool.write();
                trace!(target: TAG, "Got write lock");
                write_guard.insert(tid, Arc::clone(&new_conn));
                new_conn
            }
        }
    }
}
