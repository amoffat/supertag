.. _architecture:

Architecture
############

To understand the high-level architecture of Supertag, it is important to understand what Supertag is fundamentally
doing, at a technical level. At its core, Supertag is system for viewing tags in SQL database through a filesystem.

The following technical architecture will explain how this is achieved.


FUSE
****

Supertag uses `FUSE <https://en.wikipedia.org/wiki/Filesystem_in_Userspace>`_ to connect a :term:`collection` to the
filesystem via the ``fuse-sys`` Rust crate, written specifically for Supertag. The job of ``fuse-sys`` is to
surface a user's filesystem actions and connect them to Supertag application code.

For example, ``mkdir`` can be used to create a new :term:`tag` in Supertag, while ``ln`` can be used to manually link
a file to that tag. The actions resulting from these commands are channeled through FUSE to allow manipulating
Supertag files and tags.

SQLite database
***************

The Supertag backend is a :term:`per-collection <collection>` sqlite database. It contains the records for files,
tags, the many-to-many links between files and tags, and other useful data. See the
:ref:`Datatabase Schema <database_schema>` for more details.


The ``tag`` binary
******************

Many Supertag operations can be initiated by manipulating the filesystem in expected ways, either through the
commandline or through a GUI file browser. See :ref:`Usage <usage>`. However, some operations that
are a little more nuanced are provided as subcommands on the Supertag ``tag`` binary. For example the ``tag`` binary
will let you link a file to multiple non existant tags at the same time:

.. code-block:: bash

    tag ln /home/Desktop/somefile.pdf pdfs/documents/research/projects

The above isn't possible with regular ``ln`` unless the :term:`tagpath` ``pdfs/documents/research/projects`` already
exists, otherwise ``ln`` will report ``No such file or directory.`` However, ``tag`` binary allows you to do this
by creating tags as needed directly in the sqlite database.

Database
********

See :ref:`Database Schema <database_schema>`
