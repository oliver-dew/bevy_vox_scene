# Changelog

## 0.23

- Update to Bevy 0.20
- Generate texture atlases and `StandardMaterial`s per model instead of storing shared material assets in `VoxelContext`. This means that a flat surface containing multiple colors will now just be a single plane. Painting colors (keeping the surface flat) doesn't generate more geometry. This change opens the door to features like baked volumetric Ambient Occlusion and mip-mapping.
- Add `VoxelTextureMode::Packed(TexturePackSettings)`, which merges coplanar faces across palette indices and bakes their PBR properties into configurable packed face islands.
- Add `VoxelTextureMode::Palette` as a lower-texture-memory compatibility mode using per-model 16x16 palette atlases.
- Add optional distance-weighted voxel ambient occlusion for packed atlases through `AmbientOcclusionSettings`.
- Add optional limited mip chains with mip-safe padding and alignment through `TextureMipPolicy`.
- Add opt-in packed-atlas utilization and geometry-reduction logging through `TexturePackSettings::log_diagnostics`.
- Stop generating duplicate `@material-no-emission` sub-assets; emissive properties can be toggled directly on each model's material.
- Add the optional GLB exporter and the independently installable `vox-to-gltf` command-line tool for single-file and recursive directory conversion.

## 0.22

- Update to Bevy 0.19
- To create a minimal update path, `bevy_vox_scene` 0.22 continues to load `WorldAsset`, the new name for the older `Scene` format. A future version will change the loaded format to bsn.

## 0.21

- Update to Bevy 0.18
- Use the new "3d" feature set in the examples for quicker builds

## 0.20.1

- Add "webgpu" feature for WebGPU compatible textures. When the webgpu feature is enabled, textures use R8Unorm and Rgba8Unorm instead of the R16 variants.

## 0.20

- Update to Bevy 0.17

## 0.19

- Update to Bevy 0.16
- `VoxelModel` has been simplified, it no longer contains optional handles to the mesh, material, or cloud image to avoid duplication and potential desyncing of data. `Mesh3d`, `MeshMaterial3d`, `FogVolume` components are added to the voxel scene when it is spawned or loaded, instead of being added after spawn in an observer.
- `VoxelModelInstance` has also been simplified. The change introduced in 0.18 to support animation has been reverted: `VoxelModelInstance` now once again wraps a handle to a single `VoxelModel`. A voxel animation will have a `VoxelModelInstance` for each frame. If you want to access the parent of the animation, try observing the `VoxelAnimationPlayer` being added with `Trigger<OnAdd, VoxelAnimationPlayer>`.
- Procedural voxel generation and modification now use systems instead of custom commands or constructors that take `world`. These can be run as one-shot systems, or pipe output from other systems:
    - Replace `commands.modify_voxel_model` with `modify_voxel_model` system that takes a `VoxelModifier` input. This input now needs to include a handle to the mesh (as the `VoxelModel` no longer contains the mesh handle)
    - `VoxelContext::new` replaced with `create_voxel_context` system. This needs to be run from `world` in order to get the `Handle<VoxelContext>` return value.
    - `VoxelModel::new` replaced with `create_voxel_scene` for single models or `create_voxel_animation` for animations. Instead of returning a handle to a `VoxelModel`, they return `Handle<Scene>`, the same as if you'd loaded them from a MagicaVoxel file. They need to be run from `world` in order to get the `Handle<Scene>` return value.
- If you are going to modify a voxel mesh after it has been spawned you must now indicate that you are going to do this by passing `supports_remeshing: true` in `VoxLoaderSettings`
- `VoxelInstanceSpawned` event replaced with `VoxelInstanceReady`. It is triggered after `SceneInstanceReady`, and is no longer bubbling, and is instead targeted directly at the `SceneRoot` entity
- Add `picking` example to showcase the interaction between Bevy's mesh picking and voxel-space editing

## 0.18

- Support for loading animations from Magica Voxel 0.99.7 files, and also generating animations procedurally. Instead of a single model handle, `VoxelModelInstance` now has a vec of model handles. If you're generating your own animation, you will also need to add a `VoxelAnimationPlayer` component. See the [animation-generation example](./examples/animation-generation.rs). When loading animations from vox files, the loader will add a `VoxelAnimationPlayer` automatically. This can be adjusted using the `VoxelInstanceSpawned` hook. See the [animation-scene example](./examples/animation-scene.rs).
- `VoxelContext::new` now returns an Optional.

## 0.17.1

- Fix srgb color palettes
- `VoxelModelInstance` now has `Transform` and `Visibility` as required components

## 0.17

- MagicaVoxel cloud materials are now imported as volumetric fog textures. See the [`cloud-scene` example](/examples/cloud-scene.rs).
- Because of this change, `VoxelModel`'s mesh and material handles are now optional. This is because if a model consists only of cloud materials, it won't have any surfaces to be meshed.
- `VoxelInstanceSpawned` previously only fired for models that had been named in the Magica Voxel editor. Now it fires for all models, named and unnamed, and also includes their layer name, if a name has been assigned to the model's layer in the Magica Voxel editor.
- Add `uses_srgb` field to `VoxLoaderSettings`, defaulting to true, to more accuratelty match the colors in the Magica Voxel render pane.

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
