Development
###########

So you've decided to tinker with Supertag. Great! Let's get you set up. First you'll want to make sure your
development environment is setup. See :ref:`Development Install <dev_install>`.

Running tests
*************

Running the test suite is easy:

.. code-block:: bash

    scripts/run_tests.sh

If you're on Linux, this runs about ~110 tests and finishes pretty quickly. If you're on MacOS, I believe it is around
~140 tests, and is much slower. This is because we have to run with ``--test-threads=1`` due to some weird threading
quirks.

If a test breaks, and you want to run it individually, run the following:

.. code-block:: bash

    STAG_LOG=1 scripts/itest.sh test_funky_name

This runs the ``test_funky_name`` test. ``STAG_LOG=1`` will enable full output logging at the trace level and higher.
This is necessary for debugging the really tricky bugs.

.. note::

    ``STAG_LOG=1`` also tee's the output to ``./itest.log``, so you don't need to less/grep the test output yourself,
    just grep that logging file.

Writing tests
*************

The best advice for writing Supertag tests is to look at the existing tests to see how they work. They're readable
and mostly straightforward (as much as they can be).

Supporting all usages
=====================

As outlined in :ref:`usage`, there are 3 different ways of accomplishing most operations. Writing a good test means
covering these 3 methods. We don't want new functionality that only works for the file browser GUI, but not the
commandline, for example.

To make sure our bases are covered, we use the following pattern: for any given test, you will want to write 3 different
versions of it, all using the same underlying logic. Here is an example of a real Supertag test that tests that when
you remove a tag directory, everything works as expected:

.. code-block:: rust

    #[test]
    fn test_rm_tagdir_cli() -> TestResult {
        let th = TestHelper::new(None);
        _test_rm_tagdir(th)
    }

    #[test]
    fn test_rm_tagdir_manual() -> TestResult {
        let mut th = TestHelper::new(None);
        th.symlink_mode = OpMode::MANUAL;
        th.rm_mode = OpMode::MANUAL;
        th.rmdir_mode = OpMode::MANUAL;
        _test_rm_tagdir(th)
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_rm_tagdir_finder() -> TestResult {
        let mut th = TestHelper::new(None);
        th.symlink_mode = OpMode::FINDER;
        th.rm_mode = OpMode::FINDER;
        th.rmdir_mode = OpMode::FINDER;
        _test_rm_tagdir(th)
    }

    /// Tests removing a leaf tag removes the tag from all of the files in the path intersection
    fn _test_rm_tagdir(th: TestHelper) -> TestResult {
        // ...
        Ok(())
    }

Notice the naming convention used here. We have one function (the last function) that captures the "meat" of the test,
and it is named ``_TEST_NAME``. Then we have 3 ancillary functions (the first 3) and they are named:

- ``TEST_NAME_cli``
- ``TEST_NAME_manual``
- ``TEST_NAME_finder``

The job of an ancillary test is to instantiate the ``TestHelper`` struct and then set it up to behave as either a
the tag binary (``OpMode::CLI``), a manual system program (``OpMode::MANUAL``), or as the MacOS Finder gui
(``OpMode::FINDER``). Then the ancillary test should call the function that has the meat of the test.

We set up these behaviors by setting different mode attributes on the ``TestHelper``.
Here is a (currently) complete list of the different modes we can set:

symlink_mode
    Affects how linking a file to a tag is performed.

rmdir_mode
    Affects how removing a tag or tag group is performed.

rm_mode
    Affects how removing a *file* in Supertag is performed.

mkdir_mode
    Affects how creating a tag is performed.

rename_mode
    Affects how moving a tag or a file is performed.

import_mode
    Affects how a file is imported into Supertag.

Each of these modes attempts to behave as if the user performed the action from the tag binary, manually with a
standard OS binary, or from MacOS Finder. The default behavior for all modes is ``OpMode::CLI``, which corresponds to
the tag binary. Different ``OpMode`` may result in drastically different behavior, for example, when using
``symlink_mode``, the following operations are performed:

- ``OpMode::CLI``: file is linked to tag by calling the ``supertag::ln`` function.
- ``OpMode::MANUAL``: file is linked to the tag by calling the ``ln`` system binary.
- ``OpMode::FINDER``: file is linked to the tag by creating and writing an :ref:`alias file`.

In summary, try to write tests that cover the functionality you're testing from the different ways it can be used.

When things go wrong
=====================

Things can and will go wrong when writing your tests. You can end up in different states that are not good for your
system, for example: many temporary Supertag filesystems mounted.

Cleaning up filesystems
-----------------------

The following script will do its best to kill off zombie Supertag filesystems:

.. code-block:: bash

    scripts/cleanup_test_mounts.sh

It is not perfect, however. For example, as it is based on a specific filesystem naming scheme, in this case
``supertag:itest_col``, it will not clean up zombie Supertag filesystems from non-test runs. For those, you will need
to run a force umount directly:

.. code-block:: bash

    sudo umount -f the_supertag_filesystem_name


Contributing
************

CLA
===

If you'd like to make a contribution back upstream to Supertag, please be aware that we require a `Contributors
License Agreement <https://s3-us-west-2.amazonaws.com/supertag.ai/cla/index.html>`_ (CLA). This is because we
may offer a dual-licensed version of Supertag in the future.