Config
######

The default config file:

.. code-block:: toml

    [symbols]
    inode_char = "-"
    device_char = "﹫"
    sync_char = "\u007F"
    filedir_str = "_"

    [mount]

Below, we will explain each section, and each value in each section, including values that are not defined in the
default config file.

symbols
***********
This section contains the various symbols that exist throughout SuperTag.

inode_char
================================
A typical :term:`fully-qualified symlink` SuperTag symlink file looks like
this:

``rust_cheatsheet.png﹫64770-19022060``

In this filename example, ``﹫`` is the ``device_char``, and ``-`` is the ``inode_char``

sync_char
================================

You shouldn't need to mess with this.  This character is not visible to you as a user, and only serves
to sync a cache flush from a tagging process to the mounting daemon.

filedir_str
================================

This is the name of every filedir in a SuperTag collection.  It can be more than a single character.  For example,
you might use the name "tag_intersections" instead of "_".

mount
***********
This section contains details related to the mounting of a SuperTag collection.

base_dir
================================
By default, this directory is determined from your platform.  On Linux, for example, it is ``/mnt``.  On MacOS, it is
``/Volumes``.  You can override the mount location by changing this key.

uid
================================
This is the user id of the mounted collection.  By default, it is determined by the user id of the user that spawned
the mount daemon.

gid
================================
This is the group id of the mounted collection.  By default, it is determined by the group id of the user that spawned
the mount daemon.

permissions
================================
These are the default mount permissions of the mounted collection.  By default, it is determined by the umask of the
user that spawned the mount daemon.