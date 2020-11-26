Linking
#######

Linking is how you can *manually* associate a file with a :term:`tag`. The process is slightly different depending on
if you're using MacOS or Linux, and whether you're using the commandline, a file browser GUI, or the ``tag`` binary.
We'll cover them all here.

File browser GUI
****************

MacOS
=====

To link a file with Finder, you can drag and drop, but you must hold the option and command keys (⌥ + ⌘) while dropping.
This will create a link (specifically a :ref:`MacOS alias`) when you drop the file into Supertag:

Linux
=====


``tag`` binary
**************

The ``tag`` binary is straightforward:

.. code-block:: bash

    tag ln ~/Documents/RustCheatSheet.pdf rust/documents/pdfs/cheet-sheets

When no :term:`collection` is explicitly specified, as above, the :term:`default collection` is used.
The :term:`tagpath` is then considered relative to that collection. If you wish to specify a collection other than
the default collection, specify the tagpath as an absolute path.

Also, the tags do not have to exist, unlike other methods. If Supertag sees that a tag doesn't exist, it will create
it for you as part of the linking process.

``ln``
******

Linking a file with ``ln`` is straightforward and works as expected:

.. code-block:: bash

    ln ~/Documents/RustCheatSheet.pdf /mnt/programming/rust/documents/pdfs/cheat-sheets

The above command links ``RustCheatSheet.pdf`` to the tags ``rust``, ``documents``, ``pdfs``, and ``cheat-sheets``
in the ``programming`` :term:`collection`.

.. note::

    When using ``ln``, all of the tags you link to *must already exist.* This might be unexpected, if you're used to
    using the ``tag`` binary, which doesn't have this requirement. In other words, if the tag path in the previous
    example doesn't exist, you must create it first with ``mkdir``:

    .. code-block:: bash

        mkdir -p /mnt/programming/rust/documents/pdfs/cheat-sheets