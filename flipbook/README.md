# flipbook

Flipbook 3D animations in bevy.

Whereas the standard way to animate a 3d object is to animate its transforms, or deform its vertices with a skeleton or shaoe keys, a flipbook animation presents an entirely different mesh each frame.

One usecase is voxel animations, including ones that have been exported to gltf using `vox-to-gltf`. This exporter includes animation meta data as gltf extras, and `flipbook`, when the `vox_gltf` feature is enabled, can parse these and automatically add a `FlipbookPlayer` and `FlipbookFrame`s to the spawned scenes.

## examples

To run the example from the repo root:
```
cargo run -p bevy_flipbook --example animation-scene-gltf --features vox_gltf
```

## tests

To run the tests from the repo root:
```
cargo test -p bevy_flipbook --features vox_gltf
```
