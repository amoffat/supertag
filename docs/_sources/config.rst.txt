Config
######

The default config file:

.. code-block:: toml

    [symbols]
    inode_char = "-"
    device_char = "﹫"
    sync_char = "\u007F"
    filedir_str = "⋂"
    filedir_cli_str = "_"
    tag_group_str = "+"

    [mount]
    base_dir = "<derived>"
    uid = "<derived>"
    gid = "<derived>"
    permissions = "<derived>"

Below, we will explain each section, and each value in each section, including values that are not defined in the
default config file.

symbols
***********
This section contains the various symbols that exist throughout SuperTag.

inode_char, device_char
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

This is the name of every :term:`filedir` in a SuperTag collection.  It can be more than a single character.  For example,
you might use the string ``intersections`` instead of ``⋂``.

filedir_cli_str
================================

Sometimes it is useful to have an alternate ``filedir_str`` for the commandline. For example, if you use ``⋂`` for
your ``filedir_str``, you might notice that that is a hard character to type when you're in your terminal. The
``filedir_cli_str`` serves as an alternate string that you can use on the commandline, in place of the ``filedir_str``
when browsing your files.

tag_group_str
================================

The string that *must* be appended to a directory name in order to make it a :term:`tag group`.

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