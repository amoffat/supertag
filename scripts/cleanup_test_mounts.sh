#!/bin/bash
set -exu

uname_out="$(uname -s)"
case "${uname_out}" in
    Linux*)     machine=Linux;;
    Darwin*)    machine=Mac;;
    CYGWIN*)    machine=Cygwin;;
    MINGW*)     machine=MinGw;;
    *)          machine="UNKNOWN:${uname_out}"
esac

if [ "$machine" == "Mac" ]; then
    df | egrep '^supertag:itest_col' | awk '{print $9}' | xargs -I{} sudo umount -f {}
else
    egrep '^supertag:itest_col /tmp/col-' /etc/mtab | cut -d' ' -f 2 | xargs -I{} sudo umount -f {}
fi
