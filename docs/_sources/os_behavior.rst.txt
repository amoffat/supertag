OS-specific behavior
####################

Differences in operating systems can lead to Supertag doing fundamentally different things under the hood in order
to provide a similar user experience. We will attempt to document those differences here.

MacOS
*****

Linking
=======

:ref:`Linking <linking>` in MacOS is a complicated ordeal, made complicated by the fact that it is actually not
possible drag-and-drop a symlink in Finder. Although you *can* drag-and-drop a link, by holding the option and command
keys (⌥ + ⌘), this link is not a symlink---it is an alias file.

Long story short, an alias file is very similar to a symlink, except it is implemented at a higher level in MacOS
than a symlink. This has some benefits and some drawbacks.
On the commandline, an alias file appears as a normal file, not as the target it points to. Its contents,
contained in `resource forks <https://en.wikipedia.org/wiki/Resource_fork>`_, help MacOS determine how