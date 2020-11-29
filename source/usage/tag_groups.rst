.. _tag_groups:

Tag groups
##########

Tag groups are a limited hierarchical structure that assist in de-cluttering the massive amount of tags that can
emerge in a SuperTag :term:`collection`.  By default, tag groups end with a "+" symbol, but you can customize this in your
settings.

The basic idea is that a tag can be part of zero or more tag groups, and that wherever a tag *would* be displayed,
it's tag group is displayed instead.  In effect, the tags are "grouped" to lower the number if displayed tags.
An example will make this more clear.

Example
=======

Consider the following example.  Suppose you had a ``movies`` collection, and some of the tags you created are the lead
actors for those movies.  You might have tags like the following:

.. code-block::

    al_pacino/
    clint_eastwood/
    daniel_day_lewis/
    jack_nicholson/
    mel_gibson/
    morgan_freeman/
    robert_de_niro/
    tom_hanks/

You also decide to tag your movies by director.  So you might have these tags:

.. code-block::

    christopher_nolan/
    david_lynch/
    james_cameron/
    martin_scorsese/
    quentin_tarantino/
    stanley_kubrick/
    steven_spielberg/
    wes_anderson/

Without tag groups, all of these tags (actors and directors) would be thrown together:

.. code-block::

    al_pacino/
    christopher_nolan/
    clint_eastwood/
    daniel_day_lewis/
    david_lynch/
    jack_nicholson/
    james_cameron/
    martin_scorsese/
    mel_gibson/
    morgan_freeman/
    quentin_tarantino/
    robert_de_niro/
    stanley_kubrick/
    steven_spielberg/
    tom_hanks/
    wes_anderson/

If you wished to filter by director, without having a specific director in mind, this can be challenging.
What you really want to see is something like this:

.. code-block::

    directors+/
    actors+/

With each of those folders containing the corresponding people.  Tag groups allow you to do just this.  By assigning
all of the actors to the ``actors+/`` tag group, and all of the directors to the ``directors+/`` tag group, anywhere an
actor tag is displayed will be substituted by the ``actors+/`` tag group, and same with directors.

In a way, it imposes a very limited 1-level hierarchy on tags, so that they can be more logically grouped together.

.. _creating_taggroups:

Creating
========

To create a tag group, simply make a directory but suffix the name with "+".  Now you can symlink other tags into it.