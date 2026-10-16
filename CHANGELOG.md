# Changelog

## 0.23

Besides the update to Bevy 0.20, this release includes two big new features, and a lot of ergonomic improvements and fixes:

### 1. Export voxels to `.gltf` or `.vox` for offline voxel processing
Core voxel processing has been moved into an engine-agnostic `vox_bake` crate (with no dependency on bevy). As well as loading `.vox` files, this can now also export `.vox` and `.glb` files. As this is the same engine used in `bevy_vox_scene`, all Magical Voxel material types are supported (although note that cloud volumes are not supported in `.glb` files). 

Gltf export opens the door to offline vox processing. If you want to use voxel models in your application, but don't need to do anything specifically voxelly with them (like quering or changing the voxel data), and you don't care about cloud volumes, you can bake the models to glb offline. This means your application does not need to process voxels at runtime, or have `bevy_vox_scene` as a runtime dependency! Two offlining methods are provided:

1. If your vox assets aren't updated often, the `vox-to-gltf` command-line tool can be used to process them. It has a small footprint because it has no bevy dependency. It can work on a single file, or recursively through a directory.

2. Alternatively, if you have voxel assets that frequently update, there is a bevy vox asset processor, `VoxToGlbProcessor`, in the `bevy_vox_processor` crate. Combine this with bevy's `file_watcher` feature to re-process your vox files and hot reload them as gltfs in your running bevy app.

Both of these offline routes can even work with voxel flipbook animations! The animation code has been moved into the `flipbook` crate (with no dependency on the rest of `bevy_vox_scene`), so that it can be used to play glb flipbook animations output by either the `vox-to-gltf` CLI or `VoxToGlbProcessor`. To see this in action, run the flipbook example: `cargo run -p flipbook --example animation-scene-gltf --features vox_gltf`. `VoxelAnimationPlayer`, `VoxelAnimationFrame` and so on have been renamed to `FlipbookPlayer`, `FlipbookFrame` and moved to `flipbook` crate.

### 2. Improved texturing
- Previous versions of `bevy_vox_scene` supported the various Magica Voxel material types with a set of per-model/ per-palette 16x16 textures for diffuse, metalness, roughness, and emission. Although this was very efficient in terms of tiny texture sizes, it means each voxel colour requires geometry (texcoords) to index the correct voxel material. A flat, multicoloured square surface would need geometry for each colour, instead of being a single quad. This legacy system is still available, as `VoxelTextureMode::Palette`.
- `VoxelTextureMode::Packed` instead creates an island per co-planar face, and packs these islands into a per-model texture. This means multicoloured surfaces use dramatically less geometry, and also opens the door to a couple of optional extra features:
    - Set `TexturePackSettings::ambient_occlusion` to bake distance-weighted volumetric ambient occlusion into an AO texture.
    - Use `TextureMipPolicy` for limited mip chains with mip-safe padding and alignment.
    - Opt-in to packed-atlas utilization and geometry-reduction logging with `TexturePackSettings::log_diagnostics`.
    - We no longer need `@material-no-emission` sub-assets, as emissive properties can be toggled directly on each model's material.

### 3. API ergonomics and hardening
- Procedural generation is now much more ergonomic. The systems that used to require exclusive `&mut World` access have all been replaced by commands on two voxel asset `ParamSet`s:
    - `create_voxel_scene` is now `VoxelSceneAssets::create_scene` 
    - `create_voxel_context` is now `VoxelSceneAssets::add_palette`
    - `create_voxel_animation` is now `VoxelSceneAssets::create_animation`
    - `modify_voxel_model` is now `VoxelModelAssets::modify`

    Now that `World` access isn't needed, `VoxelSceneAssets` or `VoxelModelAssets` can be added to ordinary bevy systems. Scene creation and writing are fallible and return a result (whereas the old systems could panic).
- Remove a few API abstractions that were no longer needed:
    - Remove `VoxelContext`: `VoxelPalette` is now an asset directly, Generated palettes, scenes, and animations are now created through the `VoxelSceneAssets` system parameter.
    - Remove `VoxelData` and `VoxelQueryable`: `VoxelModel` now directly owns the voxel grid and bake settings. SDF generation accepts `VoxBakeSettings`, while generated-scene creation accepts retention and mesh asset usage explicitly.
- Replace `supports_remeshing` with independent `retain_voxel_data` and `retain_mesh_data` settings: 
    - Set `retain_mesh_data` to true when you need CPU queries of the mesh (eg for mesh picking, raycasts, or physics collider generation).
    - Set `retain_voxel_data` to true if you need to query or edit the underlying voxel data. The `VoxelModel` asset will be retained, and a handle to the asset, `VoxelModelSource`, will be added to the spawned instance.
    - Even if both of these are false, a `VoxelModelInstance` and `VoxelPaletteSource` component will still be added, and a `VoxelInstanceReady` event will still fire.
- Cloud volumes can now be modified (previously cloud textures weren't updated when the voxels changed)
- `VoxelRegion::from_center` renamed to `from_center_and_half_extent`

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
