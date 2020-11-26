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
use super::{Listener, Notifier};
use crate::common::types::note::Note;
use log::{debug, error, info, trace, warn};
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::error::Error;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread::spawn;
use std::time::{Duration, Instant};

// how many historical messages a peer will store and be allowed to traverse
const PEER_BUFFER: usize = 10_000;

pub struct UDSNotifier {
    tag: String,
    peers: Arc<Mutex<Vec<Sender<Note>>>>,
    socket_file: PathBuf,
    bound: bool,
}

fn handle_conn(conn_id: uuid::Uuid, mut stream: UnixStream, rx: Receiver<Note>) {
    let tag = format!("uds-conn-{}", conn_id.to_hyphenated().to_string());
    for note in rx {
        debug!(target: &tag, "Sending note {:?} to peer", note);
        let mut blob = serde_json::to_vec(&note).unwrap();
        blob.push(b'\n');
        match stream.write_all(blob.as_slice()) {
            Err(e) => {
                error!(target: &tag, "Error writing note to peer: {:?}", e);
                return;
            }
            Ok(_) => {
                debug!(target: &tag, "Successfully sent {:?} to peer", note);
            }
        }
    }
    debug!(target: &tag, "Connection TX closed");
}

impl UDSNotifier {
    /// If `bind` is false, we won't actually bind to the socket file. This is needed in cases
    /// where the cli needs to create a `UDSNotifier` purely to get access to `.listener()`, but
    /// we don't want to bind to the socket file, because the mount process has already done that.
    /// FIXME though, this is wonky because some cli handlers should be able to put messages onto
    /// the notifier
    pub fn new(socket_file: PathBuf, bind: bool) -> std::io::Result<Self> {
        let tag = "uds-notifier";
        let peers = Arc::new(Mutex::new(Vec::new()));

        if bind {
            if socket_file.exists() {
                warn!(
                    target: tag,
                    "Notifier socket file {} exists, removing first",
                    &socket_file.display()
                );
                std::fs::remove_file(&socket_file)?;
            }

            let socket = UnixListener::bind(&socket_file)?;

            let peers_t1 = peers.clone();
            spawn(move || {
                let tag = "uds-conn-listener";
                debug!(target: tag, "Starting listener thread");

                for maybe_stream in socket.incoming() {
                    match maybe_stream {
                        Ok(stream) => {
                            let conn_id = uuid::Uuid::new_v4();
                            debug!(target: tag, "Got a new connection {}", conn_id);
                            let (tx, rx): (Sender<Note>, _) = channel();
                            let mut guard = peers_t1.lock();
                            guard.push(tx);
                            spawn(move || handle_conn(conn_id, stream, rx));
                        }
                        Err(e) => error!(target: tag, "Error getting peer connection: {:?}", e),
                    }
                }
                debug!(target: tag, "Exiting thread");
            });
        }

        Ok(Self {
            tag: tag.to_string(),
            peers,
            socket_file,
            bound: bind,
        })
    }

    fn send_message(&self, note: Note) -> Result<(), Box<dyn Error>> {
        if self.bound {
            let mut guard = self.peers.lock();

            // send our note to our peers, but if one has a problem, remove the peer
            guard.retain(|peer| match peer.send(note.clone()) {
                Err(e) => {
                    error!(target: &self.tag, "Couldn't send note to peer, skipping: {:?}", e);
                    false
                }
                Ok(_) => {
                    debug!(target: &self.tag, "Sent to peer");
                    true
                }
            });
        } else {
            warn!(target: &self.tag, "Notifier isn't bound, skipping sending message");
        }

        Ok(())
    }
}

impl Notifier for UDSNotifier {
    type Listener = UDSListener;

    fn bad_copy(&self) -> Result<(), Box<dyn Error>> {
        info!(target: &self.tag, "bad_copy");
        self.send_message(Note::BadCopy)?;
        Ok(())
    }

    fn dragged_to_root(&self) -> Result<(), Box<dyn Error>> {
        info!(target: &self.tag, "dragged_to_root");
        self.send_message(Note::DraggedToRoot)?;
        Ok(())
    }

    fn unlink(&self, path: &Path) -> Result<(), Box<dyn Error>> {
        info!(target: &self.tag, "unlink");
        self.send_message(Note::Unlink(path.to_owned()))?;
        Ok(())
    }

    fn tag_to_tg(&self, tag: &str) -> Result<(), Box<dyn Error>> {
        info!(target: &self.tag, "tag_to_tg");
        self.send_message(Note::TagToTagGroup(tag.to_owned()))?;
        Ok(())
    }

    fn listener(&self) -> Result<Self::Listener, Box<dyn Error>> {
        Ok(UDSListener::new(self.socket_file.clone())?)
    }
}

pub struct UDSListener {
    tag: String,
    buffer: Arc<Mutex<VecDeque<(usize, Note)>>>,
    done: Arc<AtomicBool>,
}

impl Drop for UDSListener {
    fn drop(&mut self) {
        self.done.store(true, Ordering::Relaxed);
    }
}

impl UDSListener {
    pub fn new(socket_file: PathBuf) -> std::io::Result<Self> {
        let tag = "uds-listener";

        debug!(target: tag, "Attempting connection to {:?}", socket_file);
        let socket = BufReader::new(UnixStream::connect(&socket_file)?);
        debug!(target: tag, "Made connection to {:?}", socket_file);
        let buffer = Arc::new(Mutex::new(VecDeque::with_capacity(PEER_BUFFER)));
        let done = Arc::new(AtomicBool::new(false));

        let thread_buffer = buffer.clone();
        let thread_done = done.clone();
        spawn(move || UDSListener::aggregate(socket, thread_buffer, thread_done));

        Ok(Self {
            tag: tag.to_string(),
            buffer,
            done,
        })
    }

    fn aggregate(
        mut socket: BufReader<UnixStream>,
        buffer: Arc<Mutex<VecDeque<(usize, Note)>>>,
        done: Arc<AtomicBool>,
    ) {
        let tag = "uds-listener-thread";
        debug!(target: tag, "Starting aggregate thread");

        // unintuitive, but we offset the counter by 1 (making index 0 have counter 1), so that
        // our .marker() method returns 0 when the buffer is empty, and it makes sense
        let mut counter: usize = 1;

        while !done.load(Ordering::Relaxed) {
            debug!(target: tag, "Listening for line");

            // get our line
            let mut line = String::new();
            if let Err(e) = socket.read_line(&mut line) {
                error!(target: tag, "Problem reading line: {:?}", e);
                std::thread::sleep(std::time::Duration::from_millis(100));
                continue;
            }

            debug!(target: tag, "Got: {}", line.trim());

            // decode our line into a note
            let maybe_cand: serde_json::Result<Note> = serde_json::from_str(&line);
            match maybe_cand {
                Ok(cand) => {
                    let mut guard = buffer.lock();
                    guard.push_back((counter, cand.clone()));
                    counter += 1;

                    if guard.len() >= PEER_BUFFER {
                        guard.pop_front();
                    }
                }
                Err(e) => {
                    error!(target: tag, "Problem deserializing note: {:?}", e);
                }
            }
        }

        debug!(target: tag, "Done aggregating");
    }
}

impl Listener for UDSListener {
    fn marker(&self) -> usize {
        let guard = self.buffer.lock();
        match guard.back() {
            Some(val) => val.0,
            None => 0,
        }
    }

    fn wait_for_pred(
        &mut self,
        pred: impl Fn(&Note) -> bool,
        timeout: Duration,
        idx: usize,
    ) -> Option<(Note, usize)> {
        let start = Instant::now();
        info!(target: &self.tag, "Waiting for note via predicate, from idx {}", idx);

        loop {
            {
                let guard = self.buffer.lock();
                for (cand_idx, cand) in guard.iter() {
                    trace!(target: &self.tag, "Checking {:?} (idx: {})", cand, cand_idx);
                    if *cand_idx <= idx {
                        trace!(target: &self.tag, "Index is in the past, discarding");
                        continue;
                    }

                    if pred(cand) {
                        info!(target: &self.tag, "Found note {:?}", cand);
                        return Some((cand.to_owned(), *cand_idx));
                    }
                }
            }

            // check if we've timed out
            let elapsed = Instant::now() - start;
            if elapsed > timeout {
                warn!(target: &self.tag, "Timeout looking for note");
                return None;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }

    fn wait_for(&mut self, note: &Note, timeout: Duration, idx: usize) -> bool {
        info!(target: &self.tag, "Waiting for note {:?}", note);
        self.wait_for_pred(|cand| cand == note, timeout, idx)
            .is_some()
    }
}
