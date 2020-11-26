#!/bin/bash
set -ex
snap run blender -y -b supertag.blend --python-text render
