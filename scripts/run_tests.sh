#!/bin/bash
set -exu

# macos on vm can flip out with lack of fds
ulimit -S -n 2048

cargo test -- --test-threads=1