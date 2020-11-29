Tips & Tricks
#############

Browsing the database
*********************

A portable way to browse the underlying sqlite database is to use the super secret ``/.supertag`` directory that lives
in your collection's mountpoint. Inside, there is a ``db.sqlite3`` file that you can connect to. See :ref:`database_schema`
for a detailed listing of each table.

Tag Negations
*************

Sometimes it is useful to list all :term:`intersections <intersection>` *sans* a tag. For example, if you wanted to
list all files that are ``photos``, ``trips``, that are *not* tagged with ``seattle``, how would you do it? The answer
is tag negations:

.. code-block:: bash

    ls photos/trips/-seattle/⋂

Simply prepend the ``-`` character before any tags that you wish to negate from the intersection, and Supertag will
do the right thing.

Tag Group Transparency
**********************

A :term:`tag group` has "stat transparency" to the tags it contains. What this means is although you cannot always
see the tags contained by a tag group, you can pretend that they are there. An example will make this more clear.
Suppose you have the following file structure:::

    /
        2016/
            ...
        photos/
            ...
        people/
            person-1/
            person-2/
            person-3/

Above, ``people`` is a tag group for the tags ``person-1``, ``person-2``, and ``person-3``. This means that wherever
one of those tags would appear, the tag group will appear instead. So for example, if there exists an
:term:`intersection` between ``photos`` and ``person-1``, listing ``photos`` will show:::

    photos/
        people/
            person-1/

``people`` is shown as the immediate child of ``photos``, because ``person-1`` would have been listed, but ``person-1``
has a tag group, so the tag group was shown instead.

However, "stat transparency" means that although ``person-1`` isn't the immediate child of ``photos``, we can treat it
as if it was. This means that you can do ``ls people/person-1/⋂`` to list the intersection of those two tags, without
needing to go through the ``people`` tag group.