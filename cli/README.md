# vox-to-gltf

`vox-to-gltf` converts [MagicaVoxel](https://ephtracy.github.io/) `.vox` files into binary glTF (`.glb`) files for offline asset workflows.

The generated files can be loaded by Bevy's standard glTF loader or any other GLB consumer. Applications only ship the generated assets and do not need a runtime dependency on `bevy_vox_scene`.

## Installation

```sh
cargo install vox-to-gltf --locked
```

The initial implementation reuses the exporter from `bevy_vox_scene`, so installing the tool currently compiles part of Bevy. A planned `vox_bake` extraction will remove Bevy from the CLI dependency tree without changing the command-line interface.

## Usage

Convert one file:

```sh
vox-to-gltf model.vox --output model.glb
```

Convert a directory recursively while preserving relative paths:

```sh
vox-to-gltf assets/vox --output assets/generated
```

Animated voxel input produces one complete GLB per frame using `-001`, `-002`, and subsequent suffixes. Existing files are protected unless `--force` is supplied.

Loader settings that affect the baked output are available as command-line options, including voxel scale, mesh offset, texture layout, material settings, packed-atlas settings, and ambient occlusion. Their defaults come directly from `VoxLoaderSettings::default()`.

```sh
vox-to-gltf model.vox \
  --output model.glb \
  --voxel-size 0.1 \
  --ao-depth 4
```

Run `vox-to-gltf --help` for the complete option list and current defaults.

Cloud-density volumes are currently rejected because glTF has no portable representation equivalent to Bevy's voxel-backed `FogVolume`.
