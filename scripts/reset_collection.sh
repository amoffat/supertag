#!/bin/bash
set -exu

collection=$1

if [[ $OSTYPE == linux* ]];
then
    rm -rf $HOME/.config/supertag/collections/$collection
elif [[ $OSTYPE == darwin* ]];
then
    rm -rf $HOME/Library/Preferences/ai.supertag.SuperTag/collections/$collection
    rm -rf "$HOME/Library/Application Support/ai.supertag.supertag/managed_files/"
else
    echo "unsupported OS"
    exit 1
fi