use bevy::{
    asset::RenderAssetUsages,
    image::Image,
    math::{IVec3, UVec3},
};
use std::fmt::Debug;
use vox_bake::{BakeError, BakedModel, BakedPalette, VoxBakeSettings, VoxelGrid, bake_model};

use crate::VoxLoaderSettings;

use super::mesh::MeshAtlas;

/// The voxel data used to create a mesh and a material.
#[derive(Clone)]
pub struct VoxelData {
    pub(crate) grid: VoxelGrid,
    pub(crate) bake_settings: VoxBakeSettings,
    pub(crate) retain_voxel_data: bool,
    pub(crate) mesh_asset_usage: RenderAssetUsages,
}

impl Default for VoxelData {
    fn default() -> Self {
        let settings = VoxLoaderSettings::default();
        let bake_settings = VoxBakeSettings::from(&settings);
        Self {
            grid: VoxelGrid {
                dimensions: [0; 3],
                model_dimensions: [0; 3],
                voxels: Vec::new(),
                padding: if settings.mesh_outer_faces { 2 } else { 0 },
            },
            bake_settings,
            retain_voxel_data: settings.retain_voxel_data,
            mesh_asset_usage: settings.mesh_asset_usage,
        }
    }
}

impl Debug for VoxelData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VoxelData")
            .field("dimensions", &self.grid.dimensions)
            .field("voxels", &self.grid.voxels.len())
            .field("bake_settings", &self.bake_settings)
            .field("retain_voxel_data", &self.retain_voxel_data)
            .field("mesh_asset_usage", &self.mesh_asset_usage)
            .finish()
    }
}

impl VoxelData {
    /// Returns a new, empty VoxelData model
    pub fn new(size: UVec3, settings: VoxLoaderSettings) -> Self {
        let grid = VoxelGrid::empty(size.into(), settings.mesh_outer_faces);
        let bake_settings = VoxBakeSettings::from(&settings);
        Self {
            grid,
            bake_settings,
            retain_voxel_data: settings.retain_voxel_data,
            mesh_asset_usage: settings.mesh_asset_usage,
        }
    }
    /// The size of the voxel model, not including the padding that may have been added if the outer faces are being meshed.
    pub(crate) fn _size(&self) -> IVec3 {
        let size: UVec3 = self.grid.model_dimensions.into();
        IVec3::try_from(size).unwrap_or(IVec3::ZERO)
    }

    /// If the outer faces are to be meshed, the mesher requires 1 voxel of padding around the edge of the model
    pub(crate) fn padding(&self) -> u32 {
        self.grid.padding
    }

    pub(crate) fn bake(&self, palette: &BakedPalette) -> Result<BakedModel, BakeError> {
        bake_model(&self.grid, palette, &self.bake_settings)
    }

    pub(crate) fn remesh(
        &self,
        palette: &BakedPalette,
    ) -> Result<(Option<MeshAtlas>, Option<f32>, Option<Image>), BakeError> {
        let baked = self.bake(palette)?;
        let image = baked.volume.as_ref().map(super::cloud::create_cloud_image);
        Ok((baked.surface, baked.average_ior, image))
    }
}
