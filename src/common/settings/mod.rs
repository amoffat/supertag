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

use super::constants;
use super::err::{STagError, STagResult};
use crate::common::types::file_perms::UMask;
use crate::common::types::{DeviceFile, TagType};
use crate::common::{err, get_filename, strip_ext_prefix};
use directories as dir;
use log::{debug, warn};
use parking_lot::RwLock;
use std::io::Write;
use std::path::Component::{Normal, RootDir};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub mod config;
pub mod dirs;

const TAG: &str = "settings";

#[cfg(target_os = "macos")]
const VOLUMEICON: &[u8] = include_bytes!("../../../logo/VolumeIcon.icns");

/// Settings represents an interface to our settings, which relies on two underlying components. Config and Dirs.
/// Config represents the configuration loaded from a config file, while Dirs represents platform-specific locations
/// of common directories. Combining Config and Dirs underneath Settings yields all of the things we need to know in
/// order for Supertag to function.
pub struct Settings {
    config: RwLock<Option<config::Config>>,
    merged_config: ::config::Config, // FIXME currently unused
    project_dirs: Arc<dyn dirs::Dirs>,

    /// This is set after we're instantiated
    collection: Option<String>,
}

#[must_use]
fn ensure_dir(path: impl AsRef<Path>) -> std::io::Result<()> {
    debug!(
        target: TAG,
        "Ensuring dir {} exists",
        path.as_ref().display()
    );
    if !path.as_ref().exists() {
        debug!(
            target: TAG,
            "Dir {} doesn't exist, creating",
            path.as_ref().display()
        );
        std::fs::create_dir_all(path)?;
    }
    Ok(())
}

#[must_use]
fn ensure_file(path: impl AsRef<Path>, contents: impl AsRef<[u8]>) -> std::io::Result<()> {
    debug!(
        target: TAG,
        "Ensuring file {} exists",
        path.as_ref().display()
    );
    if !path.as_ref().exists() {
        debug!(
            target: TAG,
            "File {} doesn't exist, creating",
            path.as_ref().display()
        );
        let mut f = std::fs::File::create(&path)?;
        f.write_all(contents.as_ref())?;
    }
    Ok(())
}

impl Settings {
    pub fn new(project_dirs: Arc<dyn dirs::Dirs>) -> Result<Self, Box<dyn std::error::Error>> {
        let settings = Settings {
            config: Default::default(),
            project_dirs,
            collection: None,
            merged_config: Default::default(),
        };
        settings.ensure_config_files()?;
        Ok(settings)
    }

    fn ensure_config_files(&self) -> std::io::Result<()> {
        ensure_dir(self.config_dir())?;
        ensure_dir(self.project_dirs.data_dir())?;
        ensure_dir(self.collections_dir())?;
        ensure_file(self.base_config_file(), constants::DEFAULT_CONFIG_TOML)?;

        #[cfg(target_os = "macos")]
        ensure_file(self.volicon_path(), VOLUMEICON)?;

        Ok(())
    }

    fn ensure_collection_files(&self, col: &str) -> std::io::Result<()> {
        ensure_dir(self.collection_dir(col))?;
        ensure_dir(self.log_dir(col))?;
        #[cfg(target_os = "macos")]
        ensure_dir(self.managed_dir(col))?;
        Ok(())
    }

    pub fn update_config<T>(&mut self, merged_config: T)
    where
        T: ::config::Source + Send + Sync + 'static,
    {
        let mut guard = self.config.write();
        self.merged_config
            .merge(merged_config)
            .expect("Couldn't merge in new config");
        let frozen = self.merged_config.clone().try_into().unwrap();
        *guard = Some(frozen);
    }

    pub fn get_config(&self) -> config::Config {
        let guard = self.config.read();
        guard.as_ref().expect("Config not set!").clone()
    }

    pub fn get_collection(&self) -> String {
        self.collection
            .as_deref()
            .expect("Collection not set!")
            .to_string()
    }

    pub fn set_collection(&mut self, col: &str, set_config: bool) -> Option<String> {
        // part of the bootstrapping process requires set_config to be false
        if set_config {
            let col_conf = self.config_file(col);
            if col_conf.exists() {
                let source = ::config::File::from(col_conf);
                self.update_config(source);
            }
        }
        self.ensure_collection_files(col)
            .expect("Couldn't create collection files");
        self.collection.replace(col.into())
    }

    pub fn suffix_sync_char(&self, path: &Path) -> STagResult<PathBuf> {
        let mut sync_file_name = super::get_filename(path)?.to_owned();
        sync_file_name.push(self.get_config().symbols.sync_char);
        Ok(path
            .parent()
            .ok_or_else(|| STagError::InvalidPath(path.to_owned()))?
            .join(sync_file_name))
    }

    /// This is the directory that serves as the location for where all Supertag collections will be
    /// mounted.  It is generally platform-specific.  On Linux, for example, it is /mnt, while on
    /// MacOS, it is /Volumes
    pub fn supertag_dir(&self) -> PathBuf {
        self.get_config().mount.base_dir.clone()
    }

    #[cfg(target_os = "macos")]
    pub fn managed_save_path<P: AsRef<Path>>(&self, path: P, col: &str) -> PathBuf {
        let (unique_path, hash) = super::managed_file::subdir_path(&path);
        let managed_dir = self.managed_dir(col);
        let sd = managed_dir.join(unique_path);
        ensure_dir(&sd).expect("Couldn't create managed file directory");
        let sf = sd.join(hash);
        sf
    }

    /// Returns the first found collection in the mount directory
    pub fn primary_collection(&self) -> Result<Option<String>, Box<dyn std::error::Error>> {
        crate::platform::primary_collection(self)
    }

    /// Given a `path`, determine the collection name from that path, if possible.
    pub fn collection_from_path<P: AsRef<Path>>(
        &self,
        path: P,
        confirm_exists: bool,
    ) -> Option<String> {
        match path.as_ref().strip_prefix(self.supertag_dir()) {
            Ok(stripped) => {
                let first_comp = stripped.components().next()?;
                let col = first_comp.as_os_str().to_str()?.to_string();

                if confirm_exists {
                    let col_dir = self.collection_dir(&col);
                    if col_dir.exists() {
                        Some(col)
                    } else {
                        None
                    }
                } else {
                    Some(col)
                }
            }
            Err(_) => None,
        }
    }

    /// Given a path, tries the to determine a collection for it, else fall back to the primary
    /// collection if one exists
    pub fn resolve_collection<P: AsRef<Path>>(
        &mut self,
        path: P,
    ) -> Result<String, Box<dyn std::error::Error>> {
        debug!(
            target: TAG,
            "Resolving collection for path {:?}",
            &path.as_ref()
        );
        match self.collection_from_path(path, true) {
            Some(collection) => {
                self.collection = Some(collection.clone());
                Ok(collection)
            }
            None => {
                debug!(
                    target: TAG,
                    "Couldn't resolve path to collection, using default primary collection"
                );
                let pc = self
                    .primary_collection()?
                    .ok_or_else(|| "Couldn't find primary collection")?;
                self.collection = Some(pc.clone());
                Ok(pc)
            }
        }
    }

    fn volicon_path(&self) -> PathBuf {
        self.project_dirs.data_dir().join("VolumeIcon.icns")
    }

    pub fn volicon(&self) -> Option<PathBuf> {
        let path = self.volicon_path();
        if path.exists() {
            debug!(target: TAG, "VolumeIcon {} found", path.display());
            Some(path)
        } else {
            warn!(target: TAG, "VolumeIcon not found at {}", path.display());
            None
        }
    }

    pub fn notification_icon(&self) -> Option<PathBuf> {
        self.appimage_file("supertag.png")
    }

    fn appimage_file(&self, name: &str) -> Option<PathBuf> {
        match std::env::var_os("APPDIR") {
            Some(appimage_dir) => {
                let f = Path::new(&appimage_dir).join(name);
                if f.exists() {
                    Some(f)
                } else {
                    None
                }
            }
            None => None,
        }
    }

    pub fn managed_dir(&self, col: &str) -> PathBuf {
        self.collection_dir(col)
            .join(constants::MANAGED_FILES_DIR_NAME)
    }

    pub fn data_dir(&self) -> PathBuf {
        self.project_dirs.data_local_dir().to_owned()
    }

    pub fn config_dir(&self) -> PathBuf {
        self.project_dirs.config_dir().to_owned()
    }

    pub fn log_dir(&self, col: &str) -> PathBuf {
        self.collection_dir(col).join("logs/")
    }

    pub fn collections_dir(&self) -> PathBuf {
        self.config_dir().join("collections")
    }

    pub fn collection_dir(&self, col: &str) -> PathBuf {
        self.collections_dir().join(col)
    }

    pub fn daemon_mountpoint(&self) -> PathBuf {
        self.mountpoint(&self.get_collection())
    }

    /// A convenience method *only for the mount daemon* that assumes `collection` is set.
    pub fn abs_mountpoint(&self, rel: &Path) -> PathBuf {
        let mp = self.daemon_mountpoint();
        match rel.strip_prefix(std::path::MAIN_SEPARATOR.to_string()) {
            Ok(stripped) => mp.join(stripped),
            Err(_) => mp.join(rel),
        }
    }

    /// This is the specific absolute mountpoint for a given collection
    pub fn mountpoint(&self, col: &str) -> PathBuf {
        self.supertag_dir().join(col)
    }

    pub fn db_file(&self, col: &str) -> PathBuf {
        self.collection_dir(col).join(format!("{}.db", col))
    }

    pub fn notify_socket_file(&self, col: &str) -> PathBuf {
        self.collection_dir(col).join("notify.sock")
    }

    pub fn base_config_file(&self) -> PathBuf {
        let conf_dir = self.config_dir();
        conf_dir.join("config.toml")
    }

    pub fn config_file(&self, col: &str) -> PathBuf {
        self.collection_dir(col).join("config.toml")
    }

    /// Takes a path and converts it to a collection of TagTypes. This function has to exist on the `Settings` struct
    /// because it uses symbols that can only come from loading the user's settings
    pub fn path_to_tags<P: AsRef<Path>>(&self, path: P) -> Vec<TagType> {
        let mut tags = vec![];
        let mut prev_tag: Option<TagType> = None;

        for comp in path.as_ref().components() {
            match comp {
                RootDir => {}
                Normal(comp_osstr) => {
                    let tag_str = comp_osstr.to_str().unwrap();
                    let conf = self.get_config();
                    let determined_tag = {
                        if let Some(trimmed) = super::strip_negative_tag(tag_str) {
                            TagType::Negation(trimmed.to_owned())
                        } else if let Some(trimmed) =
                            strip_ext_prefix(tag_str, &conf.symbols.tag_group_str)
                        {
                            TagType::Group(trimmed.to_owned())
                        } else if tag_str == conf.symbols.filedir_str
                            || tag_str == conf.symbols.filedir_cli_str
                        {
                            TagType::FileDir
                        } else if let Ok(Some(df)) = self.filename_to_device_file(tag_str) {
                            TagType::DeviceFileSymlink(df)
                        } else if let Some(TagType::FileDir) = &prev_tag {
                            TagType::Symlink(tag_str.to_owned())
                        } else {
                            TagType::Regular(tag_str.to_owned())
                        }
                    };
                    prev_tag = Some(determined_tag.clone());
                    tags.push(determined_tag);
                }
                _ => {}
            }
        }
        tags
    }

    pub fn inodify_filename(&self, filename: &str, device: u64, inode: u64) -> String {
        let conf = self.get_config();
        let mut ifn = String::new();
        ifn.push_str(filename);
        ifn.push(conf.symbols.device_char);
        ifn.push_str(&device.to_string());
        ifn.push(conf.symbols.inode_char);
        ifn.push_str(&inode.to_string());
        ifn
    }

    /// Takes a path and captures the inode number the filename.  Originally we used a regex, but regex
    /// is incredibly slow, as reported by perf.  So we'll just do a simple linear search.  We also
    /// set `is_unlinking` to true if it's a special path that has been passed to us the signify that
    /// we are in the process of unlinking the filename.  Doing this here is simply an optimization
    /// so we don't have to do O(2n) where n is num of path characters, for every file in a potentially
    /// large directory.
    pub fn path_to_device_file(&self, path: &Path) -> Result<Option<DeviceFile>, err::STagError> {
        let filename = get_filename(path)?;
        self.filename_to_device_file(filename)
    }

    pub fn filename_to_device_file(
        &self,
        filename: &str,
    ) -> Result<Option<DeviceFile>, err::STagError> {
        let syms = &self.get_config().symbols;
        let mut inode_nums = Vec::new();
        let mut device_nums = Vec::new();
        let mut start_device_capture = false;
        let mut start_inode_capture = false;
        let mut real_filename_chars = vec![];

        // even though this is iterating over codepoints, it should be fine, since our @ is a single
        // codepoint, and so are our individual inode numbers
        for letter in filename.chars() {
            if letter == syms.device_char {
                start_device_capture = true;
                continue;
            } else if letter == syms.inode_char && start_device_capture {
                start_inode_capture = true;
                start_device_capture = false;
                continue;
            } else if letter == syms.sync_char {
                //
            } else if start_device_capture {
                device_nums.push(letter);
            } else if start_inode_capture {
                inode_nums.push(letter);
            } else if !start_inode_capture && !start_device_capture {
                real_filename_chars.push(letter);
            }
        }

        // no error, but no inode found either
        if !start_inode_capture {
            return Ok(None);
        }

        let real_filename: String = real_filename_chars.into_iter().collect();

        let device_str: String = device_nums.into_iter().collect();
        let device = device_str
            .to_string()
            .parse()
            .map_err(|_| err::STagError::BadDeviceFile(filename.to_string()))?;

        let inode_str: String = inode_nums.into_iter().collect();
        let inode = inode_str
            .parse()
            .map_err(|_| err::STagError::BadDeviceFile(filename.to_string()))?;

        Ok(Some(DeviceFile::new(&real_filename, device, inode)))
    }
}

impl From<&str> for Settings {
    fn from(_settings_str: &str) -> Self {
        unimplemented!()
    }
}

impl Default for Settings {
    fn default() -> Self {
        let mut settings_sources: Vec<Box<dyn ::config::Source + Send + Sync>> = vec![];

        let mut source = config::HashMapSource(Default::default());
        source.0.insert("mount.uid".to_string(), 1000.into()); // FIXME
        source.0.insert("mount.gid".to_string(), 1000.into()); // FIXME
        source.0.insert(
            "mount.permissions".to_string(),
            format!("{:o}", UMask::default().dir_perms().mode()).into(),
        );

        let pd = dir::ProjectDirs::from("", constants::ORG, constants::APP_NAME).unwrap();
        settings_sources.push(Box::new(source));
        let conf = config::build(settings_sources, &pd);

        let mut settings = Settings::new(Arc::new(pd)).unwrap();
        settings.update_config(conf);

        settings
    }
}

#[cfg(test)]
mod tests {
    type TestResult = Result<(), Box<dyn std::error::Error>>;
    use super::*;
    use std::path::{Path, PathBuf};

    #[test]
    fn test_good_path_to_inode() -> TestResult {
        let settings = Settings::default();
        let path = settings.inodify_filename("/test/some_file", 987, 12345);

        let res = settings.path_to_device_file(Path::new(&path))?;

        assert!(res.is_some());
        assert_eq!(res.unwrap(), DeviceFile::new("some_file", 987, 12345));
        Ok(())
    }

    #[test]
    fn test_bad_path_to_inode() -> TestResult {
        let settings = Settings::default();
        let res = settings.path_to_device_file(Path::new("/test/some_file"))?;
        assert!(res.is_none());
        Ok(())
    }

    #[test]
    fn test_empty_path_to_inode() -> TestResult {
        let settings = Settings::default();
        match settings.path_to_device_file(Path::new("")) {
            Ok(_) => panic!("Test didn't raise"),
            Err(err::STagError::InvalidPath(path)) => {
                assert_eq!(path, PathBuf::from(""));
            }
            Err(e) => panic!("Wrong error raised {:?}", e),
        }

        Ok(())
    }

    #[test]
    fn test_unlinking_path_to_inode() -> TestResult {
        let settings = Settings::default();
        let mut path = settings.inodify_filename("/test/some_file", 987, 12345);
        path.push(settings.get_config().symbols.sync_char);

        let res = settings.path_to_device_file(Path::new(&path))?;

        assert!(res.is_some());
        assert_eq!(res.unwrap(), DeviceFile::new("some_file", 987, 12345));
        Ok(())
    }
}
