.. _pinning:

Pinning
#######

:term:`Pinning <pinning>` lets you create some nested tags even when no files live at the tags intersection.

By default, the directory ``/studies/climate`` will only exist if there are files tagged with both ``studies`` and ``climate``.
This is because :term:`tagdirs <tagdir>` are tag *intersections*, meaning that if there are no files with both of those tags,
that specific directory
path won't exist.  Having it be this way is useful for navigating directories, because you will only
see sub-tagdirs that are relevant to your current tagdir. However, it is not so useful when populating the tagdirs for
the first time, because you may want to navigate to the nested tagdirs before linking anything into them.
To alleviate this problem, we use tagdir pinning.

Pinning is creating a temporary "pin" in a :term:`tagpath`, so that you can navigate to it without requiring actual files to exist
inside of it.  Without tagdir pinning, you
could not browse to ``/studies/climate`` unless there already existed files tagged with ``studies`` and ``climate``.