# Offline processing

### Context

`bevy_vox_scene` currently converts MagicaVoxel `.vox` files into Bevy meshes, images, materials, and scene hierarchies when the application loads the asset. This is useful for applications that need to load arbitrary `.vox` files, generate voxel content, edit voxels, or remesh models at runtime. It also means that applications which only use fixed, authored voxel scenes must ship the `.vox` parser, mesher, texture-atlas baker, scene conversion code, and their dependencies even though the result never changes after development.

Some users instead want `.vox` to be a source format: convert it during development or in CI, ship only the converted asset, and load it using Bevy's normal asset support. This should remove voxel processing from application startup and allow the final application to omit `bevy_vox_scene` entirely when it does not use its runtime features.

"Offline" means that parsing, greedy meshing, atlas packing, material conversion, and optional ambient-occlusion baking happen before distribution. Meshes and textures will still be read and uploaded to the GPU at runtime like any other render asset.

### Proposal

The repository now provides an offline exporter with GLB as its first output format. GLB is a good match for the static result of the current loader: it can contain indexed meshes, embedded textures, PBR materials, named scene nodes, transforms, hierarchy, shared mesh references, and standard material extensions in one file. Bevy can load that file without a `bevy_vox_scene` dependency.

Structure the implementation as three layers:

1. A Bevy-independent `vox_bake` workspace crate parses and bakes voxel data into an owned representation of surface geometry, material images and parameters, optional cloud-density volumes, scene nodes, and animation tracks. This representation describes the complete voxel scene rather than only the subset supported by glTF. It owns the expensive classification, meshing, texture, material, hierarchy, and frame-selection work used by every adapter.
2. Output adapters consume `vox_bake` results according to their capabilities. `bevy_vox_scene` creates Bevy meshes, images, materials, `FogVolume` entities, and flipbook animation entities. An optional glTF adapter converts surface content to GLB and represents each animated shape as sibling frame nodes using `KHR_node_visibility`; it rejects cloud volumes through an explicit policy because glTF cannot represent their density fields. The glTF and PNG dependencies remain optional and do not enter the Bevy runtime-only dependency graph.
3. The `vox-to-gltf` CLI package depends on `vox_bake` and its glTF adapter, plus command-line and filesystem concerns. It must not depend on Bevy or `bevy_vox_scene`. Applications consuming its output do not depend on any of these crates.

The three-layer architecture is implemented. `vox_bake` owns source parsing, settings, complete scene and model baking, classification, meshing, atlas packing and painting, AO, material analysis, volume extraction, frame evaluation, and the optional GLB writer. The CLI depends directly on `vox_bake` and contains no Bevy dependency. The Bevy adapter consumes the same owned `BakedScene` while preserving runtime hierarchy, labeled assets, cloud entities, and flipbook animation components.

The resulting dependency direction should be:

```text
bevy_vox_scene -> vox_bake <- vox-to-gltf
       |                         |
      bevy                    gltf/png/clap
```

If `vox-to-gltf` is distributed through crates.io, `vox_bake` must also be published there because registry packages cannot depend only on an unpublished workspace path crate. `vox_bake` can remain an implementation-focused package in this repository even though it is independently versioned and published.

The CLI supports explicit conversion such as:

```text
vox-to-gltf input.vox --output output.glb
```

Loader settings that affect the baked result, including voxel size, mesh offset, outer-face meshing, texture mode, mip policy, emission strength, and ambient occlusion, are available as CLI arguments. The CLI and runtime loader derive their defaults from `VoxBakeSettings`. A serialized settings file can be added if future options make command-line flags unwieldy.

Use the optional [`gltf`](https://crates.io/crates/gltf) dependency to construct typed `gltf::json` data and `gltf::binary::Glb` to write the final container. The exporter remains responsible for building aligned binary buffers, accessors, buffer views, embedded image data, materials, meshes, nodes, and extension declarations. Atlas images should initially be embedded as PNG data. A direct `gltf-json` dependency is unnecessary because `gltf` re-exports it as `gltf::json`.

#### GLB feature mapping

- Export mesh positions, normals, UV0, optional AO UV1, and indices through glTF accessors and buffer views.
- Export packed or palette atlas images, preserving the correct linear or sRGB interpretation for each material property.
- Map base color, metallic, roughness, occlusion, alpha behavior, and emissive properties to glTF PBR materials.
- Use the relevant glTF material extensions for emissive strength, transmission, index of refraction, and volume properties where Bevy's glTF loader supports them.
- Preserve scene hierarchy, node names, transforms, hidden-layer behavior, and instancing by allowing multiple nodes to reference one exported mesh. Exporting layer names remains future metadata work.
- Preserve useful MagicaVoxel or `bevy_vox_scene` metadata in node `extras` only when it has a documented meaning; loading the GLB must not require that metadata.
- Record every extension used by the document and validate the finished file with an independent glTF validator.

Some current features do not have a direct portable glTF representation:

- Cloud materials currently become Bevy `FogVolume` entities backed by 3D density textures. `KHR_materials_volume` describes transmission through a surface material and is not a replacement for a volumetric density field. The first version should report cloud models as unsupported, with an explicit policy to fail, omit clouds, or export only their non-cloud geometry. Silent loss should not be the default.
- Voxel animation works by switching visibility among separately meshed frames. Each animated shape is exported inside the same GLB as sibling mesh nodes under its original transform. Siblings are named `shape-001`, `shape-002`, and so on; numbering is one-based and padded to at least three digits. Frame one is visible and later frames use the ratified `KHR_node_visibility` extension with `visible: false`. Equivalent frame-index and initial-visibility metadata is stored in node extras so engines without extension support can initialize a flipbook component. Animated transform tracks currently retain only their first transform and produce a warning.
- Runtime voxel modification, queries, and remeshing require voxel data and runtime systems. They are outside the portable GLB output and remain features of `bevy_vox_scene`.
- PNG does not store a precomputed mip chain. The exporter currently embeds only the base image; mip policy still affects atlas padding and alignment, but the loss of baked mip levels is not yet reported. Add a diagnostic and document renderer-side mip generation. KTX2 can be considered later if retaining baked mip levels becomes important.

A Bevy asset processor should be a later adapter over the same exporter. It would add automatic dependency tracking, caching, per-asset metadata, and file watching, but it does not remove the need for an output format. Its deployed output should remain an ordinary GLB loadable by Bevy's glTF loader. The standalone CLI comes first because it is easy to run manually or in CI, has an explicit output location, and proves that the converter is independent of an application's runtime setup.

### Implementation plan

#### Scope and public behavior

- [x] Define the first release as static `.vox` scene/model to `.glb` conversion, with animated content exported as an ordered sequence of static GLB files.
- [x] Document which `VoxLoaderSettings` affect export and inherit omitted CLI options from the runtime loader defaults.
- [ ] Define explicit policies for every unsupported material property and source metadata field. Cloud content already fails explicitly and hidden nodes are omitted, but remaining unmapped properties need an audited policy.
- [x] Define animation export as one GLB containing per-shape sibling mesh frames, using one-based `-001`, `-002`, ... node-name suffixes and shared parent transforms.
- [x] Preserve independent animated shapes with differing frame counts rather than expanding them into scene-wide snapshots. Each shape retains exactly its own model frames.
- [ ] Report every unsupported or lossy conversion through structured warnings or errors. Cloud volumes fail explicitly, but mip-policy loss and any unmapped source metadata still need diagnostics.
- [x] Require identical input and settings to produce byte-identical GLB files and cover this with an exporter regression test.
- [x] Add the public `export_vox_to_glb(bytes, settings) -> Result<ExportedGlb, ExportError>` library entry point.

#### Shared bake representation

- [x] Replace the exporter's temporary use of Bevy-side `VoxelData`, meshing, palette, naming, and transform helpers with the shared bake pipeline before moving the writer.
- [x] Add a `vox_bake` package to the workspace with no dependency on `bevy` or `bevy_vox_scene`.
- [x] Move engine-neutral AO, texture mode, texture packing, mip policy, and unit-offset types and defaults into `vox_bake`, with compatibility re-exports from `bevy_vox_scene`.
- [x] Add `VoxBakeSettings` as the source of bake defaults, while keeping `supports_remeshing` in the Bevy-specific `VoxLoaderSettings` wrapper.
- [x] Add an owned `BakedMesh` with position, normal, UV0, optional UV1, and index arrays. Make the exporter consume it directly and convert it to a Bevy `Mesh` only at runtime adapter boundaries.
- [x] Add `BakedImage`, explicit pixel formats, and `BakedMeshAtlas`; move palette-index and AO image data into the shared atlas result consumed by both adapters.
- [x] Add `BakedVolume` for cloud density fields and convert it to a Bevy 3D image only in the runtime adapter; keep the GLB adapter's explicit unsupported-volume error.
- [x] Move compact raw-voxel storage, mesher-visible voxel types, material visibility classification, and padded cloud-density extraction into `vox_bake`. The shared classifier owns packed/palette merge keys, translucent IOR aggregation, and exclusion of cloud voxels from surface geometry.
- [x] Move greedy face merging into `vox_bake` and expose engine-neutral baked quads. Keep vertex generation, atlas painting, and AO in the existing pipeline until they can move as one coherent atlas-building stage.
- [x] Move deterministic atlas rectangle packing and its `rectangle-pack` dependency into `vox_bake`. Material and AO atlas layouts now share the engine-neutral packer, including padding and mip-alignment rules.
- [x] Split `vox_bake` into focused `settings`, `voxel`, `mesh`, `atlas`, and `output` modules while retaining a flat public API through crate-root re-exports.
- [x] Move per-quad position, normal, index, material UV, palette UV, and AO UV generation into `vox_bake`. The Bevy-side pipeline now assembles shared geometry and UV results rather than calculating them with Bevy math types.
- [x] Move palette-index atlas painting and the complete voxel-residency AO implementation into dedicated `vox_bake` modules, including stencil generation, distance weighting, intensity sanitization, edge padding, and AO image painting.
- [x] Add engine-neutral `BakedMaterial` and `BakedPalette` records and move MagicaVoxel material interpretation into `vox_bake`, including emission/radiant-flux scaling, diffuse roughness, metalness, transmission, IOR, cloud density, color-space metadata, and derived classification tables. Keep Bevy `Color` and `StandardMaterial` construction in the Bevy adapter.
- [x] Add an optional `gltf` feature to `vox_bake` which gates the `gltf`, `png`, and exporter-error dependencies. Default `vox_bake` builds remain free of those dependencies.
- [x] Move model-name traversal, animation snapshot count, and MagicaVoxel-to-right-handed-Y-up frame transforms into a shared scene module. Make the existing exporter consume these shared functions before relocating the writer itself.
- [x] Add an owned `VoxelGrid` and move MagicaVoxel model ingestion, coordinate conversion, padding, and linear storage construction into `vox_bake`. Keep the public Bevy `VoxelData` API as an adapter over the shared ingestion result during migration.
- [x] Add `bake_model(VoxelGrid, BakedPalette, VoxBakeSettings)` orchestration returning optional surface and volume content, average IOR, and model dimensions with structured atlas-size errors. Make the existing GLB exporter consume this shared result as its model-baking path.
- [x] Add `BakedSceneNode` and frame-evaluated scene snapshots in `vox_bake`. Shared traversal owns hidden-node filtering, group flattening, hold-last transform/model frame selection, numeric model references, names, and local transforms; make the current GLB writer consume this snapshot.
- [x] Remove the GLB writer's remaining Bevy data dependencies. It now consumes `VoxBakeSettings`, `BakedPalette`, `BakedMeshAtlas`, baked model dimensions, and `BakedSceneNode`; shared material helpers perform linear/sRGB and emissive texture encoding.
- [x] Move `.vox` parsing, bake-relevant settings and defaults, voxel classification, meshing, texture packing, AO, material analysis, cloud-density extraction, shared scene traversal, and animation-frame evaluation into `vox_bake`.
- [x] Define Bevy-independent data using owned arrays, pixel buffers, explicit formats, numeric resource indices, and a small engine-neutral math dependency. Do not expose Bevy `Mesh`, `Image`, `StandardMaterial`, handles, entities, or asset contexts.
- [x] Complete separation of `.vox` parsing and baking from `AssetLoader`, `LoadContext`, and Bevy asset handles. Source parsing followed by `bake_scene` completes before either output adapter creates its native resources.
- [x] Introduce an owned `BakedScene` containing the analyzed palette, model grids, surface meshes, material images and parameters, optional cloud-density volumes, layers, nodes, transform tracks, shape-model tracks, hierarchy, names, and shared numeric references.
- [x] Represent each baked model as optional surface and volume content so mixed models retain both parts. Do not reduce the shared representation to glTF's supported feature set.
- [x] Store shape-model and transform tracks independently of playback implementation. The GLB adapter emits shape-model tracks as sibling nodes with initial visibility metadata; the Bevy adapter builds runtime flipbook entities from the same owned scene tracks. GLB transform tracks currently retain their first transform with a structured warning.
- [x] Give each current output adapter an explicit unsupported-content policy. The Bevy adapter supports surface and cloud content; the GLB adapter fails on cloud volumes rather than silently omitting them.
- [x] Preserve the existing coordinate conversion from MagicaVoxel's left-handed Z-up space to Bevy/glTF-compatible right-handed Y-up space in one well-tested place. `VoxelGrid::from_vox_model` owns the axis swap, X reflection, and padding offset, with a direct parsed-model boundary test; shared scene tests cover hold-last frame transforms.
- [x] Make both the Bevy runtime loader and GLB exporter consume `BakedScene`. The Bevy adapter preserves runtime flipbook entities and labeled sub-scenes while the GLB adapter converts each shape track into sibling frame nodes.
- [x] Split bake settings from Bevy runtime settings. Keep `supports_remeshing` and other runtime-only choices in `bevy_vox_scene`; source every shared default used by the CLI and runtime loader from `vox_bake`.
- [x] Preserve the existing `bevy_vox_scene` settings API where practical through re-exports or conversion into `vox_bake` settings, and document any unavoidable migration.
- [x] Keep Bevy-specific mesh, image, material, `FogVolume`, animation-entity, and asset registration work in the `bevy_vox_scene` adapter and out of `vox_bake`.
- [x] Keep GLB buffer construction, PNG encoding, glTF material mapping, animation sibling construction, and unsupported-feature errors in the feature-gated export adapter.
- [x] Move `vox-to-gltf` from its temporary dependency on `bevy_vox_scene` to `vox_bake`, then verify `cargo tree -p vox-to-gltf` contains no Bevy crates.
- [x] Choose an optional `gltf` feature in `vox_bake` for the glTF adapter and add its dependency boundary; retain a reusable library API and keep its dependencies out of default builds.
- [x] Relocate the GLB writer and its tests from `bevy_vox_scene` into the gated `vox_bake` module, then expose `vox_bake::export_vox_to_glb`.
- [x] Preserve `bevy_vox_scene::export_vox_to_glb` as a compatibility wrapper accepting `VoxLoaderSettings`.
- [x] Remove direct `dot_vox`, `ndshape`, `block-mesh`, and `ndcopy` dependencies from `bevy_vox_scene`. Source parsing, grid indexing, mesher visibility, and quad geometry now cross the `vox_bake` API boundary.
- [x] Make initial runtime loading, generated voxel scenes and animations, and runtime remeshing consume `vox_bake::bake_model`. Retain `BakedPalette` in the Bevy palette adapter and reduce the Bevy mesh module to `BakedMesh` conversion only.
- [x] Verify existing runtime asset labels, named sub-scenes, hierarchy, cloud loading, generated voxel APIs, runtime modification, and flipbook playback through the Bevy adapter test suite after migration to `BakedScene`.

#### GLB writer

- [x] Add an `export` Cargo feature with optional `gltf` and image-encoding dependencies.
- [x] Build one aligned binary buffer containing vertex attributes, indices, and embedded image payloads.
- [x] Generate valid buffer views and typed accessors, including position bounds required by glTF.
- [x] Convert shared `BakedMesh` vertex and index arrays to glTF triangle primitives.
- [x] Encode generated atlas images as PNG and attach the correct MIME type, texture sampler, and material slot.
- [x] Map material constants and textures to glTF metallic-roughness PBR properties with correct channels and color spaces.
- [x] Implement and declare supported material extensions for emissive strength, transmission, IOR, and volume.
- [x] Export scene nodes, transforms, hierarchy, names, and shared mesh references without duplicating instanced geometry.
- [x] Pad JSON and binary chunks according to the GLB alignment requirements and write them with `gltf::binary::Glb`.
- [x] Return structured errors for invalid VOX input, bake failures, oversized buffers, image encoding failures, and unsupported scene features.

#### CLI workspace package

- [x] Add a separate `vox-to-gltf` package to the Cargo workspace.
- [x] Keep argument parsing, filesystem traversal, terminal reporting, and process exit behavior in the CLI package rather than the library.
- [x] Support a single input and explicit output path first.
- [x] Add `--force` protection against accidental overwrite.
- [x] Write animated input to the requested single GLB path; model-frame suffixes apply to sibling node names rather than filenames.
- [x] Expose the stable bake settings needed for reproducible output. Consider a serialized settings file if future options make command-line flags unwieldy.
- [x] Print a concise per-scene conversion summary including source, model, mesh, triangle, texture, output-size, and destination counts plus structured lossy-conversion warnings.
- [x] Add recursive directory conversion with source-relative output paths and batch-wide overwrite protection.
- [x] Verify publishable metadata, README accuracy, license inclusion, minimal archive contents, generated library documentation, and packaged-source compilation for `vox_bake`. Confirm the CLI archive contains only its manifest, lockfile, README, license, and source, and document the required `vox_bake`-then-`vox-to-gltf` publish order. Full CLI archive verification necessarily follows registry publication of `vox_bake 0.1.0`.

#### Validation

- [x] Export an existing model fixture and parse every generated GLB through the independent `gltf` reader.
- [x] Export a multi-model scene and verify hierarchy, names, and shared mesh references.
- [x] Add direct assertions for exported transform matrices and hidden-node/layer filtering by comparing every named exported node with the shared frame-evaluated scene, including duplicate names and instanced transforms.
- [x] Export animation tracks into one valid GLB and verify each shape's sibling count, one-based zero-padded node names, common parent transform, first-frame visibility, and hidden later frames.
- [ ] Verify an animated export can be loaded as an ordered `Handle<Gltf>` collection and played without `bevy_vox_scene`.
- [x] Add exporter coverage for packed and palette texture modes, embedded PNG decoding, AO UV1, emission, transmission, IOR, volume, hierarchy, instancing, cloud rejection, and sibling-node animation frames.
- [x] Add field-level pixel assertions for base color and alpha, metalness/roughness, emission, transmission, and AO textures. GLB PNG pixels are compared exactly with the shared bake, and the Bevy material adapter is checked against the same palette and AO channel data.
- [x] Compare exported mesh attributes and decoded base-color and metalness/roughness texture pixels with the shared baked output, and verify that the Bevy mesh adapter preserves every shared vertex attribute and index buffer exactly.
- [x] Validate representative packed, palette, AO, animated, hierarchical, emissive, and transmissive exports with the pinned Khronos glTF Validator in CI, treating specification errors as failures.
- [x] Add initial round-trip inspection using the `gltf` reader, including meshes and the embedded binary buffer. Extend this with field-level assertions as more fixtures are added.
- [ ] Render representative runtime-loaded and GLB-loaded scenes in Bevy and compare them visually, allowing only documented differences.
- [ ] Verify conversion on native platforms and loading on native, WebGPU, WebGL2, and WebAssembly targets supported by the project.
- [ ] Measure conversion time, output size, runtime load time, and final dependency/build-size reduction on representative scenes.
- [ ] Audit unsupported source data. Define policies for remaining MagicaVoxel material properties and metadata, and emit typed warnings for every supported but lossy conversion.

#### Bevy asset processor follow-up

- [ ] Implement a Bevy `Process` or `LoadTransformAndSave` adapter that invokes the shared GLB exporter.
- [ ] Map per-asset `.meta` settings to the same bake and export settings used by the CLI.
- [ ] Ensure changes to source `.vox` files or settings invalidate and rebuild the processed GLB.
- [ ] Confirm a release application can consume only processed GLB assets without registering `VoxScenePlugin`.
- [x] Document local CLI installation, recursive conversion, deterministic CI checks, `build.rs` integration through `vox_bake`, packaging implications, and runtime GLB consumption. Asset-processor-specific workflow documentation remains part of its implementation task.

### Dependencies

- `vox_bake` should retain the existing engine-neutral parsing and baking dependencies such as `dot_vox`, `block-mesh`, `ndshape`, and `rectangle-pack`, plus only the small math or serialization dependencies required by its public data model. It must not depend on Bevy.
- [`gltf`](https://crates.io/crates/gltf), optional to the glTF adapter, for typed glTF JSON structures, validation helpers, and the GLB container writer.
- A PNG encoder for embedding the generated 2D texture atlases. Prefer reusing an existing transitive/image dependency only if it offers a stable direct API; otherwise add the smallest suitable optional dependency explicitly.
- [`clap`](https://crates.io/crates/clap) in `vox-to-gltf` for generated help, argument validation, and a maintainable interface as more bake settings are exposed.

The shipped game does not need the CLI, glTF, or PNG-writing dependencies when it consumes the exported GLB. Runtime-only `bevy_vox_scene` users depend on `vox_bake` for shared processing but do not compile the glTF adapter. The `vox-to-gltf` dependency tree contains `vox_bake` and export dependencies but no Bevy crates.
