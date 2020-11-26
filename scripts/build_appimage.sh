#!/bin/bash

set -exu
DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" >/dev/null 2>&1 && pwd )"

tag_bin="$1"
version="$2"
dist="$3"
supertag="$DIR"/..
data="$supertag"/data/appimage
signkey="EB19C5A7D839413DC078E074D2D5C7DFE8DA08B1"

cd "$supertag"

appdir=$(mktemp -d -t "supertag-appimage-XXXXXXXX")

cd "$appdir"
cp "$tag_bin" .
chmod +x ./tag

cp "$data"/AppRun .
chmod +x ./AppRun

mkdir -p ./usr/lib

# technically fuse is not needed, because for an appimage to run, the user must already have fuse installed. however,
# our appimage test framework extracts the appimage, without mounting it, so the test framework doesn't use fuse. this
# means supertag won't have fuse. so we'll include fuse explicitly
cp /lib/x86_64-linux-gnu/libfuse.so.2 usr/lib/
cp /usr/lib/x86_64-linux-gnu/libsqlite3.so.0 usr/lib

cp /lib/x86_64-linux-gnu/libmount.so.1 usr/lib

cp "$supertag"/logo/512.png ./supertag.png
cp "$data"/tag.desktop .

/home/amoffat/Applications/appimagetool-x86_64.AppImage --sign --sign-key "$signkey" "$appdir" "$dist"/supertag
