#![forbid(missing_docs, unsafe_code)]
//! Engine-independent types and processing for baking MagicaVoxel assets.

mod ambient_occlusion;
mod atlas;
mod bake;
#[cfg(feature = "gltf")]
mod gltf_export;
mod grid;
mod material;
mod mesh;
mod output;
mod paint;
mod scene;
mod settings;
pub mod source;
mod voxel;

pub use atlas::{
    AtlasRect, PackedAtlas, ambient_occlusion_quad_uvs, pack_atlas_rectangles, packed_quad_uvs,
    palette_quad_uvs,
};
pub use bake::{BakeError, BakedModel, bake_model};
#[cfg(feature = "gltf")]
pub use gltf_export::{
    ExportError, ExportStatistics, ExportWarning, ExportedGlb, export_vox_to_glb,
};
pub use grid::{VoxelGrid, linearize};
pub use material::{BakedMaterial, BakedPalette, analyze_vox_palette};
pub use mesh::{BakedQuad, BakedQuadGeometry, bake_quad_geometry, greedy_mesh_quads};
pub use output::{BakedImage, BakedMesh, BakedMeshAtlas, BakedPixelFormat, BakedVolume};
pub use paint::paint_palette_island;
pub use scene::{
    BakedLayer, BakedScene, BakedSceneGraphNode, BakedSceneModel, BakedSceneNode, SceneBakeError,
    bake_scene, baked_scene_frame_count, evaluate_baked_scene_frame, frame_transform, model_names,
};
pub use settings::{
    AmbientOcclusionSettings, TextureMipPolicy, TexturePackSettings, UnitOffset, VoxBakeSettings,
    VoxelTextureMode,
};
pub use voxel::{
    ClassifiedVoxels, RawVoxel, VisibleVoxel, VoxelVisibility, classify_voxels,
    extract_cloud_volume,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_engine_independent_and_stable() {
        let pack = TexturePackSettings::default();
        assert_eq!(VoxelTextureMode::default(), VoxelTextureMode::Packed(pack));
        assert_eq!(pack.padding, 1);
        assert_eq!(pack.alignment, 4);
        assert_eq!(pack.max_dimension, 8192);
        assert_eq!(UnitOffset::CENTER.to_array(), [0.5; 3]);
        assert_eq!(TextureMipPolicy::Limited { max_lod: 9 }.max_lod(), 4);
        assert_eq!(VoxBakeSettings::default().emission_strength, 20.0);
    }

    #[test]
    fn classification_handles_surface_and_volume_materials() {
        let voxels = [RawVoxel::EMPTY, RawVoxel(1), RawVoxel(2), RawVoxel(3)];
        let mut ior = vec![None; 256];
        ior[2] = Some(1.5);
        let mut density = vec![None; 256];
        density[3] = Some(0.75);
        let packed = classify_voxels(&voxels, VoxelTextureMode::default(), &ior, &density);
        assert_eq!(
            packed
                .voxels
                .iter()
                .map(|voxel| voxel.visibility)
                .collect::<Vec<_>>(),
            vec![
                VoxelVisibility::Empty,
                VoxelVisibility::Opaque,
                VoxelVisibility::Translucent,
                VoxelVisibility::Empty
            ]
        );
        assert!(packed.has_surface);
        assert_eq!(packed.average_ior, Some(1.5));
        assert!(packed.voxels.iter().all(|voxel| voxel.merge_index == 0));
        let palette = classify_voxels(&voxels, VoxelTextureMode::Palette, &ior, &density);
        assert_eq!(
            palette
                .voxels
                .iter()
                .map(|voxel| voxel.merge_index)
                .collect::<Vec<_>>(),
            vec![255, 1, 2, 3]
        );
    }

    #[test]
    fn cloud_extraction_removes_padding_and_preserves_linear_order() {
        let dimensions = [4, 3, 3];
        let mut voxels = vec![RawVoxel::EMPTY; 36];
        let index = |x, y, z| x + dimensions[0] * (y + dimensions[1] * z);
        voxels[index(1, 1, 1) as usize] = RawVoxel(3);
        voxels[index(2, 1, 1) as usize] = RawVoxel(4);
        let mut density = vec![None; 256];
        density[3] = Some(0.25);
        density[4] = Some(0.75);
        let volume = extract_cloud_volume(&voxels, dimensions, &density).unwrap();
        assert_eq!(volume.dimensions, [2, 1, 1]);
        assert_eq!(volume.densities, vec![0.25, 0.75]);
        assert!(extract_cloud_volume(&voxels, dimensions, &vec![None; 256]).is_none());
    }

    #[test]
    fn greedy_meshing_merges_adjacent_voxels() {
        let opaque = VisibleVoxel {
            index: 1,
            visibility: VoxelVisibility::Opaque,
            merge_index: 0,
        };
        let dimensions = [4, 3, 3];
        let mut voxels = vec![
            VisibleVoxel {
                index: RawVoxel::EMPTY.0,
                visibility: VoxelVisibility::Empty,
                merge_index: 0
            };
            36
        ];
        let index = |x, y, z| x + dimensions[0] * (y + dimensions[1] * z);
        voxels[index(1, 1, 1) as usize] = opaque;
        voxels[index(2, 1, 1) as usize] = opaque;
        let quads = greedy_mesh_quads(&voxels, dimensions);
        assert_eq!(quads.len(), 6);
        assert!(quads.iter().any(|quad| quad.width == 2 || quad.height == 2));
    }

    #[test]
    fn atlas_packing_is_deterministic_and_honors_alignment() {
        let sizes = [(10, 2), (3, 7), (5, 5)];
        let first = pack_atlas_rectangles(&sizes, 1, 4, 8192, 4).unwrap();
        let second = pack_atlas_rectangles(&sizes, 1, 4, 8192, 4).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.width % 4, 0);
        assert_eq!(first.height % 4, 0);
        assert!(first.rectangles.iter().all(|rect| rect.x % 4 == 0));
        assert!(first.rectangles.iter().all(|rect| rect.y % 4 == 0));
        assert!(pack_atlas_rectangles(&sizes, 1, 4, 8, 4).is_none());
    }
}
pub use ambient_occlusion::{
    AmbientOcclusionSample, ambient_occlusion_at, ambient_occlusion_stencil, face_normal,
    paint_ambient_occlusion_island, sanitized_ambient_occlusion_intensity,
};
