.. _quickstart:

Quickstart
###########

After you've :ref:`installed <installation>` Supertag, you are ready to start using it. First, let's mount a new
:term:`collection`:

.. code-block:: bash

    tag mount myfiles

You should see something like:

.. code-block::

    Mounting to "/mnt/myfiles"
    Forked into background PID 19754

Great, Supertag tells where it mounted your collection and opens that location in a file browser. We can check what collections are available and where they're mounted with:

.. code-block:: bash

    tag fstab

Output:

.. code-block::

    Collections:
      * myfiles => /mnt/myfiles

The star in front indicates a :term:`primary collection`, and because the collection is mounted, we see the path that
it is mounted to.

Now let's start tagging files. First, save this photo of M. C. Escher's *Waterfall.* We'll
use it to test out tagging:

.. image:: ../images/waterfall.jpg

Now let's tag it using our tag binary:

.. code-block:: bash

    tag ln waterfall.jpg escher/waterfall/architecture

Now let's browse to ``/mnt/myfiles`` and explore what Supertag did. Supertag created 3 new tags and represented them as directories:

.. image:: ../images/quickstart/3tags.png

Click into ``waterfall``. You'll see the following directories listed:

.. image:: ../images/quickstart/subdirs.png

What's happening here is Supertag is showing ``escher`` and ``architecture`` as subdirectories
because there exists at least 1 file (waterfall.jpg) that shares the tag ``waterfall`` as well as the
tags ``escher`` and ``architecture``.

You'll also notice a funny directory named ``⋂``. This directory means "the intersection of." Its
contents will be the *files* of the intersection of the tags in your current path:

.. image:: ../images/quickstart/intersection.png

Let's add a couple more tags, but this time, let's use some of our native system binaries. First
let's create some :term:`tagdirs` to ensure that we have somewhere to link to:

.. code-block:: bash

    mkdir -p /mnt/myfiles/lithograph/impossible

Now let's link our photo with a symlink:

.. code-block:: bash

    ln -sf waterfall.jpg /mnt/myfiles/lithograph/impossible

This added 2 more tags to our photo:

.. image:: ../images/quickstart/moretags.png

.. note::

    We could have created our tags separately with individual mkdir commands, and also linked
    the photo to those two tags with individual ln commands, but I wanted to demonstrate the
    concept of :term:`pinning`, as well as applying multiple tags at once.

