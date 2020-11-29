.. _creating_tags:

Creating tags
#############

Creating tags in Supertag is accomplished by creating a directory (a :term:`tagdir`). To this end, you can use any
tool that creates directories. The primary motivation for creating tagdirs is to then link files to them. The ``tag``
binary doesn't require the existence of tagdirs in order to link files to them, so we only show the file browser and
system command methods here.

File browser GUI
****************

To create a tag in your file browser, usually you can right-click and select "New Folder". The folder
that you create will be a tag.

mkdir
***************

.. code-block:: bash

    mkdir /mnt/myfiles/pdfs

This will create the ``pdfs`` tagdir in your ``myfiles`` collection. You may also create multiple tags at once and
:ref:`pin them <pinning>` with the ``-p`` argument to ``mkdir``:

.. code-block:: bash

    mkdir -p /mnt/myfiles/pdfs/programming/rust