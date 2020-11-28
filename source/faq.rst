FAQ
###


Security
********

Is Supertag safe?
=================

First and foremost, the standard open source software details apply:

* This software is provided without warranty.
* The software author or license can not be held liable for any damages inflicted by the software.

I know that doesn't inspire confidence when trusting your files to SuperTag, so let me elaborate with the following:
Since all of the files stored in SuperTag are *virtual files in a database*, and SuperTag
operates on this database to create/change/remove virtual files, this greatly minimizes any risk to your existing
files being changed or removed.

Performance
***********

Why is ``ls`` so slow?
======================

Some systems will alias ``ls`` to something like ``ls='ls --color=tty'``, which changes the color of the individual
symlinks based on whether or not they are broken.  So it will essentially call the fuse ``readlink`` function on each
file individually, and this can take a lot of time if there are a lot of files.

The solution, is to use ``\ls``, which will call the ``ls`` binary unaliased, and avoid doing symlink lookups on each
file as part of listing the directory. Another solution is to use ``ls --color=never``.

Why is my file browser using so much CPU?
=========================================

Some GUI file browsers, like Linux's `Thunar <https://en.wikipedia.org/wiki/Thunar>`_ aggressively attempt to index
the folders that a user is browsing. I believe it is attempting to "look ahead" to cache and collect various metadata
about the directories. On Supertag, however, this can be...taxing. The reason is clear when you take a moment to think
about it: Supertag tag directories are dynamically created, and may represent an extraordinarily broad and deep tree,
when fully walked. Thunar can often never finish walking it, hence the CPU usage.

Other GUI file browsers, like MacOS's Finder, can be instructed to not index the Supertag :term:`manifolder`, and we
do our best to provide this instruction, so this doesn't seem to be an issue on MacOS.


Usage
*****

Why do I get an error when I drag and drop a file into a SuperTag directory?
============================================================================

SuperTag currently only handles the creation of links/symlinks/aliases.  You cannot currently copy a file directly into
a SuperTag collection.  Most file browsers allow you to drag and drop a link:

* Linux's Thunar file browser - hold ctrl+shift as you drag a file into SuperTag
* MacOS's Finder - hold Cmd + Option as you drag a file into SuperTag


How do I type ``⋂`` on the commandline?
============================================================================

You can type ``_`` instead. Although it isn't listed when you ``ls`` the directory, Supertag will interpret ``_`` as ``⋂``.


Why can't I search in Finder?
============================================================================

Finder searching is currently not added to Supertag.  If you know the name of the file, you can search your regular
hard drive for it, since Supertag just contains links to those files.  Searching in Supertag is analogous to browsing
your tag directories to narrower and narrower search results, so filename searching shouldn't be explicitly necessary.

Why can't I save directly into SuperTag?
============================================================================

Short answer: There are technical reasons why this is not possible without severe usability degradation.

------------

Long answer:

Initially, I wanted the ability to save files directly into SuperTag, without needing to save them somewhere else first
and link them into SuperTag.  Indeed most of the functionality and automated testing exists for it.  It was
implemented as the concept of a "managed file", which means that if you saved a file into SuperTag, SuperTag would figure
out where that file should really be stored on disk, and create a symlink for you automatically in SuperTag to that
managed location.  Under the hood, the managed file would live somewhere in your user's app data directory under a
hashed path.

However it turned out that enabling this ability opened up some breaking usability issues.  There are two major
underlying problems:

1. Programs that create files sometimes expect that file name to exist exactly.
2. There's no way in our filesystem handler code to determine where a copied file comes from.

The first issue is demonstrated in the following scenario:  You download a file in google chrome.  Google chrome
a `filename.crdownload` temporary file to use as its download buffer.  When it's done, it wants to move
`filename.crdownload` to `filename`, but therein lies the problem: a file saved in SuperTag gets the device id and inode
id suffixed to the filename.  So chrome, even though it had a valid file descriptor to the file it created, is not
able to find the file by name, because SuperTag has it under a different name.  This causes chrome to think that the
download process has failed.

One might think that the solution is to have another `opcache` cache, one that stored a reference from the created name
to actual name.  This way, when a process goes to look for a specific filename it created, its requests will really be
proxied to the actual symlink target.  Unfortunately, I don't believe this would work, because there is no
deterministic way to clear that entry from the cache, and the longer the entry lives, the more
problems it can cause.  For example, downloading two files of the same name at two separate times might cause chrome
to fail on the second time, after seeing the first file existing on disk, because the opcache says it exists.  You
might think that you can always clear the entry when the `release` FUSE operation is called for that file (signifying
a `close`), but I don't believe this is always true, for example, in the case of chrome trying to `rename` the file
after it has written to it and closed it.

The second issue is demonstrated by copying a file into SuperTag from a file browser.  If you copy `rust.pdf` to
`/mnt/myfiles/rust/` and then also copy `rust.pdf` to `/mnt/myfiles/pdfs`, you will have two independent copies of
`rust.pdf` on your system, in the form of managed files, with two independent symlinks pointing two those two copies.
This is because the create + write has no access to *where* the file data is coming from, so it has no way of knowing
that it came from the same file, and therefore no way to create a single managed file backing two symlinks.  The
major problem with this is that users will think they are tagging a single file with multiple tags, by dragging and
dropping into multiple tagdirs, when in actuality, they are creating many independent copies, and editing one won't
effect the others.

Unfortunately there is no way to separate the two issues.  In order to allow users to save files to SuperTag, you must
enable the `create` handler.  But if you enable the `create` handler, nothing stops them from accidentally creating
multiple copies of a file without knowing it.  The best course of action, in my mind, unfortunately, is to disable
creating files directly in SuperTag through managed files, and preserve the consistency of the core SuperTag offering.


Can SuperTag track files on multiple filesystems?
============================================================================

Yes!  Each file in the SuperTag overlay database is keyed off of (device_id, inode), so you can add files from multiple
filesystems to one SuperTag mountpoint, and there is no chance for collision.



Why can't I delete a tag or a tagged file from a file browser?
============================================================================

I had to disable unlink and rmdir through the file browser because of delete behavior that can cause some files to be
untagged.  If you attempted to delete a top level tag directory through a file browser, the expected behavior
would be to remove that tag from all files, and nothing else.  However, file browsers delete directories recursively,
depth-first, which means that all of the tagged files inside all of the tag intersections underneath the tag you wish
to delete, will also be deleted.  This behavior cannot be mitigated against, because SuperTag only sees individual delete
requests, and does not see the overall recursive delete request initiating them.


What happens when two files of the same name share the same tags?
============================================================================

On collision, symlinks in SuperTag are suffixed with the device id and inode number of the file they point to.
This prevents all name collisions.


Design
******


Will Windows be supported?
============================================================================

It is on the roadmap, but there are substantial technical hurdles to overcome to port the code base to Windows.
Help in this area is appreciated.


Why does SuperTag use symlinks?  Why not hardlinks?
============================================================================

Hard links cannot be created across devices or mount points (even on the same device).  Since SuperTag is its own
mountpoint and device, hard links cannot be made from anywhere else onto SuperTag, because that would be a
cross-device link.


Why are the tagged files in the ``⋂`` directory, and not alongside the other tag subdirectories?
=================================================================================================================

Originally, that's the way it was: a SuperTag tag directory contained all of the files tagged by it, and all of the
intersecting tags as well.  Unfortunately though, when you start tagging a lot of files, these directories get pretty
big, and it can be difficult to navigate the tags with your shell's autocomplete.  Imagine you have a top level tag
``code`` that contains every source code file on your system.  If you tried to autocomplete the next level directory,
say ``code/rust/``, it could be very taxing on your system.  It is also difficult to see all of the other tag dirs under
``code/`` because, by default, ``ls`` sorts alphabetically, not directory-first (although there *is* a non-default option
for doing that with ``ls``).

So to make SuperTag easier to use, I relegated all of the intersecting files to the subdirectory, ``⋂``.  Now you only
incur the cost of computing that file intersection, and listing all of those files, when you navigate to that
subdirectory.  If you don't like the name of the subdirectory, you can change it via your SuperTag app settings.



Why are my files in SuperTag sometimes suffixed with "@" and some numbers?
============================================================================

This @ separates your file name and its target device id and inode number.  These numbers must be embedded in the
filename itself, for two reasons:

1. We have no other way of distinguishing between a tag directory and a file.

Suppose our code receives a request from the FUSE system asking if the path `/docs/test` exists, and what
type of object it is (directory or symlink).  Is `test` a file, or a tag directory?  What if both exist?
On most filesystems, a file with the same name as a directory, in the same place, is not allowed.  As such, we can only
report back to FUSE one or the other, not both.  To avoid this situation, since all of our files have the inode number,
we'll either see a request for `/docs/test` or `/docs/test@12345-384820`, in which case we know if
we've received a request for a tag directory or a file.

2. We also have no way of showing that two different files with the same names and same tags are actually separate
files.  Imagine a very common filename, like `README` being tagged with the same tags.  If you listed a tag
directory with those two files, you would see identical filenames, which isn't allowed.  Suffixing the device id
and inode number gets around this as well.

Both of these cases are rarer in typical hierarchical filesystems, because you can just choose to rename a file in one
specific location in the directory tree, but in the case of SuperTag, a tagged file exists in many tag subdirectories at
once, so renaming it can have collisions elsewhere.


Why isn't the "﹫" in the filename suffix a real "at" character?
============================================================================

I use a special unicode ﹫ to lower the chances of colliding with a filename with an authentic ascii @ in a filename.
You can change this character in the SuperTag app settings.


Why didn't you use the `fuse crate <https://crates.io/crates/fuse>`_?
============================================================================

Andreas Neuhaus's fuse crate is an excellent package, but it doesn't do a very specific thing I needed SuperTag to do,
and that is consistently report absolute path names to fuse callbacks.  After an email conversation with Andreas,
he advised me on why his fuse crate took the direction it did, and how the support I was seeking might be implemented.
I decided to use SuperTag to stage a prototype version of a fuse integration that provides the functionality I needed.

Why didn't you use an ORM for the database?
============================================================================

ORMs are fine, but for SuperTag, I wanted a very clear, low level amount of control over the queries I was going to be
constructing.  This may change in the future.





General
*******

Can it work with Dropbox?
============================================================================

Yes. It works because Dropbox files
appear on your harddisk like any other file, and Supertag can create symlinks to them.

Can you share the Supertag folder with others?
============================================================================

You can easily mount Supertag on a cloud server and provide a simple web interface. Work is being done to support this.
Please reach out to me directly at `arwmoffat@gmail.com <mailto:arwmoffat@gmail.com>`_ if this is something that
interests you.

Will you support other database backends?
============================================================================

I am open to the idea.  Someone should make a compelling argument for pluggable backends, then we will have that
discussion.  The main advantage of a database server is remote readers and writers, but that doesn't totally make sense
with a local filesystem, where the entries are symlinks pointing to other files on disk.  The code would also need
to migrate to use an ORM, but that isn't a huge deal.

Aren't tag groups a hierarchical structure?
============================================================================

Yes, but albeit a very limited one.  The focus of SuperTag is tags, but I also don't want tags to get in your way of
using tags to their fullest potential.  If that means adding a hierarchical abstraction, then that's still a win.

Who made your logo?
============================================================================
I did. You can see the source blender file I used to build the logo in the ``logo/`` directory of this repo.

Why is the Linux app an AppImage?
============================================================================
I tried to figure out doing a debian package, but I found the process very difficult. I still want to have a debian
package at some point, since they are easy to upgrade, but I need someone to help me with that.