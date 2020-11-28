.. image:: https://raw.githubusercontent.com/amoffat/supertag/master/logo/logo.gif
    :target: https://amoffat.github.io/supertag/
    :alt: Logo

|

.. image:: https://img.shields.io/travis/amoffat/supertag/master.svg?style=flat-square
    :target: https://travis-ci.org/amoffat/supertag
    :alt: Build Status
.. image:: https://img.shields.io/badge/Documentation-v0.1.1-brightgreen?style=flat-square&logo=read-the-docs&logoColor=white&color=1a6cff
    :target: https://amoffat.github.io/supertag/
    :alt: Docs

|

Supertag is a tag-based filesystem, written in Rust, for Linux and MacOS. It provides a tag-based view of your files by
removing the hierarchy constraints typically imposed on files and folders.

.. image:: https://raw.githubusercontent.com/amoffat/supertag/master/images/intersection-opt.gif
    :alt: Intersection

Installation
============

Linux
-----

.. code-block:: bash

    curl -Ls https://github.com/amoffat/supertag/releases/latest/download/supertag-v0.1.1-x86_64.AppImage > tag
    sudo mv tag /usr/local/bin/tag
    sudo chmod +x /usr/local/bin/tag

Mac
---

.. code-block:: bash

    brew install amoffat/rnd/supertag
