# Changelog

## 0.17

- MagicaVoxel cloud materials are now imported as volumetric fog textures. See the [`cloud-scene` example](/examples/cloud-scene.rs).
- Because of this change, `VoxelModel`'s mesh and material handles are now optional. This is because if a model consists only of cloud materials, it won't have any surfaces to be meshed.
- `VoxelInstanceSpawned` previously only fired for models that had been named in the Magica Voxel editor. Now it fires for all models, named and unnamed, and also includes their layer name, if a name has been assigned to the model's layer in the Magica Voxel editor.

## 0.16

- Update to Bevy 0.15
- Add a `pbr_transmission_textures` feature tracking the same-named feature in Bevy. This allows you to disable `pbr_transmission_textures`, freeing up texture slots if you want to instead support other rendering features like percentage-closer soft shadows on macOS or webGL. See [this Bevy PR](https://github.com/bevyengine/bevy/pull/16068) for an explanation. If you disable `pbr_transmission_textures` but still need to use transmissive materials in your scene, you should ensure that each model only contains either solid or transmissive materials, but not a mixture of both: combining solid and transmissive in a single model requires transmission textures. An added bonus of dividing models this way is that the solid sections will be visible through the translucent sections (whereas in a model combining solid and transparent materials, the solid sections of the model won't be visible through the transparent sections).
- Add a `VoxelInstanceSpawned` event that automatically propagates up through the scene hierarchy, allowing you to scope observers to specific branches of your scene.

## 0.15

- Remove `VoxelScene` and `VoxelSceneBundle`: `VoxSceneLoader` now loads Voxel files directly into a Bevy `Scene`
- `VoxelSceneHook` and `VoxelSceneHookBundle` removed in favour of observers
- Ability to inject global `VoxLoaderSettings` into `VoxSceneLoader` as a workaround for the bugs in Bevy where `load_with_settings` ignores settings under [various](https://github.com/bevyengine/bevy/issues/11111) [conditions](https://github.com/bevyengine/bevy/issues/12320).
- Add `UnitOffset` parameter to `VoxLoaderSettings` to override how vertex positions are centered

## 0.14

- Support Bevy 0.14
- Show new bevy rendering features in examples (volumetric fog in [transmissions scene](./examples/transmission-scene.rs) and depth-of-field in [voxel collisions](./examples/voxel-collisions.rs))
- Create a palette from a gradient