# Changelog

## 0.15

- Remove `VoxelScene` and `VoxelSceneBundle`: `VoxSceneLoader` now loads Voxel files directly into a Bevy `Scene`
- `VoxelSceneHook` and `VoxelSceneHookBundle` removed in favour of observers
- Ability to inject global `VoxLoaderSettings` into `VoxSceneLoader` as a workaround for [the](https://github.com/bevyengine/bevy/issues/11111) [bugs](https://github.com/bevyengine/bevy/issues/12320) in bevy where `load_with_settings` ignores settings under various conditions.

## 0.14

- Support Bevy 0.14
- Show new bevy rendering features in examples (volumetric fog in [transmissions scene](./examples/transmission-scene.rs) and depth-of-field in [voxel collisions](./examples/voxel-collisions.rs))
- Create a palette from a gradient