.. _mounting:

Mounting
########

Mounting attaches a collection :term:`collection` to the filesystem so that you can use it. It can only be done on
the commandline, using the ``tag`` binary.

.. code-block:: bash

    tag mount myfiles

This will mount a collection named ``myfiles`` to your system's mount directory and fork into the background.
Your mount directory can be changed with :ref:`mount.base_dir <mount_base_dir>`.
If the collection doesn't exist, it
will be created for you. The result is that you will have a folder available at either ``/Volumes/myfiles`` (on Mac)
or ``/mnt/myfiles`` (on Linux).

.. note::

    If you're on Linux, you'll need to make sure ``/mnt/myfiles`` exists and is owned by your user, before you mount.
    On MacOS, this doesn't seem to be necessary.

You can also mount your collection without forking a background process using the ``-f`` flag. In this case,
``SIGINT`` will unmount and end the process.

Unmounting
==========

Similar to mounting, you can unmount:

.. code-block:: bash

    tag unmount myfiles

This will unmount the ``myfiles`` collection. The collection argument is optional, so if you wish to unmount all of
your collections, don't specify a collection to the unmount subcommand.