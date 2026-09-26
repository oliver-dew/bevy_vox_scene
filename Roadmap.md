# Roadmap

## Per-mesh texture packing

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
- it should contain two values, `depth: usize, intensity: f32`

For each exposed face, calculate the voxel residency within an inverted cone (with the tip sliced off) positioned just above the face, pointing up according to the face normal. eg depth 1 will only check voxel residency a max of 1 diagonal step away

normalize the residency, multiply by intensity, write to an AO texture, which, unlike the others, will be sampled with linear interpolation 

### Changes needed

#### Design and representation

- [x] Add configurable atlas alignment and maximum texture dimensions, with tightly sized (not necessarily power-of-two) textures as the default.
- [x] Add loader/generation settings for enabling per-mesh atlases and configuring island padding.
- [x] Define an intermediate `MeshedFace`/island representation containing the face orientation, voxel-space origin, width, height, and packed atlas rectangle.
- [x] Define a mesh-build result that returns the Bevy `Mesh` together with the images and material values needed by that mesh.
- [x] Support a per-model 16x16 palette atlas as the compatibility/low-memory mode; do not retain palette textures or materials in the shared `VoxelContext`.

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
- [ ] Verify mipmap behavior at a distance.
- [ ] Add pixel-level tests for every baked channel, including face orientation and padding.

#### Asset and runtime integration

- [x] Refactor palette material creation into one per-model material pipeline that can bake either a packed face atlas or a 16x16 palette atlas.
- [x] Update `.vox` loading to register mesh-local images and a mesh-local `StandardMaterial` as labeled sub-assets.
- [x] Preserve the current opaque/transmissive material behavior, average IOR, thickness, and non-emissive material variants.
- [x] Update generated voxel scenes so every model receives its own material; keep only palette data and other genuinely shared state in `VoxelContext`.
- [x] Update runtime remeshing so voxel edits rebuild the atlas, image assets, material constants, and UVs together without leaking stale assets.
- [x] Verify animated models use the correct atlas and material for every frame.
- [x] Check whether public APIs or labeled asset names change, and document the model-qualified material labels used by the example.

#### Validation and rollout

- [x] Add an example or fixture containing a large, highly multicoloured plane to demonstrate the geometry reduction.
    - the existing scene already has this (eg the floor tiles)
- [ ] Add regression coverage for single-colour, emissive, metallic/rough, glass, cloud-only, animated, and runtime-edited models.
- [ ] Compare rendered output against the current palette-atlas path for all supported MagicaVoxel material properties.
- [ ] Benchmark mesh generation time, atlas-baking time, vertex count, draw count, and texture memory.
- [ ] Test native, WebGPU, and WebGL2 builds, including their existing texture-format differences.
- [x] Document the memory/performance trade-off and the new configuration in the README and changelog.

### Dependencies

Use [rectangle-pack](https://github.com/chinedufn/rectangle-pack) for deterministic, static island packing. Atlas sizing remains the responsibility of a small wrapper that tries a bounded set of candidate dimensions and selects the most compact successful result.
