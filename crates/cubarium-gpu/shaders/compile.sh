#!/bin/sh
# Compile every shader beside its source. Run on the desktop; the .spv files are
# checked in because `shaderc` needs cmake and a C++ toolchain the board does not
# have, and the brief allows pre-compiled SPIR-V with the GLSL beside it.
set -eu
cd "$(dirname "$0")"
for s in background.frag water.frag sprite.vert sprite.frag present.frag fullscreen.vert voxel.frag; do
    glslc --target-env=vulkan1.1 -O -I. -o "$s.spv" "$s"
    echo "  $s -> $s.spv"
done
