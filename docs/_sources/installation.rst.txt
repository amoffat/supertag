.. _installation:

Installation
############

Use the :ref:`Normal <normal_install>` method if you just want to try Supertag. If you're interested in development and contributing to the
project, use the :ref:`Development <dev_install>` method.

.. _normal_install:

Normal
******

This will install the Supertag binary and set up all dependencies for you.

MacOS
=====

Install `OSXFuse <https://osxfuse.github.io/>`_. Supertag is built on the
`FUSE <https://en.wikipedia.org/wiki/Filesystem_in_Userspace>`_ platform, and OSXFuse is MacOS's port of it:

.. code-block:: bash

    brew cask install osxfuse

Now go to ``System Preferences -> Security & Privacy -> General`` and click "Allow" for "Benjamin Fleischer".
Benjamin Fleischer is the author of OSXFuse, and OSXFuse needs to be explicitly confirmed as trusted software.
You will need to reboot.

Now you're ready to install Supertag:

.. code-block:: bash

    brew install amoffat/rnd/supertag

Linux
=====

The following `AppImage <https://appimage.org/>`_ works on all major Linux distributions:

.. code-block:: bash

    curl -Ls https://github.com/amoffat/supertag/releases/latest/download/supertag-x86_64.AppImage > tag
    sudo mv tag /usr/local/bin/tag
    sudo chmod +x /usr/local/bin/tag

.. _dev_install:

Development
***********

This will set up the Supertag repository on your computer and get the development environment ready.

Common
======

Whether you're installing on MacOS or Linux, you'll need to do the following setup.

Grab a copy of the Supertag source code from Github:

.. code-block:: bash

    git clone https://github.com/amoffat/supertag.git


MacOS
=====

Dependencies
------------

Make sure you have Homebrew installed:

.. code-block:: bash

    /usr/bin/ruby -e "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/master/install)"

Now install Rust:

.. code-block:: bash

    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

Install pkg-config, which will help us compile things:

.. code-block:: bash

    brew install pkg-config

Install `OSXFuse <https://osxfuse.github.io/>`_. Supertag is built on the
`FUSE <https://en.wikipedia.org/wiki/Filesystem_in_Userspace>`_ platform, and OSXFuse is MacOS's port of it:

.. code-block:: bash

    brew cask install osxfuse

Now go to ``System Preferences -> Security & Privacy -> General`` and click "Allow" for "Benjamin Fleischer".
Benjamin Fleischer is the author of OSXFuse, and OSXFuse needs to be explicitly confirmed as trusted software.
You will need to reboot.

Linux
=====

Dependencies
------------

Install Rust:

.. code-block:: bash

    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

Now install some basic dependencies for building:

.. code-block:: bash

    sudo apt install \
        build-essential \
        libfuse-dev \
        pkg-config \
        clang \
        llvm \
        libsqlite3-dev \
        cmake