.. _removing_tags:

Removing tags
#############

Removing tags untags files with that tag and removes it from your collection. You might think that you can simply
delete a tag and everything will work out correctly, however, this is not the case, due to some quirks. See the
:ref:`FAQ entry <deleting_tags>` on this.

Because of this, if you're not using the tag binary to remove tags, the basic process is: rename the tag to ``delete``.
Yes, it is ugly, but it side-steps the issues with normal unlinking.

.. note::

    Deleting a tag has different implications depending on where you delete it from. See below.

If you delete a tag from the top level of your collection, the effect that it has is of removing the tag from *all*
files in your collection. However, if you delete a tag from within a :term:`tagpath`, the effect is that it will
remove the tag *only from the files intersected in the tagpath.* In the second scenario, the tag may still apply to
other files in your collection, whereas in the first scenario, the tag would have been completely removed from all
tags in your collection.

File browser GUI
******************

Select the tag in your file browser and perform the operation required to rename it. Now rename it to ``delete``.
The tag should disappear.

mv
*********

In the collection, simply move the tagdir to the name ``delete``:

.. code-block:: bash

    mv /mnt/myfiles/some-tag /mnt/myfiles/delete

tag binary
*******************

.. code-block:: bash

    tag rmdir some-tag