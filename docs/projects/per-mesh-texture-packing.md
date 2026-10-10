# Per-mesh texture packing

### Context

A key feature of bevy_vox_scene is that it supports the full PBR material set that MagicaVoxel vends. To do this it creates 16x16 textures for albedo, metallic, roughness, emission etc, and uses texture coordinates to pick a single pixel. Surfaces that are the same voxel colour are combined into a single face. A disadvantage of this is that if you have a flat plane with lots of different colours, you create a lot of extra geometry.

### Proposal

Each mesh has a unique set of textures. Each face of the mesh corresponds to a unique "island" on that texture. 
1. Meshes will only have the textures they need. If they consist of a single color, or unvarying metalness, roughness etc, then those properties don't need a texture, and can just be represented by a constant color or f32.
2. There will still need to be vertex splits at UV seams/hard normals
3. There should be a 1-1 relationship between texture pixel and voxel so that the textures can be sampled with "nearest" (pixelart) sampling and the pixels will line up perfectly with the voxels.
4. Face islands will be packed into the texture in an efficient way, with configurable padding around each island to prevent bleed (although the built in MagicaVoxel parameters will use "nearest" sampling, so shouldn't need a padding, if we add computed ambient occlusion, that would use linear sampling, so would need padding)

#### Ambient Occlusion
Bakes ambient occlusion based on voxel residency

- Ambient occlusion should be configurable with an `Option<AmbientOcclusionSettings>` member on VoxLoaderSettings
- it should contain two values, `depth: usize, intensity: f32`. Valid depths are 1..=8 (depth of 0 produces no effect, over 8 will likely degrade performance).

For each exposed face, calculate the voxel residency within an inverted cone (with the tip sliced off) positioned just above the face, pointing up according to the face normal. eg depth 1 will only check voxel residency a max of 1 diagonal step away. Depth of 8 will create a cone 8 voxels high, with a radius of 8.

We could use the sdf module (if we added a cone primitive), to generate this shape.

normalize the residency, multiply by intensity, write to an AO texture, which, unlike the others, will be sampled with linear interpolation 

#### Ambient Occlusion implementation plan

##### API and behavior

- [x] Add a public, serializable `AmbientOcclusionSettings { depth: usize, intensity: f32 }` with documented defaults and export it from the crate root.
- [x] Add `ambient_occlusion: Option<AmbientOcclusionSettings>` to `VoxLoaderSettings`, defaulting to `None` so existing scenes retain their current appearance and build cost.
- [x] Treat depth `0` as disabled, clamp depth to a maximum of `8`, and treat negative or non-finite intensity as zero.
- [x] Add explicit tests for zero depth, oversized depth, negative intensity, and non-finite intensity.
- [x] Define AO output as visibility: `1.0` is fully exposed and `0.0` is fully occluded. Compute it as `1.0 - clamp(normalized_residency * intensity, 0.0, 1.0)`.
- [x] Restrict baked AO to `VoxelTextureMode::Packed`, because the palette mode reuses one UV texel for every face of a color and therefore cannot represent position-dependent AO. Palette-plus-AO configurations disable AO.
- [x] Enforce an effective packed-island padding of at least one texel when AO is enabled, even when `TexturePackSettings::padding == 0`.

##### Sampling kernel

- [x] Use a circular truncated cone in integer voxel coordinates. Layer `1` is immediately above the face, each layer's radius equals its distance along the normal, and samples on the circular boundary are included.
- [x] Precompute one oriented integer offset stencil per face for the configured depth, without depending on the feature-gated SDF module.
- [x] Weight each sample by inverse Euclidean distance so nearby occluders contribute more strongly. Keep angular weighting as a possible later refinement if the result is too harsh or produces visible bands.
- [x] Normalize residency by the sum of all possible distance weights rather than the raw sample count, so increasing depth does not automatically darken the result. Treat positions outside the model as empty so AO does not darken model boundaries.
- [x] Treat opaque and transmissive voxels as occluders; exclude empty and cloud/media cells through their existing visibility classification.
- [x] Add dedicated occupancy tests for opaque, transmissive, cloud/media, and out-of-bounds samples.
- [x] Rotate the canonical stencil into the local normal, U, and V axes of each of the six face orientations.
- [x] Build each face-orientation stencil once per mesh so baking performs voxel lookups rather than rebuilding cone geometry for every texel.

##### Atlas baking

- [x] Extend `MeshAtlas` with optional AO texel data, keeping geometric AO separate from the palette-index data consumed by `VoxelPalette`.
- [x] Pack AO separately as `(width + 1) × (height + 1)` islands and calculate it on the voxel-face corner lattice using the oriented face basis.
- [x] Write AO as linear `R8Unorm`, where `255` is fully exposed and `0` is fully occluded.
- [ ] Verify that linear-filtered `R8Unorm` works on native, WebGPU, and WebGL2 targets.
- [x] Extend each island's AO edge values into its padding so bilinear samples cannot blend with unrelated islands or unused atlas space.
- [x] Give the AO image a linear minification/magnification sampler while retaining nearest-neighbour sampling for voxel-authored PBR maps.
- [x] Omit the AO image and material binding when AO is disabled.
- [ ] If every baked AO value is fully exposed, consider omitting the texture as a later optimization.

##### Material and asset integration

- [x] Register each AO image as a model-qualified labeled sub-asset, assign it to `StandardMaterial::occlusion_texture`, and select the independently packed UV1 coordinates through `occlusion_channel`.
- [x] Include the AO image handle when cleaning up and replacing material images during runtime voxel remeshing.
- [x] Ensure generated scenes, loaded `.vox` scenes, and every animation frame receive their own AO texture.
- [x] Keep cloud-density texture generation and palette material properties independent from AO baking.

##### Correctness and performance

- [ ] Add unit tests for isolated faces, fully open space, a single nearby occluder, opposing walls, model boundaries, and all six face orientations.
- [x] Add unit coverage showing concave geometry produces occlusion and palette mode omits spatial AO.
- [x] Add material-level tests for the AO image format, pixel bytes, material binding, and linear sampler configuration.
- [ ] Add pixel-level tests for AO normalization, intensity scaling, and padding replication.
- [x] Add a unit test confirming nearby stencil samples carry more weight than distant samples.
- [ ] Add a symmetry test: rotating the same voxel arrangement and face normal should produce the same AO value.
- [ ] Add runtime-remeshing coverage showing that adding or removing a nearby voxel updates affected AO texels and retires the previous image asset.
- [ ] Add a visual example that can toggle baked AO on and off, and compare it with Bevy SSAO to catch banding or over-darkening.
- [ ] Benchmark depths `1`, `2`, `4`, and `8` on representative models. Record sample count, bake time, atlas memory, and runtime-remeshing cost.
- [ ] If depth `8` is too expensive, investigate a summed-volume occupancy structure or separable approximation before parallelizing the baker.
- [ ] Test native, WebGPU, WebGL2, and WebAssembly builds with AO enabled.

### Changes needed

#### Design and representation

- [x] Add configurable atlas alignment and maximum texture dimensions, with tightly sized (not necessarily power-of-two) textures as the default.
- [x] Add loader/generation settings for enabling per-mesh atlases and configuring island padding.
- [x] Define an intermediate `MeshedFace`/island representation containing the face orientation, voxel-space origin, width, height, and packed atlas rectangle.
- [x] Define a mesh-build result that returns the Bevy `Mesh` together with the images and material values needed by that mesh.
- [x] Support a per-model 16x16 palette atlas as the compatibility/low-memory mode; do not retain palette textures or materials in the shared `VoxelPalette` asset.

#### Geometry and islands

- [x] Change `VisibleVoxel::MergeValue` so greedy meshing merges coplanar voxels with the same visibility class regardless of palette index.
- [x] Verify that opaque and translucent voxels are not incorrectly merged across visibility/material boundaries.
- [x] Preserve enough face orientation and voxel-coordinate information to map every texel in a merged quad back to its source voxel.
- [x] Add a unit test showing that a multicoloured flat plane becomes one quad.
- [ ] Add tests showing that exposed faces, texture orientation, and winding remain correct in all six directions.
- [ ] Record before/after quad, vertex, and index counts for representative models.

#### Atlas packing and UVs

- [x] Add `rectangle-pack` as the static, deterministic rectangle-packing dependency.
- [ ] Verify `rectangle-pack` and the packed-atlas path in a WebAssembly build.
- [x] Treat each merged quad as a rectangular island sized to one texel per voxel, plus configured padding.
- [x] Calculate the lower bounds for packing from the total island area and the widest/tallest islands.
- [x] Generate a small deterministic set of aligned candidate widths around the square root of the total area, bounded by the widest island and maximum texture dimension.
- [x] Use `rectangle-pack` to try each candidate with sufficient height, then crop successful layouts to their used bounds.
- [x] Select the successful layout with the smallest texture area, using aspect ratio as a tie-breaker to avoid unnecessarily long, thin atlases.
- [x] Pack all islands for one mesh and fail if they cannot fit within the configured maximum texture size.
- [ ] Replace the current packing panic with a structured, useful error.
- [x] Convert each island's packed texel rectangle into normalized UV coordinates using texel-edge coordinates, with orientation chosen to match its source face.
- [ ] Confirm Bevy's image origin/UV convention and test for rotations or mirrored faces on all six face orientations.
- [ ] Add tests for non-square atlases, single-texel islands, padding, and deterministic packing.

#### Texture baking

- [x] Rasterize each island by looking up the palette index of its corresponding source voxel.
- [x] Bake base color with the correct sRGB or linear texture format.
- [x] Bake emissive color, preserving the current emission-strength behavior and HDR-capable format.
- [x] Bake metallic and roughness into the channels expected by Bevy's `StandardMaterial`.
- [x] Bake specular transmission when the `pbr_transmission_textures` feature is enabled.
- [x] Fill island padding by extending edge texels so linear filtering and mipmaps cannot sample unrelated islands.
- [x] Use constant `StandardMaterial` properties and omit a texture whenever a property is uniform across the mesh.
- [x] Set nearest-neighbour sampling for the voxel-derived maps.
- [x] Add an opt-in limited mip policy, mip-block-aligned island packing, CPU mip generation for every generated map, and linear blending between mip levels.
- [ ] Verify mipmap behavior at a distance.
- [ ] Add pixel-level tests for every baked channel, including face orientation and padding.

#### Asset and runtime integration

- [x] Refactor palette material creation into one per-model material pipeline that can bake either a packed face atlas or a 16x16 palette atlas.
- [x] Update `.vox` loading to register mesh-local images and a mesh-local `StandardMaterial` as labeled sub-assets.
- [x] Preserve the current opaque/transmissive material behavior, average IOR, and thickness.
- [x] Update generated voxel scenes so every model receives its own material and shares a `VoxelPalette` asset.
- [x] Update runtime remeshing so voxel edits rebuild the atlas, image assets, material constants, and UVs together without leaking stale assets.
- [x] Verify animated models use the correct atlas and material for every frame.
- [x] Check whether public APIs or labeled asset names change, and document the model-qualified material labels used by the example.

#### Validation and rollout

- [x] Add an example or fixture containing a large, highly multicoloured plane to demonstrate the geometry reduction.
    - the existing scene already has this (eg the floor tiles)
- [ ] Add regression coverage for single-colour, emissive, metallic/rough, glass, cloud-only, animated, and runtime-edited models.
- [ ] Compare rendered output against the current palette-atlas path for all supported MagicaVoxel material properties.
- [ ] Benchmark mesh generation time, atlas-baking time, vertex count, draw count, and texture memory.
- [x] Add opt-in atlas diagnostics for dimensions, occupancy, padding/alignment overhead, packing waste, island count, and vertex reduction.
- [ ] Test native, WebGPU, and WebGL2 builds, including their existing texture-format differences.
- [x] Document the memory/performance trade-off and the new configuration in the README and changelog.

### Dependencies

Use [rectangle-pack](https://github.com/chinedufn/rectangle-pack) for deterministic, static island packing. Atlas sizing remains the responsibility of a small wrapper that tries a bounded set of candidate dimensions and selects the most compact successful result.
