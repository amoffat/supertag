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

use log::{Metadata, Record};
use parking_lot::Mutex;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::fs::{File, OpenOptions};
use std::io::prelude::*;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

pub static REQ_COUNTER: AtomicUsize = AtomicUsize::new(0);
thread_local!(pub static REQUEST_ID: RefCell<usize> = RefCell::new(0));

struct RotatingState {
    stream: BufWriter<File>,
    cur_log: PathBuf,
    archives: VecDeque<PathBuf>,
    num_backups: usize,
}

impl RotatingState {
    pub fn rotate(&mut self, new_log: PathBuf, h: File) -> std::io::Result<()> {
        self.archives.push_back(self.cur_log.clone());
        self.cur_log = new_log;

        if self.archives.len() > self.num_backups {
            // notice we're only ever deleting files that we know *for certain* that we have created. this is erring
            // on the side of caution, because it is possible to leave around undeleted files each time we restart
            // the process
            let to_rm = self.archives.pop_front().unwrap();
            std::fs::remove_file(to_rm)?;
        }

        self.stream.flush()?;
        self.stream = BufWriter::new(h);
        Ok(())
    }
}

/// A date-based rotating logger (similar to fern::DateBased) but also deletes old log files. However, it only deletes
/// log files that it itself created in this process. In other words, it will not delete old log files when the process
/// relaunches.
pub struct RotatingLogger {
    rotate_check: u64,
    counter: AtomicU64,
    state: Mutex<RotatingState>,
    log_dir: PathBuf,
    fmt: String,
}

impl RotatingLogger {
    pub fn new(
        log_dir: PathBuf,
        fmt: String,
        num_backups: usize,
        rotate_check: u64,
    ) -> std::io::Result<Self> {
        // Gets the oldest log files and deletes them. This is only done on startup, to cleanup old files from the
        // last run. It is not called as part of a regular rotate, as its easier to keep track of those files in-memory
        // than read the filesystem
        let oldest = RotatingLogger::get_oldest_logs(&log_dir)?;
        let overage = oldest.len() as i32 - num_backups as i32;
        if overage > 0 {
            for to_rm in &oldest[..overage as usize] {
                std::fs::remove_file(to_rm)?;
            }
        }

        let log_path = RotatingLogger::generate_name(&log_dir, &fmt);
        let h = RotatingLogger::open(&log_path)?;

        let logger = RotatingLogger {
            rotate_check,
            counter: AtomicU64::new(0),
            state: Mutex::new(RotatingState {
                stream: BufWriter::new(h),
                cur_log: log_path,
                archives: VecDeque::new(),
                num_backups,
            }),
            log_dir,
            fmt,
        };

        Ok(logger)
    }

    pub fn generate_name(log_dir: &Path, fmt: &str) -> PathBuf {
        let filename = chrono::Utc::now().format(fmt).to_string();
        log_dir.join(filename)
    }

    fn open(path: &Path) -> std::io::Result<File> {
        OpenOptions::new()
            .write(true)
            .append(true)
            .create(true)
            .open(path)
    }

    fn get_oldest_logs(log_dir: &Path) -> std::io::Result<Vec<PathBuf>> {
        let mut entries: Vec<(PathBuf, std::time::SystemTime)> = std::fs::read_dir(log_dir)?
            .filter_map(|e| e.ok())
            .filter_map(|e| {
                // notice, this check keeps us from deleting non-log files.  yes, it is inflexible, but i'd rather be
                // inflexible for the time being than accidentally delete something i shouldn't
                // FIXME
                if e.file_name().to_string_lossy().ends_with(".log") {
                    Some(e)
                } else {
                    None
                }
            })
            .filter_map(|e| {
                let maybe_t = e.metadata().and_then(|md| md.created()).ok();
                if let Some(t) = maybe_t {
                    Some((e.path(), t))
                } else {
                    None
                }
            })
            .collect();

        entries.sort_by_cached_key(|(_path, btime)| btime.to_owned());
        Ok(entries
            .iter()
            .map(|(path, _btime)| path.to_owned())
            .collect())
    }
}

impl log::Log for RotatingLogger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        let count = self.counter.fetch_add(1, Ordering::Relaxed);
        let mut state = self.state.lock();
        if count % self.rotate_check == 0 {
            // is it time to use a new log file?
            let new_log = RotatingLogger::generate_name(&self.log_dir, &self.fmt);
            if state.cur_log != new_log {
                match RotatingLogger::open(&new_log) {
                    Ok(h) => {
                        if let Err(e) = state.rotate(new_log, h) {
                            eprintln!("Couldn't rotate log: {:?}", e);
                        }
                    }
                    Err(e) => eprintln!("Couldn't allocate new log file: {:?}", e),
                };
            }
        }
        if let Err(e) = write!(state.stream, "{}\n", record.args()) {
            eprintln!("Couldn't write record to stream: {:?}", e);
        }

        if let Err(e) = state.stream.flush() {
            eprintln!("Couldn't flush log: {:?}", e);
        }
    }

    fn flush(&self) {
        let mut state = self.state.lock();
        if let Err(e) = state.stream.flush() {
            eprintln!("Couldn't flush log: {:?}", e);
        }
    }
}

pub fn setup_logger(
    level: log::LevelFilter,
    outputs: Vec<fern::Output>,
) -> Result<(), fern::InitError> {
    let mut logger = fern::Dispatch::new()
        .format(move |out, message, record| {
            REQUEST_ID.with(|req_id| {
                out.finish(format_args!(
                    "{}[Thread: {:?}][Request: {}][{}][{}] {}",
                    chrono::Local::now().format("[%Y-%m-%d][%H:%M:%S][%s%.3f]"),
                    std::thread::current().id(),
                    *req_id.borrow(),
                    record.target(),
                    record.level(),
                    message
                ))
            });
        })
        .level(level);

    for output in outputs {
        logger = logger.chain(output);
    }

    logger.apply()?;

    Ok(())
}
