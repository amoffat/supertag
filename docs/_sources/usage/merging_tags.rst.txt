.. _merging_tags:

Merging tags
############

Merging tags is the process of combining the file :term:`intersection` represented by one :term:`tagpath` into another
tagpath.

The basic idea is that renaming a tagdir only applies to the files captured by the tagpath's intersection. For example,
in the following structure:

.. code-block::

    t1/
        b.pdf
        t2/
            a.pdf
    t3/
        t4/
            c.pdf

If we rename ``t1/t2/`` to ``t3/t4/``, we end up with the following structure:

.. code-block::

    t1/
        b.pdf
    t3/
        t4/
            c.pdf
            a.pdf

The result is that ``a.pdf`` (and not ``b.pdf``) was untagged by ``t2`` and then re-tagged as ``t3`` and ``t4``.

It can be confusing at first, but when you think about it, there is an intuition behind it.

File browser GUI
******************

Select the tag in your file browser and perform the operation required to rename it. Now rename it to the tag you wish
to merge it into.

.. note::

    If you're merging in the file browser, you will only be able to rename into a *single* destination tag, whereas
    other methods allow you to rename into multiple destination tags at once.

mv
*********

In the collection, simply move the tagdir to the name to the full destination path:

.. code-block:: bash

    mv /mnt/myfiles/pdf_documents /mnt/myfiles/pdfs

Everything tagged with ``pdf_documents`` will now be tagged with ``pdfs`` instead.

tag binary
*******************

.. code-block:: bash

    tag mv pdf_documents pdfs