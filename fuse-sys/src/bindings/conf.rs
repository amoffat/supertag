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

//-o hard_remove         immediate removal (don't hide files)
//-o use_ino             let filesystem set inode numbers
//-o readdir_ino         try to fill in d_ino in readdir
//-o direct_io           use direct I/O
//-o kernel_cache        cache files in kernel
//-o [no]auto_cache      enable caching based on modification times (off)
//-o umask=M             set file permissions (octal)
//-o uid=N               set file owner
//-o gid=N               set file group
//-o entry_timeout=T     cache timeout for names (1.0s)
//-o negative_timeout=T  cache timeout for deleted names (0.0s)
//-o attr_timeout=T      cache timeout for attributes (1.0s)
//-o ac_attr_timeout=T   auto cache timeout for attributes (attr_timeout)
//-o noforget            never forget cached inodes
//-o remember=T          remember cached inodes for T seconds (0s)
//-o nopath              don't supply path if not necessary
//-o intr                allow requests to be interrupted
//-o intr_signal=NUM     signal to send on interrupt (10)
//-o modules=M1[:M2...]  names of modules to push onto filesystem stack
//
//-o max_write=N         set maximum size of write requests
//-o max_readahead=N     set maximum readahead
//-o max_background=N    set number of maximum background requests
//-o congestion_threshold=N  set kernel's congestion threshold
//-o async_read          perform reads asynchronously (default)
//-o sync_read           perform reads synchronously
//-o atomic_o_trunc      enable atomic open+truncate support
//-o big_writes          enable larger than 4kB writes
//-o no_remote_lock      disable remote file locking
//-o no_remote_flock     disable remote file locking (BSD)
//-o no_remote_posix_lock disable remove file locking (POSIX)
//-o [no_]splice_write   use splice to write to the fuse device
//-o [no_]splice_move    move data while splicing to the fuse device
//-o [no_]splice_read    use splice to read from the fuse device
//
//Module options:
//
//[iconv]
//-o from_code=CHARSET   original encoding of file names (default: UTF-8)
//-o to_code=CHARSET      new encoding of the file names (default: UTF-8)
//
//[subdir]
//-o subdir=DIR           prepend this directory to all paths (mandatory)
//-o [no]rellinks         transform absolute symlinks to relative
//
pub struct FuseConfig {
    pub hard_remove: Option<bool>,
    pub use_ino: Option<bool>,
    pub readdir_ino: Option<bool>,
    pub direct_io: Option<bool>,
    pub kernel_cache: Option<bool>,
    pub auto_cache: Option<bool>,
    pub umask: Option<i32>,
    pub uid: Option<i32>,
    pub entry_timeout: Option<i32>,
    pub negative_timeout: Option<i32>,
    pub attr_timeout: Option<i32>,
    pub ac_attr_timeout: Option<i32>,
    pub noforget: Option<bool>,
    pub remember: Option<i32>,
    pub nopath: Option<bool>,
    pub intr: Option<bool>,
    pub intr_signal: Option<i32>,
    pub modules: Option<Vec<String>>,
    pub max_write: Option<i32>,
    pub max_readahead: Option<i32>,
    pub max_background: Option<i32>,
    pub congestion_threshold: Option<i32>,
    pub async_read: Option<bool>,
    pub sync_read: Option<bool>,
    pub atomic_o_trunc: Option<bool>,
    pub big_writes: Option<bool>,
    pub no_remote_lock: Option<bool>,
    pub no_remote_flock: Option<bool>,
    pub splice_write: Option<bool>,
    pub splice_move: Option<bool>,
    pub splice_read: Option<bool>,
    pub volicon: Option<String>,
}

impl Default for FuseConfig {
    fn default() -> Self {
        FuseConfig {
            hard_remove: None,
            use_ino: None,
            readdir_ino: None,
            direct_io: None,
            kernel_cache: None,
            auto_cache: None,
            umask: None,
            uid: None,
            entry_timeout: None,
            negative_timeout: None,
            attr_timeout: None,
            ac_attr_timeout: None,
            noforget: None,
            remember: None,
            nopath: None,
            intr: None,
            intr_signal: None,
            modules: None,
            max_write: None,
            max_readahead: None,
            max_background: None,
            congestion_threshold: None,
            async_read: None,
            sync_read: None,
            atomic_o_trunc: None,
            big_writes: None,
            no_remote_lock: None,
            no_remote_flock: None,
            splice_write: None,
            splice_move: None,
            splice_read: None,
            volicon: None,
        }
    }
}

macro_rules! opt_expand {
    (bool, $conf:ident, $args:ident, $name:ident) => {
        if let Some(true) = $conf.$name {
            $args.push(String::from(format!("-o{}", stringify!($name))));
        }
    };
    (int, $conf:ident, $args:ident, $name:ident) => {
        if let Some(val) = $conf.$name {
            $args.push(String::from(format!("-o{}={}", stringify!($name), val)));
        }
    };
    (oct, $conf:ident, $args:ident, $name:ident) => {
        if let Some(val) = $conf.$name {
            $args.push(String::from(format!("-o{}={:03o}", stringify!($name), val)));
        }
    };
    (str, $conf:ident, $args:ident, $name:ident) => {
        if let Some(val) = $conf.$name {
            $args.push(String::from(format!("-o{}={}", stringify!($name), val)));
        }
    };
}

#[allow(clippy::cognitive_complexity)]
impl From<FuseConfig> for Vec<String> {
    fn from(conf: FuseConfig) -> Self {
        let mut args: Vec<String> = Vec::new();

        opt_expand!(bool, conf, args, hard_remove);
        opt_expand!(bool, conf, args, use_ino);
        opt_expand!(bool, conf, args, readdir_ino);
        opt_expand!(bool, conf, args, direct_io);
        opt_expand!(bool, conf, args, kernel_cache);
        opt_expand!(bool, conf, args, auto_cache);
        opt_expand!(oct, conf, args, umask);
        opt_expand!(int, conf, args, uid);
        opt_expand!(int, conf, args, entry_timeout);
        opt_expand!(int, conf, args, negative_timeout);
        opt_expand!(int, conf, args, attr_timeout);
        opt_expand!(int, conf, args, ac_attr_timeout);
        opt_expand!(bool, conf, args, noforget);
        opt_expand!(int, conf, args, remember);
        opt_expand!(bool, conf, args, nopath);
        opt_expand!(bool, conf, args, intr);
        opt_expand!(int, conf, args, intr_signal);
        opt_expand!(int, conf, args, max_write);
        opt_expand!(int, conf, args, max_background);
        opt_expand!(int, conf, args, congestion_threshold);
        opt_expand!(bool, conf, args, async_read);
        opt_expand!(bool, conf, args, sync_read);
        opt_expand!(bool, conf, args, atomic_o_trunc);
        opt_expand!(bool, conf, args, big_writes);
        opt_expand!(bool, conf, args, no_remote_lock);
        opt_expand!(bool, conf, args, no_remote_flock);
        opt_expand!(bool, conf, args, splice_write);
        opt_expand!(bool, conf, args, splice_move);
        opt_expand!(bool, conf, args, splice_read);

        if let Some(modules) = conf.modules {
            let mod_str = modules.join(":");
            args.push(format!("-omodules={}", mod_str));
        }

        opt_expand!(str, conf, args, volicon);

        args
    }
}

//-o allow_other         allow access to other users
//-o allow_root          allow access to root
//-o auto_unmount        auto unmount on process termination
//-o nonempty            allow mounts over non-empty file/dir
//-o default_permissions enable permission checking by kernel
//-o fsname=NAME         set filesystem name
//-o subtype=NAME        set filesystem type
//-o large_read          issue large read requests (2.4 only)
//-o max_read=N          set maximum size of read requests
pub struct MountConfig {
    pub allow_other: Option<bool>,
    pub allow_root: Option<bool>,
    pub auto_unmount: Option<bool>,
    pub nonempty: Option<bool>,
    pub default_permissions: Option<bool>,
    pub fsname: Option<String>,
    pub subtype: Option<String>,
    pub large_read: Option<bool>,
    pub max_read: Option<i32>,
    //
    // exclusively osxfuse https://github.com/osxfuse/osxfuse/wiki/Mount-options
    //
    pub allow_recursion: Option<bool>,
    pub auto_cache: Option<bool>,
    pub auto_xattr: Option<bool>,
    pub daemon_timeout: Option<i32>,
    pub debug: Option<bool>,
    pub defer_permissions: Option<bool>,
    pub direct_io: Option<bool>,
    pub extended_security: Option<bool>,
    pub fsid: Option<i32>,
    pub fssubtype: Option<i32>,
    pub fstypename: Option<String>,
    pub iosize: Option<i32>,
    pub jail_symlinks: Option<bool>,
    pub kill_on_unmount: Option<bool>,
    pub local: Option<bool>,
    pub negative_vncache: Option<bool>,
    pub noappledouble: Option<bool>,
    pub noapplexattr: Option<bool>,
    pub nobrowse: Option<bool>,
    pub nolocalcaches: Option<bool>,
    pub noubc: Option<bool>,
    pub novncache: Option<bool>,
    pub ping_diskarb: Option<bool>,
    pub noping_diskarb: Option<bool>,
    pub quiet: Option<bool>,
    pub rdonly: Option<bool>,
    pub volname: Option<String>,
}

impl Default for MountConfig {
    fn default() -> Self {
        MountConfig {
            allow_other: None,
            allow_root: None,
            auto_unmount: None,
            nonempty: None,
            default_permissions: None,
            fsname: None,
            subtype: None,
            large_read: None,
            max_read: None,
            //
            // exclusively osxfuse https://github.com/osxfuse/osxfuse/wiki/Mount-options
            //
            allow_recursion: None,
            auto_cache: None,
            auto_xattr: None,
            daemon_timeout: None,
            debug: None,
            defer_permissions: None,
            direct_io: None,
            extended_security: None,
            fsid: None,
            fssubtype: None,
            fstypename: None,
            iosize: None,
            jail_symlinks: None,
            kill_on_unmount: None,
            local: None,
            negative_vncache: None,
            noappledouble: None,
            noapplexattr: None,
            nobrowse: None,
            nolocalcaches: None,
            noubc: None,
            novncache: None,
            ping_diskarb: None,
            noping_diskarb: None,
            quiet: None,
            rdonly: None,
            volname: None,
        }
    }
}

impl From<MountConfig> for Vec<String> {
    fn from(conf: MountConfig) -> Self {
        let mut args: Vec<String> = Vec::new();

        opt_expand!(bool, conf, args, allow_other);
        opt_expand!(bool, conf, args, allow_root);
        opt_expand!(bool, conf, args, auto_unmount);
        opt_expand!(bool, conf, args, nonempty);
        opt_expand!(bool, conf, args, default_permissions);
        opt_expand!(str, conf, args, fsname);
        opt_expand!(str, conf, args, subtype);
        opt_expand!(bool, conf, args, large_read);
        opt_expand!(int, conf, args, max_read);

        //
        // exclusively osxfuse https://github.com/osxfuse/osxfuse/wiki/Mount-options
        //
        opt_expand!(bool, conf, args, allow_recursion);
        opt_expand!(bool, conf, args, auto_cache);
        opt_expand!(bool, conf, args, auto_xattr);
        opt_expand!(int, conf, args, daemon_timeout);
        opt_expand!(bool, conf, args, debug);
        opt_expand!(bool, conf, args, defer_permissions);
        opt_expand!(bool, conf, args, direct_io);
        opt_expand!(bool, conf, args, extended_security);
        opt_expand!(int, conf, args, fsid);
        opt_expand!(int, conf, args, fssubtype);
        opt_expand!(str, conf, args, fstypename);
        opt_expand!(int, conf, args, iosize);
        opt_expand!(bool, conf, args, jail_symlinks);
        opt_expand!(bool, conf, args, kill_on_unmount);
        opt_expand!(bool, conf, args, local);
        opt_expand!(bool, conf, args, negative_vncache);
        opt_expand!(bool, conf, args, noappledouble);
        opt_expand!(bool, conf, args, noapplexattr);
        opt_expand!(bool, conf, args, nobrowse);
        opt_expand!(bool, conf, args, nolocalcaches);
        opt_expand!(bool, conf, args, noubc);
        opt_expand!(bool, conf, args, novncache);
        opt_expand!(bool, conf, args, ping_diskarb);
        opt_expand!(bool, conf, args, noping_diskarb);
        opt_expand!(bool, conf, args, quiet);
        opt_expand!(bool, conf, args, rdonly);
        opt_expand!(str, conf, args, volname);

        args
    }
}
