# vox-to-gltf

`vox-to-gltf` converts [MagicaVoxel](https://ephtracy.github.io/) `.vox` files into binary glTF (`.glb`) files for offline asset workflows.

The generated files can be loaded by Bevy's standard glTF loader or any other GLB consumer. Applications only ship the generated assets and do not need a runtime dependency on `bevy_vox_scene`.

## Installation

To install the latest release from crates.io, run:
```sh
cargo install vox-to-gltf --locked
```
Alternatively, if you are developing vox-to-gltf and want to install from local checkout, run:
```sh
cargo install --path vox-to-gltf vox-to-gltf --locked 
```

The converter depends on the engine-independent `vox_bake` library and does not compile or depend on Bevy.

## Usage

Convert one file:

```sh
vox-to-gltf model.vox --output model.glb
```

Convert a directory recursively while preserving relative paths:

```sh
vox-to-gltf assets/vox --output assets/generated
```

Animated voxel input remains one GLB. Each animated shape contains sibling mesh nodes named with `-001`, `-002`, and subsequent suffixes beneath a common transform. Frame one is visible and later frames carry `KHR_node_visibility: false` plus equivalent `vox_bake` node extras for custom engine setup. Scene-node motion is exported as a standard glTF animation with smooth translation, rotation, and scale channels; `--animation-fps` controls how MagicaVoxel frame indices map to seconds. Existing files are protected unless `--force` is supplied.

Bake settings that affect the output are available as command-line options, including voxel scale, mesh offset, texture layout, material settings, packed-atlas settings, and ambient occlusion. Their defaults come directly from `VoxBakeSettings::default()`.

```sh
vox-to-gltf model.vox \
  --output model.glb \
  --voxel-size 0.1 \
  --ao-depth 4
```

Run `vox-to-gltf --help` for the complete option list and current defaults.

Each generated scene reports its model, mesh, triangle, texture, and byte counts. Supported but lossy conversions produce warnings; for example, PNG images contain only the base mip level even when packed-atlas settings reserve space for additional levels.

Cloud-density volumes are currently rejected because glTF has no portable representation equivalent to Bevy's voxel-backed `FogVolume`.

See the [offline processing guide](https://github.com/Utsira/bevy_vox_scene/blob/main/docs/offline-processing.md) for local checkout installation, CI consistency checks, build-script integration, and loading the generated files without `bevy_vox_scene`.
