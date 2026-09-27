# Offline processing

### Context

`bevy_vox_scene` currently converts MagicaVoxel `.vox` files into Bevy meshes, images, materials, and scene hierarchies when the application loads the asset. This is useful for applications that need to load arbitrary `.vox` files, generate voxel content, edit voxels, or remesh models at runtime. It also means that applications which only use fixed, authored voxel scenes must ship the `.vox` parser, mesher, texture-atlas baker, scene conversion code, and their dependencies even though the result never changes after development.

Some users instead want `.vox` to be a source format: convert it during development or in CI, ship only the converted asset, and load it using Bevy's normal asset support. This should remove voxel processing from application startup and allow the final application to omit `bevy_vox_scene` entirely when it does not use its runtime features.

"Offline" means that parsing, greedy meshing, atlas packing, material conversion, and optional ambient-occlusion baking happen before distribution. Meshes and textures will still be read and uploaded to the GPU at runtime like any other render asset.

### Proposal

Add an offline exporter in this repository with GLB as its first output format. GLB is a good match for the static result of the current loader: it can contain indexed meshes, embedded textures, PBR materials, named scene nodes, transforms, hierarchy, shared mesh references, and standard material extensions in one file. Bevy can load that file without a `bevy_vox_scene` dependency.

Structure the implementation as three layers:

1. A Bevy-independent `vox_bake` workspace crate parses and bakes voxel data into an owned representation of surface geometry, material images and parameters, optional cloud-density volumes, scene nodes, and animation tracks. This representation describes the complete voxel scene rather than only the subset supported by glTF. It owns the expensive classification, meshing, texture, material, hierarchy, and frame-selection work used by every adapter.
2. Output adapters consume `vox_bake` results according to their capabilities. `bevy_vox_scene` creates Bevy meshes, images, materials, `FogVolume` entities, and flipbook animation entities. An optional glTF adapter evaluates animation tracks into static snapshots and converts supported surface content to GLB; it rejects cloud volumes through an explicit policy because glTF cannot represent their density fields. The glTF and PNG dependencies remain optional and do not enter the Bevy runtime-only dependency graph.
3. The `vox-to-gltf` CLI package depends on `vox_bake` and its glTF adapter, plus command-line and filesystem concerns. It must not depend on Bevy or `bevy_vox_scene`. Applications consuming its output do not depend on any of these crates.

The resulting dependency direction should be:

```text
bevy_vox_scene -> vox_bake <- vox-to-gltf
       |                         |
      bevy                    gltf/png/clap
```

If `vox-to-gltf` is distributed through crates.io, `vox_bake` must also be published there because registry packages cannot depend only on an unpublished workspace path crate. `vox_bake` can remain an implementation-focused package in this repository even though it is independently versioned and published.

The initial command should support an explicit conversion such as:

```text
vox-to-gltf input.vox --output output.glb
```

Loader settings that affect the baked result, including voxel size, mesh offset, outer-face meshing, texture mode, mip policy, emission strength, and ambient occlusion, should be expressible through CLI arguments and/or a settings file. The exporter should use the same defaults and validation as runtime loading.

Use the optional [`gltf`](https://crates.io/crates/gltf) dependency to construct typed `gltf::json` data and `gltf::binary::Glb` to write the final container. The exporter remains responsible for building aligned binary buffers, accessors, buffer views, embedded image data, materials, meshes, nodes, and extension declarations. Atlas images should initially be embedded as PNG data. A direct `gltf-json` dependency is unnecessary because `gltf` re-exports it as `gltf::json`.

#### GLB feature mapping

- Export mesh positions, normals, UV0, optional AO UV1, and indices through glTF accessors and buffer views.
- Export packed or palette atlas images, preserving the correct linear or sRGB interpretation for each material property.
- Map base color, metallic, roughness, occlusion, alpha behavior, and emissive properties to glTF PBR materials.
- Use the relevant glTF material extensions for emissive strength, transmission, index of refraction, and volume properties where Bevy's glTF loader supports them.
- Preserve scene hierarchy, node names, transforms, layer names where possible, and instancing by allowing multiple nodes to reference one exported mesh.
- Preserve useful MagicaVoxel or `bevy_vox_scene` metadata in node `extras` only when it has a documented meaning; loading the GLB must not require that metadata.
- Record every extension used by the document and validate the finished file with an independent glTF validator.

Some current features do not have a direct portable glTF representation:

- Cloud materials currently become Bevy `FogVolume` entities backed by 3D density textures. `KHR_materials_volume` describes transmission through a surface material and is not a replacement for a volumetric density field. The first version should report cloud models as unsupported, with an explicit policy to fail, omit clouds, or export only their non-cloud geometry. Silent loss should not be the default.
- Voxel animation currently works by switching visibility among separately meshed frames. Core glTF animation targets transforms and morph weights, so each animation frame should instead be exported as a separate, complete GLB. Given an output path such as `walk.glb`, frames are named `walk-001.glb`, `walk-002.glb`, and so on. Numbering is one-based and padded to at least three digits; animations longer than 999 frames use the digits required without truncation. This preserves the exact meshed appearance of each frame and lets applications choose their own playback or asset-array system without depending on `bevy_vox_scene`.
- Runtime voxel modification, queries, and remeshing require voxel data and runtime systems. They are outside the portable GLB output and remain features of `bevy_vox_scene`.
- PNG does not store a precomputed mip chain. The initial exporter should document that the consuming renderer generates mipmaps, or reject/ignore the current CPU-generated mip levels with a diagnostic. KTX2 can be considered later if retaining baked mip levels becomes important.

A Bevy asset processor should be a later adapter over the same exporter. It would add automatic dependency tracking, caching, per-asset metadata, and file watching, but it does not remove the need for an output format. Its deployed output should remain an ordinary GLB loadable by Bevy's glTF loader. The standalone CLI comes first because it is easy to run manually or in CI, has an explicit output location, and proves that the converter is independent of an application's runtime setup.

### Implementation plan

#### Scope and public behavior

- [x] Define the first release as static `.vox` scene/model to `.glb` conversion, with animated content exported as an ordered sequence of static GLB files.
- [x] Document which `VoxLoaderSettings` affect export and inherit omitted CLI options from the runtime loader defaults.
- [x] Define explicit policies for cloud content, hidden layers, and unsupported material properties. The first implementation rejects cloud content and omits hidden nodes.
- [x] Define animation export as one complete scene snapshot per frame, using one-based `-001`, `-002`, ... filename suffixes.
- [x] Define how scene animations with tracks of different lengths select a frame after a shorter track ends, matching the current runtime repeat/hold behavior where possible. Shorter tracks hold their last frame.
- [x] Make unsupported or lossy conversion visible through structured warnings and errors; do not silently discard content by default.
- [ ] Define deterministic output requirements so identical input and settings produce byte-identical GLB files where practical.
- [x] Add the public `export_vox_to_glb(bytes, settings) -> Result<Vec<ExportedGlb>, ExportError>` library entry point.

#### Shared bake representation

- [ ] Treat the full baked-scene refactor as a later consolidation step. The working GLB exporter may continue to reuse the existing `VoxelData`, meshing, palette, naming, and transform helpers until another consumer justifies the broader abstraction.
- [ ] Add a `vox_bake` package to the workspace with no dependency on `bevy` or `bevy_vox_scene`.
- [ ] Move `.vox` parsing, bake-relevant settings and defaults, voxel classification, meshing, texture packing, AO, material analysis, cloud-density extraction, scene hierarchy, and animation-track evaluation into `vox_bake`.
- [ ] Define Bevy-independent data using owned arrays, pixel buffers, explicit formats, numeric resource indices, and either plain transform arrays or a small engine-neutral math dependency. Do not expose Bevy `Mesh`, `Image`, `StandardMaterial`, handles, entities, or asset contexts.
- [ ] Separate `.vox` parsing and baking from `AssetLoader`, `LoadContext`, and Bevy asset handles.
- [ ] Introduce an owned baked-scene representation containing surface meshes, material parameters, 2D image pixels and sampling information, optional 3D cloud-density volumes, nodes, transforms, hierarchy, names, animation tracks, and shared-resource indices.
- [ ] Represent each model as optional surface and volume content so mixed models retain both parts. Do not reduce the shared representation to glTF's supported feature set.
- [ ] Store animation as frame-indexed model and transform references independent of playback implementation. The Bevy adapter should build flipbook entities and `VoxelAnimationPlayer`; the GLB adapter should evaluate the same tracks into one static scene snapshot per output frame.
- [ ] Give each output adapter an explicit unsupported-content policy. The initial GLB adapter fails on cloud volumes; future policies may omit them or export only surface content, but must report the loss.
- [ ] Preserve the existing coordinate conversion from MagicaVoxel's left-handed Z-up space to Bevy/glTF-compatible right-handed Y-up space in one well-tested place.
- [ ] Refactor voxel classification, cloud-density extraction, greedy meshing, per-mesh texture packing, AO baking, material analysis, hierarchy construction, and animation frame selection so both runtime loading and export consume the same results.
- [ ] Split bake settings from Bevy runtime settings. Keep `supports_remeshing` and other runtime-only choices in `bevy_vox_scene`; source every shared default used by the CLI and runtime loader from `vox_bake`.
- [ ] Preserve the existing `bevy_vox_scene` settings API where practical through re-exports or conversion into `vox_bake` settings, and document any unavoidable migration.
- [ ] Keep Bevy-specific mesh, image, material, `FogVolume`, animation-entity, and asset registration work in an adapter from the baked representation into `LoadContext`.
- [ ] Keep GLB buffer construction, PNG encoding, glTF material mapping, static frame evaluation, and unsupported-feature errors in the export adapter.
- [ ] Move `vox-to-gltf` from its temporary dependency on `bevy_vox_scene` to `vox_bake`, then verify `cargo tree -p vox-to-gltf` contains no Bevy crates.
- [ ] Decide whether the glTF adapter lives behind an optional `gltf` feature in `vox_bake` or in a small separate library target owned by `vox-to-gltf`; retain a reusable library API and avoid duplicating bake logic.
- [ ] Preserve `bevy_vox_scene::export_vox_to_glb` as a compatibility re-export or document its replacement before removing it.
- [ ] Verify the refactor does not change existing runtime asset labels, scene behavior, or generated voxel APIs.

#### GLB writer

- [x] Add an `export` Cargo feature with optional `gltf` and image-encoding dependencies.
- [x] Build one aligned binary buffer containing vertex attributes, indices, and embedded image payloads.
- [x] Generate valid buffer views and typed accessors, including position bounds required by glTF.
- [x] Convert Bevy mesh topology and index formats to glTF triangle primitives.
- [x] Encode generated atlas images as PNG and attach the correct MIME type, texture sampler, and material slot.
- [x] Map material constants and textures to glTF metallic-roughness PBR properties with correct channels and color spaces.
- [x] Implement and declare supported material extensions for emissive strength, transmission, IOR, and volume.
- [x] Export scene nodes, transforms, hierarchy, names, and shared mesh references without duplicating instanced geometry.
- [x] Pad JSON and binary chunks according to the GLB alignment requirements and write them with `gltf::binary::Glb`.
- [x] Return structured errors for unsupported mesh attributes, oversized buffers, image encoding failures, and unsupported scene features.

#### CLI workspace package

- [x] Add a separate `vox-to-gltf` package to the Cargo workspace.
- [x] Keep argument parsing, filesystem traversal, logging, and process exit behavior in the CLI package rather than the library.
- [x] Support a single input and explicit output path first.
- [x] Add `--force` protection against accidental overwrite.
- [x] For animated input, derive every frame path from the requested output path and check the whole output set for collisions before writing any frame.
- [x] Expose the stable bake settings needed for reproducible output. Consider a serialized settings file if future options make command-line flags unwieldy.
- [ ] Print a concise conversion summary including frame, model, mesh, triangle, texture, and output-size counts plus any lossy-conversion warnings.
- [x] Add recursive directory conversion with source-relative output paths and batch-wide overwrite protection.

#### Validation

- [x] Export an existing model fixture and parse every generated GLB through the independent `gltf` reader.
- [x] Export a multi-model scene and verify hierarchy, transforms, names, hidden-node policy, and shared mesh references.
- [x] Export an animation and verify frame count, zero-based API indices, valid per-frame GLBs, and one-based zero-padded CLI filenames.
- [ ] Verify an animated export can be loaded as an ordered `Handle<Gltf>` collection and played without `bevy_vox_scene`.
- [ ] Add coverage for packed and palette texture modes, constant material properties, AO UV1, emission, metalness/roughness, alpha, and glass.
- [ ] Compare exported mesh attributes and decoded texture pixels with the runtime loader's baked output.
- [ ] Validate every test export with the Khronos glTF Validator and treat specification errors as test failures.
- [x] Add initial round-trip inspection using the `gltf` reader, including meshes and the embedded binary buffer. Extend this with field-level assertions as more fixtures are added.
- [ ] Render representative runtime-loaded and GLB-loaded scenes in Bevy and compare them visually, allowing only documented differences.
- [ ] Verify conversion on native platforms and loading on native, WebGPU, WebGL2, and WebAssembly targets supported by the project.
- [ ] Measure conversion time, output size, runtime load time, and final dependency/build-size reduction on representative scenes.

#### Bevy asset processor follow-up

- [ ] Implement a Bevy `Process` or `LoadTransformAndSave` adapter that invokes the shared GLB exporter.
- [ ] Map per-asset `.meta` settings to the same bake and export settings used by the CLI.
- [ ] Ensure changes to source `.vox` files or settings invalidate and rebuild the processed GLB.
- [ ] Confirm a release application can consume only processed GLB assets without registering `VoxScenePlugin`.
- [ ] Document development and CI workflows for both the CLI and asset processor.

### Dependencies

- `vox_bake` should retain the existing engine-neutral parsing and baking dependencies such as `dot_vox`, `block-mesh`, `ndshape`, and `rectangle-pack`, plus only the small math or serialization dependencies required by its public data model. It must not depend on Bevy.
- [`gltf`](https://crates.io/crates/gltf), optional to the glTF adapter, for typed glTF JSON structures, validation helpers, and the GLB container writer.
- A PNG encoder for embedding the generated 2D texture atlases. Prefer reusing an existing transitive/image dependency only if it offers a stable direct API; otherwise add the smallest suitable optional dependency explicitly.
- [`clap`](https://crates.io/crates/clap) in `vox-to-gltf` for generated help, argument validation, and a maintainable interface as more bake settings are exposed.

The shipped game does not need the CLI, glTF, or PNG-writing dependencies when it consumes the exported GLB. Runtime-only `bevy_vox_scene` users depend on `vox_bake` for shared processing but do not compile the glTF adapter. The `vox-to-gltf` dependency tree contains `vox_bake` and export dependencies but no Bevy crates.
