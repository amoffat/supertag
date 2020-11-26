#!/bin/bash
set -ex

TEST_NAME=$1

if [[ $OSTYPE == linux* ]];
then
    RUST_BACKTRACE=1 cargo test --test integration_tests $TEST_NAME -- --nocapture\
        --test-threads=1 | tee itest.log
elif [[ $OSTYPE == darwin* ]];
then
    # RUST_BACKTRACE=1 can cause a crash on macos
    # https://github.com/rust-lang/rust/issues/44859
    cargo test --test integration_tests $TEST_NAME -- --nocapture --test-threads=1\
        | tee itest.log
else
    echo "unsupported OS"
    exit 1
fi
