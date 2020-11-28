OS-specific behavior
####################

Differences in operating systems can lead to Supertag doing fundamentally different things under the hood in order
to provide a similar user experience. We will attempt to document those differences here.

MacOS
*****

.. _macos_alias:

Linking
=======

:ref:`Linking <linking>` in MacOS is a complicated ordeal, made complicated by the fact that it is actually not
possible drag-and-drop a *symlink* in Finder. Although you *can* drag-and-drop a link, by holding the option and command
keys (⌥ + ⌘), this link is not a symlink---it is an alias file.

Long story short, an alias file is very similar to a symlink, except it is implemented at a higher level in MacOS
than a symlink. This has some benefits and some drawbacks.
On the commandline, an alias file appears as a normal file, not as the target it points to. Its contents,
contained in `resource forks <https://en.wikipedia.org/wiki/Resource_fork>`_, help MacOS determine how

What Supertag does with alias files is complicated. On one hand, we want the self-healing properties of an alias file.
On the other hand, we want the referential transparency of a symlink on the commandline. So we do both: when you
tag a file in Supertag, we *create* an alias record to the file, but we *present* a symlink to the file that the
alias record resolves to.

The alias record itself is stored in a ``managed_files`` directory in your collection's data folder. So when you move
the target of the file, the alias record (which we have created) self-heals, and the symlink that we present always
points to the alias record's target, so the symlink is never broken.