.. _mounting:

Mounting
########

Mounting attaches a collection :term:`collection` to the filesystem so that you can use it.

.. code-block:: bash

    tag mount myfiles

This will mount a collection named ``myfiles`` to your system's mount directory. If the collection doesn't exist, it
will be created for you. The result is that you will have a folder available at either ``/Volumes/myfiles`` (on Mac)
or ``/mnt/myfiles`` (on Linux).

.. note::

    If you're on Linux, you'll need to make sure ``/mnt/myfiles`` exists and is owned by your user, before you mount.