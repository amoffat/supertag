Glossary
########

.. glossary::

    collection
        An individual top-level Supertag folder. You might have different collections for different broad groups of
        files, for example, ``MyPhotos``, or ``MyDocuments`` or a collection related to all the files in a specific
        project.

    primary collection
        When multiple collections are mounted, this is the oldest currently-mounted collection. If a command accepts
        a collection argument, but one is omitted, the primary collection is implied and used.

    tag
        A tag is a label applied to a file. Tags show up as directories in a
        :term:`collection`. Tags are linked to files in a many-to-many relationship: a tag can apply to many files,
        and a file can be labeled with many tags.

    filedir
        A filedir is how you access the intersection of the :term:`tags <tag>` in a path. By default, it is the
        mathematical symbol for set intersection, ``⋂``

    manifolder
        The on-disk folder that represents a :term:`collection`. It is a portmanteau of "manifold" and "folder", meant
        to capture the idea of being a multidimensional organization of files.

    intersection
        The files inside of a :term:`filedir` that represents the set intersection of all of the tags in a
        :term:`tagpath`.

    tagpath
        A relative path inside of a Supertag :term:`collection`, containing only tags. For example,
        ``photos/vacations/seattle/2016``. It is useful to think about the :term:`intersection` of files at a tagpath.

    tag group
        A tag group is a placeholder for collection of similar tags. For example, you may group all of the photos
        tagged with your friends under the ``people`` tag. Then, whenever a person tag would be shown, it will show
        ``people`` instead. This keeps the filesystem from looking overly cluttered.

    default collection
        The oldest mounted :term:`collection` that is currently mounted. The default collection is automatically used
        for operations where specifying a specific collection is optional.

    fully-qualified symlink
        This is a symlink that has the inode and the device id suffixed to the symlink name. You will typically only
        ever see a fully-qualified symlink in the case that there are multiple files with the same base name, in which
        case, Supertag will fully-qualify the symlinks before displaying them, so you can distinguish between the two.

    pinning
        Forcing a :term:`tagpath` to exist even although no files occupy its intersection. This is a useful technique
        when creating tags from a file GUI to tag a file with multiple tags at once.