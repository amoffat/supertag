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

#![allow(dead_code)]

use crate::common::dirs::TestDirectories;
use chrono::TimeZone;
use fuse_sys::{mount, MountHandle};
use libc::{gid_t, uid_t};
use log::{debug, error, info, LevelFilter};
use parking_lot::Mutex;
#[cfg(target_os = "macos")]
use rand::Rng;
use rusqlite::{Connection, TransactionBehavior};
use serde::export::Formatter;
use std::collections::HashSet;
use std::error::Error;
use std::fs;
use std::io::{ErrorKind, Result as IOResult};
use std::iter::FromIterator;
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Once;
use std::time::Duration;
use supertag::common::err::STagResult;
use supertag::common::log::setup_logger;
use supertag::common::notify::uds::UDSNotifier;
use supertag::common::notify::{Listener, Notifier};
use supertag::common::types::file_perms::UMask;
use supertag::common::types::note::Note;
use supertag::common::{get_device_inode, has_ext_prefix, settings};
use supertag::fuse::opcache::READDIR_EXPIRE_S;
use supertag::sql::tpool::ThreadConnPool;
use supertag::{common, fuse, sql};

pub mod dirs;
#[macro_use]
pub mod macros;
pub mod notify;
pub mod td;

const TEST_TAG: &str = "integration_tests";

const TEST_CONFIG: &str = r###"
[symbols]
inode_char = "-"
device_char = "﹫"
sync_char = "\u007F"
filedir_str = "⋂"
filedir_cli_str = "_"
tag_group_str = "+"

[mount]
"###;

pub type TestResult = Result<(), Box<dyn Error>>;

static START: Once = Once::new();

pub fn mtime<T: AsRef<Path>>(path: T) -> chrono::DateTime<chrono::Local> {
    let md = path.as_ref().metadata().unwrap();
    let dt = chrono::Local.timestamp(md.mtime(), md.mtime_nsec() as u32);
    dt
}

pub fn mtime_pause() {
    spin_sleep::sleep(std::time::Duration::from_millis(200));
}

pub fn make_unlink_name<P: AsRef<Path>>(path: P) -> PathBuf {
    let num_comps = path.as_ref().components().collect::<Vec<Component>>().len();

    let mut full_path = PathBuf::new();
    for comp in path.as_ref().components().take(num_comps - 1) {
        full_path.push(comp.as_os_str().to_str().unwrap());
    }
    full_path.push(common::constants::UNLINK_NAME);
    full_path
}

// Let's make sure our tests don't wipe out real things
fn assert_in_tmp<P: AsRef<Path>>(path: P) {
    assert!(path
        .as_ref()
        .canonicalize()
        .unwrap()
        .starts_with(std::env::temp_dir().canonicalize().unwrap()));
}

fn breadth_first_walk<P: AsRef<Path>>(dir: P, cb: &dyn Fn(&Path) -> IOResult<()>) -> IOResult<()> {
    if dir.as_ref().is_dir() {
        let mut dirs: Vec<PathBuf> = Vec::new();
        let mut files: Vec<PathBuf> = Vec::new();

        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                dirs.push(path.to_owned());
            } else {
                files.push(path.to_owned());
            }
        }

        for file in files {
            cb(&file)?;
        }

        for dir in dirs {
            breadth_first_walk(&dir, cb)?;
            cb(&dir)?;
        }
    }
    cb(dir.as_ref())?;
    Ok(())
}

fn depth_first_walk<P: AsRef<Path>>(dir: P, cb: &dyn Fn(&Path) -> IOResult<()>) -> IOResult<()> {
    if dir.as_ref().is_dir() {
        let mut dirs: Vec<PathBuf> = Vec::new();
        let mut files: Vec<PathBuf> = Vec::new();

        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                dirs.push(path.to_owned());
            } else {
                files.push(path.to_owned());
            }
        }

        for dir in dirs {
            depth_first_walk(&dir, cb)?;
            cb(&dir)?;
        }

        for file in files {
            cb(&file)?;
        }
    }
    cb(dir.as_ref())?;
    Ok(())
}

#[derive(Eq, PartialEq)]
pub enum OpMode {
    // we're using system level commands (ln, rm, mkdir) to manipulate the filesystem
    MANUAL,
    // we're using the supertag `tag` binary to do the ops
    CLI,
    // we're using macos finder
    FINDER,
}

/// An instance of TestHelper represents a mounted filesystem in a temporary directory, backed
/// by a temporary sqlite database, from which we can spawn fresh connections.  When the TestHelper
/// is dropped, everything is unmounted and removed.
pub struct TestHelper {
    pub project_directories: Arc<TestDirectories>,
    pub handle: Arc<Mutex<MountHandle>>,
    pub mountpoint: tempfile::TempDir,
    pub collection: String,

    // indicates whether to use the `tag` interface to create a symlink, or to use the unix syscall.
    // both are supported.
    pub symlink_mode: OpMode,
    pub rmdir_mode: OpMode,
    pub rm_mode: OpMode,
    pub mkdir_mode: OpMode,
    pub rename_mode: OpMode,

    pub settings: Arc<settings::Settings>,

    pub uid: uid_t,
    pub gid: gid_t,
    pub umask: UMask,
    pub test_basedir: PathBuf,
    pub notifier: Arc<Mutex<UDSNotifier>>,
    pub cwd: PathBuf,
}

pub struct LinkedFile<'a> {
    pub th: &'a TestHelper,
    pub tmp: Rc<tempfile::NamedTempFile>,
    pub tags: &'a [&'a str],
}

impl std::fmt::Display for LinkedFile<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "{}", self.tmp.path().display())
    }
}

impl<'a> LinkedFile<'a> {
    pub fn new(th: &'a TestHelper, tmp: Rc<tempfile::NamedTempFile>, tags: &'a [&str]) -> Self {
        Self { th, tmp, tags }
    }

    pub fn link_filename(&self, inodify: bool) -> String {
        self.th.filename(self.tmp.path(), inodify)
    }

    pub fn target_path(&self) -> PathBuf {
        // canonicalize for macos
        self.tmp.path().canonicalize().unwrap()
    }

    /// This returns the path to the symlink, in the filedir directory, under the passed in tags
    pub fn link_filedir_path(&self, tags: &[&str], inodify: bool) -> PathBuf {
        let filedir_path = self.th.filedir_path(tags);
        filedir_path.join(self.link_filename(inodify))
    }

    /// Provide a new name for our linked file, and tack on the inode and device correctly.  This
    /// is primarily useful in tests that do a mv, where we want to check if our desination path
    /// exists, and we construct the new filename based on the old filename
    pub fn new_link_filename(&self, new_name: &str, inodify: bool) -> String {
        if inodify {
            let (device, inode) = get_device_inode(self.tmp.path()).unwrap();
            self.th.settings.inodify_filename(new_name, device, inode)
        } else {
            new_name.to_owned()
        }
    }

    /// Like `new_link_filename` but for an entire path
    pub fn new_link_path(&self, tags: &[&str], new_name: &str, inodify: bool) -> PathBuf {
        let real_new_name = self.new_link_filename(new_name, inodify);
        let filedir_path = self.th.filedir_path(tags);
        filedir_path.join(real_new_name)
    }
}

impl TestHelper {
    pub fn new(test_config: Option<&str>) -> Self {
        let logging = std::env::var_os("STAG_LOG").is_some();
        let test_basedir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests");

        let uid = unsafe { libc::getuid() };
        let gid = unsafe { libc::getgid() };
        let umask = UMask::default();
        let perms = umask.dir_perms();

        let mountpoint = tempfile::Builder::new().prefix("col-").tempdir().unwrap();
        // on macos, this makes sure the /var/ dir becomes /private/var/, or whatever
        let mp_path = mountpoint.path().canonicalize().unwrap();

        let mut test_source = settings::config::HashMapSource(Default::default());
        test_source
            .0
            .insert("mount.uid".to_string(), (uid as i64).into());
        test_source
            .0
            .insert("mount.gid".to_string(), (gid as i64).into());
        test_source
            .0
            .insert("mount.permissions".to_string(), perms.octal_string().into());

        test_source.0.insert(
            "mount.base_dir".to_string(),
            mp_path
                .parent()
                .unwrap()
                .to_string_lossy()
                .to_string()
                .into(),
        );

        let sources: Vec<Box<dyn config::Source + Send + Sync>> = vec![
            Box::new(test_source),
            Box::new(config::File::from_str(
                test_config.unwrap_or(TEST_CONFIG),
                config::FileFormat::Toml,
            )),
        ];

        let collection = match mp_path.components().last() {
            Some(std::path::Component::Normal(dir)) => dir.to_string_lossy().into_owned(),
            _ => panic!("invalid temp mountdir"),
        };

        if logging {
            START.call_once(|| {
                setup_logger(LevelFilter::Trace, vec![std::io::stdout().into()]).unwrap();
            });
        }

        let pd = Arc::new(dirs::TestDirectories::new(test_basedir.clone()));
        let config = settings::config::build(sources, &*pd);
        let mut settings = settings::Settings::new(pd.clone()).unwrap();
        settings.update_config(config);
        settings.set_collection(&collection, true);
        let share_settings = Arc::new(settings);

        debug!(
            target: TEST_TAG,
            "Creating test helper for collection {} at supertag dir {:?}",
            collection,
            share_settings.supertag_dir()
        );

        // set up our tables
        let db_file = share_settings.db_file(&collection);
        let mut conn = sql::get_conn(&db_file).unwrap();
        sql::migrations::migrate(&mut conn, &*common::version_str()).unwrap();

        let conn_pool = ThreadConnPool::new(db_file);

        let socket_file = share_settings.notify_socket_file(&collection);
        let uds_notifier = UDSNotifier::new(socket_file, true).unwrap();
        let notifier = Arc::new(Mutex::new(uds_notifier));
        let ops = fuse::TagFilesystem::new(share_settings.clone(), conn_pool, notifier.clone());

        let fuse_conf = fuse::util::make_fuse_config(None);
        let mut mount_conf =
            fuse::util::make_mount_config("itest_col", share_settings.db_file(&collection));

        #[cfg(target_os = "macos")]
        {
            let mut rng = rand::thread_rng();

            // override our macos ids so the tests don't collide
            mount_conf.volname = Some(
                rng.sample_iter(&rand::distributions::Alphanumeric)
                    .take(30)
                    .collect::<String>(),
            );

            mount_conf.fsid = Some(rng.gen_range(1, 0xFFFFFF))
        }

        mount_conf.daemon_timeout = Some(1); // faster teardown
        let handle = mount(&mp_path, ops, false, fuse_conf, mount_conf).unwrap();
        let cwd = std::env::current_dir().unwrap();

        TestHelper {
            project_directories: pd.clone(),
            handle,
            mountpoint,
            collection,
            symlink_mode: OpMode::CLI,
            rmdir_mode: OpMode::CLI,
            rm_mode: OpMode::CLI,
            mkdir_mode: OpMode::CLI,
            rename_mode: OpMode::CLI,
            settings: share_settings,
            uid,
            gid,
            umask,
            test_basedir,
            notifier: notifier.clone(),
            cwd,
        }
    }

    pub fn test_data(&self, path: impl AsRef<Path>) -> PathBuf {
        self.test_basedir.join("../data").join(path)
    }

    pub fn set_cwd(&self, path: &Path) {
        debug!(target: TEST_TAG, "Setting cwd to {}", path.display());
        std::env::set_current_dir(path).unwrap();
    }

    /// Turns a target file path into the filename that will exist in the supertag filesystem
    pub fn filename(&self, path: &Path, inodify: bool) -> String {
        if inodify {
            let (device, inode) = get_device_inode(path).unwrap();
            self.settings.inodify_filename(
                path.file_name().unwrap().to_str().unwrap(),
                device,
                inode,
            )
        } else {
            path.file_name().unwrap().to_string_lossy().to_string()
        }
    }

    pub fn real_mountpoint(&self) -> PathBuf {
        // macos requires that we canonicalize, because of its weird way of handling tmp user paths
        self.mountpoint.path().canonicalize().unwrap()
    }

    pub fn inspect(&self) {
        println!(
            "\n\ndb: {}\nmounted at: {}\nproject dir: {}\n\n",
            self.settings.db_file(&self.collection).display(),
            self.real_mountpoint().display(),
            self.project_directories.base().display(),
        );

        #[cfg(target_os = "linux")]
        {
            let _ = std::process::Command::new("thunar")
                .arg(self.real_mountpoint())
                .spawn();
            let _ = std::process::Command::new("sqlitebrowser")
                .arg(self.settings.db_file(&self.collection))
                .spawn();
        }

        let mut s = String::new();
        std::io::stdin().read_line(&mut s).unwrap();
    }

    pub fn assert_no_note(&self) {
        let listener = self
            .notifier
            .lock()
            .listener()
            .expect("Couldn't get listener");

        assert_eq!(listener.note_count(), 0);
    }

    pub fn assert_note<L: Listener>(
        &self,
        listener: &mut L,
        idx: usize,
        notes: &[&Note],
        timeout: Duration,
    ) {
        info!(target: TEST_TAG, "Waiting for notes {:?}", notes);

        assert!(listener
            .wait_for_pred(
                |cn| {
                    for &note in notes {
                        if cn == note {
                            return true;
                        }
                    }
                    false
                },
                timeout,
                idx,
            )
            .is_some());

        info!(target: TEST_TAG, "Saw notes {:?}", notes);
    }

    #[must_use]
    pub fn fresh_conn(&self) -> Connection {
        sql::get_conn(self.settings.db_file(&self.collection)).unwrap()
    }

    #[must_use]
    pub fn getattr_exists<P: AsRef<Path>>(&self, path: P) -> bool {
        path.as_ref().metadata().is_ok()
    }

    #[must_use]
    pub fn readdir_exists<P: AsRef<Path>>(&self, path: P) -> bool {
        let full_path = path.as_ref();

        // this checks by listing the parent directory for the file name, which calls the readdir
        // fuse callback
        let needle = full_path.file_name().unwrap();
        let parent_dir = full_path.parent().unwrap();

        match fs::read_dir(parent_dir) {
            Ok(listing) => {
                let mut found_needle = false;
                for sibling in listing {
                    if needle == sibling.unwrap().file_name() {
                        found_needle = true;
                        break;
                    }
                }
                found_needle
            }
            // notice that if our error is "not found", meaning we can't even list the *directory*
            // containing the last part of our path, we say the whole thing isn't found.  this can
            // happen if we remove a tag from a file, and that file happened to be the only file
            // that was intersecting a bunch of other tags, thus making those other tags not visible
            Err(e) => match e.kind() {
                ErrorKind::NotFound => false,
                _ => {
                    error!("{:?}", e);
                    panic!("{:?}", e)
                }
            },
        }
    }

    pub fn ls(&self, parts: &[&str]) -> std::io::Result<Vec<String>> {
        let path = self.mountpoint_path(parts);
        debug!(target: TEST_TAG, "Listing {}", path.display());
        let listing = fs::read_dir(path)?;
        let mut file_list = listing
            .map(|entry| entry.map(|e| e.file_name().to_string_lossy().to_string()))
            .collect::<std::io::Result<Vec<String>>>()?;
        file_list.sort();
        Ok(file_list)
    }

    pub fn ls_filedir(&self, parts: &[&str]) -> std::io::Result<Vec<String>> {
        let mut all_files = vec![];
        let mut vparts = Vec::from(parts);
        let conf = self.settings.get_config();

        vparts.push(&conf.symbols.filedir_str);
        let listing = self.ls(vparts.as_slice())?;

        // remove the canary file

        all_files.extend(listing.into_iter().filter(|e| {
            if e.as_str() == common::constants::UNLINK_CANARY {
                return false;
            }
            true
        }));

        Ok(all_files)
    }

    pub fn ls_tags(&self, tags: &[&str]) -> std::io::Result<Vec<String>> {
        let listing = self.ls(tags);
        listing.map(|f| {
            f.into_iter()
                .filter(|e| e.as_str() != self.settings.get_config().symbols.filedir_str)
                .collect()
        })
    }

    pub fn assert_count(&self, parts: &[&str], size: u64) {
        info!(
            target: TEST_TAG,
            "Asserting file count for {:?} is {}", parts, size,
        );
        let path = self.mountpoint_path(parts);
        assert_eq!(path.metadata().map_or(0, |md| md.len()), size);
    }

    pub fn assert_path_exists<P: AsRef<Path>>(&self, path: P) {
        info!(
            target: TEST_TAG,
            "Asserting {} exists",
            path.as_ref().display()
        );
        assert!(
            self.getattr_exists(&path),
            "{:?} wasn't found via getattr",
            path.as_ref()
        );
        assert!(
            self.readdir_exists(&path),
            "{:?} file wasn't found via readdir",
            path.as_ref()
        );
    }

    pub fn assert_path_not_exists<P: AsRef<Path>>(&self, path: P) {
        info!(
            target: TEST_TAG,
            "Asserting {} doesn't exist",
            path.as_ref().display()
        );
        assert!(
            !self.getattr_exists(&path),
            "{:?} was still found via getattr",
            path.as_ref()
        );
        assert!(
            !self.readdir_exists(&path),
            "{:?} was still found via readdir",
            path.as_ref()
        );
    }

    pub fn assert_schema_wait(&self, schema: &str, assert_top_level_tags: bool, timeout: f32) {
        let timeout = std::time::Duration::from_secs_f32(timeout);
        let start = std::time::Instant::now();
        loop {
            debug!(target: TEST_TAG, "Checking schema");
            let correct = self.check_schema(schema, assert_top_level_tags, false);
            if correct {
                debug!(target: TEST_TAG, "Schema was correct!");
                break;
            }

            debug!(
                target: TEST_TAG,
                "Schema was incorrect, trying again in a sec"
            );
            self.sleep(0.25);
            let now = std::time::Instant::now();
            let elapsed = now - start;
            if elapsed > timeout {
                self.check_schema(schema, assert_top_level_tags, true);
            }
        }
    }

    pub fn check_schema(&self, schema: &str, assert_top_level_tags: bool, do_assert: bool) -> bool {
        let js: serde_json::Value = serde_json::from_str(schema).unwrap();
        let map = js.as_object().unwrap();

        if assert_top_level_tags {
            if !self.check_only_tags(
                &[],
                map.keys()
                    .map(String::as_str)
                    .collect::<Vec<&str>>()
                    .as_slice(),
                do_assert,
            ) {
                return false;
            }
        }

        let conf = self.settings.get_config();
        for (subdir, files) in map {
            let res = if has_ext_prefix(subdir, &conf.symbols.tag_group_str) {
                self.check_only_tg(
                    &subdir,
                    files
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|v| v.as_str().unwrap())
                        .collect::<Vec<&str>>()
                        .as_slice(),
                    do_assert,
                )
            } else {
                self.check_only_files(
                    &[&subdir],
                    files
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|v| v.as_str().unwrap())
                        .collect::<Vec<&str>>()
                        .as_slice(),
                    do_assert,
                )
            };
            if !res {
                return false;
            }
        }

        true
    }

    pub fn check_only_tg(&self, tg: &str, desired_tags: &[&str], assert: bool) -> bool {
        let existing = self.ls_tags(&[tg]).unwrap();
        let eset: HashSet<String> = HashSet::from_iter(existing);
        let dset: HashSet<String> = HashSet::from_iter(desired_tags.iter().map(|s| s.to_string()));
        if assert {
            assert!(
                eset.eq(&dset),
                "{}: existing {:?} didn't equal desired {:?}",
                tg,
                eset,
                dset
            );
        }
        eset.eq(&dset)
    }

    pub fn check_only_tags(&self, path_tags: &[&str], desired_tags: &[&str], assert: bool) -> bool {
        let existing = self.ls_tags(&[]).unwrap();
        let eset: HashSet<String> = HashSet::from_iter(existing);
        let dset: HashSet<String> = HashSet::from_iter(desired_tags.iter().map(|s| s.to_string()));
        if assert {
            assert!(
                eset.eq(&dset),
                "{:?}: existing {:?} didn't equal desired {:?}",
                path_tags,
                eset,
                dset
            );
        }
        eset.eq(&dset)
    }

    pub fn check_only_files(&self, tags: &[&str], files: &[&str], assert: bool) -> bool {
        let existing_res = self.ls_filedir(tags);

        match existing_res {
            Ok(existing) => {
                let eset: HashSet<String> = HashSet::from_iter(existing);
                let dset: HashSet<String> = HashSet::from_iter(files.iter().map(|s| s.to_string()));
                if assert {
                    assert!(
                        eset.eq(&dset),
                        "{:?}: existing {:?} didn't equal desired {:?}",
                        tags,
                        eset,
                        dset
                    );
                }
                eset.eq(&dset)
            }
            Err(e) if assert => {
                panic!(format!("Didn't exist {:?}: {:?}", tags, e));
            }
            Err(_e) => {
                return false;
            }
        }
    }

    pub fn assert_file_exists(&self, tags: &[&str], fname: &str) {
        let mut parts = Vec::from(tags);
        let conf = self.settings.get_config();
        parts.push(&conf.symbols.filedir_str);
        parts.push(fname);
        self.assert_parts_exists(parts.as_slice());
    }

    pub fn assert_parts_exists(&self, parts: &[&str]) {
        self.assert_path_exists(self.mountpoint_path(parts));
    }

    pub fn assert_parts_not_exists(&self, parts: &[&str]) {
        self.assert_path_not_exists(self.mountpoint_path(parts));
    }

    pub fn assert_parts_empty(&self, parts: &[&str]) {
        self.assert_path_empty(self.mountpoint_path(parts));
    }

    pub fn assert_path_empty<P: AsRef<Path>>(&self, path: P) {
        match fs::read_dir(path.as_ref()) {
            Ok(listing) => {
                let found: Vec<_> = listing.collect();
                if !found.is_empty() {
                    panic!("Found files {:?}", found);
                }
            }
            Err(e) => panic!("{:?}", e),
        }
    }

    #[must_use]
    pub fn rmdir(&self, tags: &[&str]) -> STagResult<()> {
        info!(target: TEST_TAG, "rmdir for {:?}", tags);
        match self.rmdir_mode {
            OpMode::MANUAL => {
                let to_remove = self.mountpoint_path(tags);
                assert_in_tmp(&to_remove);
                let dst_path = make_unlink_name(&to_remove);

                debug!(
                    target: TEST_TAG,
                    "rmdir via mv from {} to {}",
                    to_remove.display(),
                    dst_path.display()
                );
                let output = Command::new("mv").arg(&to_remove).arg(&dst_path).output()?;

                if !output.status.success() {
                    return Err(std::io::Error::new(
                        ErrorKind::Other,
                        format!(
                            "rmdir failed. stdout: {}. stderr: {}",
                            String::from_utf8(output.stdout).unwrap(),
                            String::from_utf8(output.stderr).unwrap()
                        ),
                    ))?;
                }
            }
            OpMode::FINDER => {
                let to_remove = self.mountpoint_path(tags);
                assert_in_tmp(&to_remove);
                let dst_path = make_unlink_name(&to_remove);
                fs::rename(&to_remove, dst_path)?;
            }
            OpMode::CLI => {
                let mut conn = self.fresh_conn();
                supertag::rmdir(
                    &self.settings,
                    &mut conn,
                    self.real_mountpoint(),
                    &self.mountpoint_path(tags),
                )?;
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn rm(&self, path: &Path) -> STagResult<()> {
        info!(target: TEST_TAG, "rm for {}", path.display());
        match self.rm_mode {
            OpMode::MANUAL => {
                assert_in_tmp(&path);
                let output = Command::new("rm").arg(&path).output()?;

                if !output.status.success() {
                    return Err(std::io::Error::new(
                        ErrorKind::Other,
                        format!(
                            "rmdir failed. stdout: {}. stderr: {}",
                            String::from_utf8(output.stdout).unwrap(),
                            String::from_utf8(output.stderr).unwrap()
                        ),
                    ))?;
                }
            }
            OpMode::FINDER => {
                assert_in_tmp(&path);
                fs::remove_file(&path)?;
            }
            OpMode::CLI => {
                let mut conn = self.fresh_conn();
                supertag::rm(&self.settings, &mut conn, path, &self.real_mountpoint())?;
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn mkdir(&self, tag: &str) -> STagResult<PathBuf> {
        info!(target: TEST_TAG, "mkdir {}", tag);
        let tag_path = self.mountpoint_path(&[tag]);

        match self.mkdir_mode {
            OpMode::MANUAL => {
                if !Command::new("mkdir")
                    .arg("-p")
                    .arg(&tag_path)
                    .output()?
                    .status
                    .success()
                {
                    return Err(std::io::Error::new(ErrorKind::Other, "mkdir failed"))?;
                }
            }
            OpMode::FINDER => {
                fs::create_dir(&tag_path)?;
            }
            OpMode::CLI => {
                let trimmed_path = tag_path.strip_prefix(self.real_mountpoint()).unwrap();

                let mut conn = self.fresh_conn();
                let tx = conn.transaction_with_behavior(TransactionBehavior::Exclusive)?;
                supertag::common::fsops::mkdir(
                    &self.settings,
                    &tx,
                    &trimmed_path,
                    self.uid,
                    self.gid,
                    &UMask::default().dir_perms(),
                )?;
                tx.commit()?;
            }
        }
        Ok(tag_path)
    }

    #[must_use]
    pub fn mv(&self, src: impl AsRef<Path>, dst: impl AsRef<Path>) -> STagResult<()> {
        info!(
            target: TEST_TAG,
            "mv {} to {}",
            src.as_ref().display(),
            dst.as_ref().display()
        );
        match self.rename_mode {
            OpMode::MANUAL => {
                if !Command::new("mv")
                    .args(&[src.as_ref(), dst.as_ref()])
                    .output()?
                    .status
                    .success()
                {
                    return Err(std::io::Error::new(ErrorKind::Other, "mv failed"))?;
                }
            }
            OpMode::FINDER => {
                fs::rename(src, dst)?;
            }
            OpMode::CLI => {
                let mut conn = self.fresh_conn();
                supertag::rename(
                    &self.settings,
                    &mut conn,
                    &self.real_mountpoint(),
                    src,
                    dst,
                    self.uid,
                    self.gid,
                    &UMask::default(),
                    &*(self.notifier.lock()),
                )?;
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn ln_with_tempfile<'a>(
        &'a self,
        to_ln: Rc<tempfile::NamedTempFile>,
        tags: &'a [&'a str],
    ) -> STagResult<LinkedFile<'a>> {
        debug!(target: TEST_TAG, "Running ln from builder");
        let linked = LinkedFile::new(self, to_ln, tags);
        self.ln_with_file(&linked.target_path(), tags)?;
        debug!(target: TEST_TAG, "Symlink created for {}", linked);
        Ok(linked)
    }

    #[must_use]
    pub fn ln_with_file(&self, src: &Path, tags: &[&str]) -> STagResult<()> {
        match self.symlink_mode {
            OpMode::MANUAL | OpMode::FINDER => {
                debug!(
                    target: TEST_TAG,
                    "Symlinking {} to {:?} in manual mode",
                    src.display(),
                    tags
                );
                let original_name = src.file_name().unwrap().to_str().unwrap();

                // pin so we can link to it
                // FIXME can this be removed and the tests made to work in a more-correct way?
                let tag_path = self.mountpoint_path(tags);
                fs::create_dir_all(&tag_path)?;

                let dst = self.mountpoint_path(tags).join(original_name);

                if self.symlink_mode == OpMode::MANUAL {
                    let output = Command::new("ln").arg("-sf").args(&[src, &dst]).output()?;
                    if !output.status.success() {
                        error!(
                            target: TEST_TAG,
                            "Problem with ln: {:?}",
                            output.status.code()
                        );
                        return Err(std::io::Error::new(ErrorKind::Other, "ln failed"))?;
                    }
                } else if self.symlink_mode == OpMode::FINDER {
                    #[cfg(target_os = "macos")]
                    {
                        supertag::platform::mac::alias::create_alias(src, &dst)?;
                    }
                    #[cfg(not(target_os = "macos"))]
                    {
                        unimplemented!()
                    }
                }
            }
            OpMode::CLI => {
                debug!(target: TEST_TAG, "We're in cli symlink mode");
                let tag_path = self.mountpoint_path(tags);
                self.ln_cli(src, &tag_path)?;
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn ln_cli(&self, src: &Path, dst: &Path) -> STagResult<()> {
        debug!(target: TEST_TAG, "ln_cli {:?} to {:?}", src, dst);
        let mut cmd_conn = self.fresh_conn();
        supertag::ln(
            &self.settings,
            &mut cmd_conn,
            &self.real_mountpoint(),
            vec![src],
            &dst,
            self.uid,
            self.gid,
            &UMask::default(),
            &*(self.notifier.lock()),
        )
    }

    #[must_use]
    pub fn ln<'a>(&'a self, tags: &'a [&'a str]) -> STagResult<LinkedFile<'a>> {
        let mut builder = tempfile::Builder::new();
        builder
            .prefix("supertag-testfile")
            .suffix(".tmp")
            .rand_bytes(8);

        self.ln_with_tempfile(Rc::new(builder.tempfile().unwrap()), tags)
    }

    pub fn mountpoint_path(&self, tags: &[&str]) -> PathBuf {
        let mut start = self.real_mountpoint();
        for &tag in tags {
            start = start.join(tag);
        }
        start
    }

    pub fn filedir_path(&self, tags: &[&str]) -> PathBuf {
        let mut parts = tags.to_vec();
        let conf = self.settings.get_config();
        parts.push(&conf.symbols.filedir_str);
        self.mountpoint_path(parts.as_slice())
    }

    pub fn assert_size(&self, tags: &[&str], size: u64) {
        let path = self.mountpoint_path(tags);
        assert_eq!(path.metadata().unwrap().len(), size);
    }

    pub fn sleep(&self, amt: f32) {
        spin_sleep::sleep(std::time::Duration::from_secs_f32(amt));
    }

    pub fn sleep_readdir_cache(&self) {
        self.sleep(READDIR_EXPIRE_S as f32 + 0.1);
    }
}

impl Drop for TestHelper {
    fn drop(&mut self) {
        debug!(target: TEST_TAG, "Resetting cwd to {}", self.cwd.display());
        self.set_cwd(&self.cwd);
    }
}
