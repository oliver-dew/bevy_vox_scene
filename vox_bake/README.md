# vox_bake

Engine-independent types and processing for baking [MagicaVoxel](https://ephtracy.github.io/) assets.

This crate provides the shared bake pipeline used by [`bevy_vox_scene`](https://github.com/Utsira/bevy_vox_scene) and offline conversion tools. It lets command-line tools parse, mesh, and export MagicaVoxel assets without depending on Bevy.

The crate owns shared bake setting types and defaults; MagicaVoxel parsing, material analysis, model-grid ingestion, scene hierarchy and animation tracks; compact voxel storage, material visibility classification, cloud-density extraction, greedy face merging, deterministic atlas layout and painting, mesh geometry and UV generation, and ambient-occlusion baking. Its public API is re-exported from the crate root while implementation is divided into focused modules.

The optional `gltf` feature exposes `export_vox_to_glb` and gates its glTF, PNG, and error dependencies. These dependencies are absent from default runtime builds.

`bake_model` is the engine-independent per-model entry point. It classifies a `VoxelGrid` against a `BakedPalette` and returns optional surface and cloud-volume content plus optical metadata.

`evaluate_baked_scene_frame` produces an owned static node hierarchy for one animation frame using hold-last semantics. Output adapters resolve its numeric model references without accessing Bevy entities or asset handles.

`bake_scene` is the complete adapter-neutral entry point. It returns a `BakedScene` containing the analyzed palette, named model grids and bake outputs, layer metadata, hierarchy, and unevaluated transform and shape-model tracks. Runtime adapters can preserve those tracks, while snapshot formats can evaluate them with `evaluate_baked_scene_frame`.

The `source` module owns MagicaVoxel parsing and source-format types. Runtime adapters therefore do not need a direct dependency on the underlying parser crate.

Shared material color helpers provide the linear and sRGB encodings needed by output adapters without depending on Bevy color types.

The optional exporter returns scene statistics and structured warnings for supported but lossy conversions. Animated voxel models are emitted as sibling frame nodes using `KHR_node_visibility`. See the [repository's offline processing guide](https://github.com/Utsira/bevy_vox_scene/blob/main/docs/offline-processing.md) for CLI, CI, and build-script workflows.
