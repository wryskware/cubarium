#!/bin/sh
# Compile every shader beside its source. Run on the desktop; the .spv files are
# checked in because `shaderc` needs cmake and a C++ toolchain the board does not
# have, and the brief allows pre-compiled SPIR-V with the GLSL beside it.
set -eu
cd "$(dirname "$0")"
for s in background.frag water.frag sprite.vert sprite.frag present.frag fullscreen.vert voxel.frag bloom.frag bloom_smooth.frag figure.vert figure.frag composite.vert composite.frag figure_sun.vert figure_sun.frag; do
    glslc --target-env=vulkan1.1 -O -I. -o "$s.spv" "$s"
    echo "  $s -> $s.spv"
done
# The plant pass (`crate::plants`): its vertex stage, and `voxel.frag` again as its two
# fragment stages, so they share the walk's face functions.
glslc --target-env=vulkan1.1 -O -I. -o plant.vert.spv plant.vert
echo "  plant.vert -> plant.vert.spv"
for stage in vis shade; do
    define=$(echo "$stage" | tr a-z A-Z)
    glslc --target-env=vulkan1.1 -O -I. -fshader-stage=frag -DPLANT_$define -o "plant_$stage.frag.spv" voxel.frag
    echo "  voxel.frag (PLANT_$define) -> plant_$stage.frag.spv"
done
