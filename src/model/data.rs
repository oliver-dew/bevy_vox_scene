use bevy::{
    image::Image,
    math::{IVec3, UVec3},
};
use std::fmt::Debug;
use vox_bake::{BakedModel, BakedPalette, VoxelGrid, bake_model};

use crate::VoxLoaderSettings;

use super::{RawVoxel, mesh::MeshAtlas};

/// The voxel data used to create a mesh and a material.
#[derive(Clone)]
pub struct VoxelData {
    pub(crate) dimensions: [u32; 3],
    pub(crate) voxels: Vec<RawVoxel>,
    pub(crate) settings: VoxLoaderSettings,
}

impl Default for VoxelData {
    fn default() -> Self {
        Self {
            dimensions: [0, 0, 0],
            voxels: Default::default(),
            settings: VoxLoaderSettings::default(),
        }
    }
}

impl Debug for VoxelData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VoxelData")
            .field("dimensions", &self.dimensions)
            .field("voxels", &self.voxels.len())
            .field("settings", &self.settings)
            .finish()
    }
}

impl VoxelData {
    /// Returns a new, empty VoxelData model
    pub fn new(size: UVec3, settings: VoxLoaderSettings) -> Self {
        let padding = if settings.mesh_outer_faces {
            UVec3::splat(2)
        } else {
            UVec3::ZERO
        };
        let dimensions: [u32; 3] = (size + padding).into();
        let size = dimensions.iter().product::<u32>() as usize;
        Self {
            dimensions,
            voxels: vec![RawVoxel::EMPTY; size],
            settings,
        }
    }
    /// The size of the voxel model, not including the padding that may have been added if the outer faces are being meshed.
    pub(crate) fn _size(&self) -> IVec3 {
        let raw_size: UVec3 = self.dimensions.into();
        let padded = raw_size - UVec3::splat(self.padding());
        IVec3::try_from(padded).unwrap_or(IVec3::ZERO)
    }

    /// If the outer faces are to be meshed, the mesher requires 1 voxel of padding around the edge of the model
    pub(crate) fn padding(&self) -> u32 {
        if self.settings.mesh_outer_faces { 2 } else { 0 }
    }

    pub(crate) fn bake(&self, palette: &BakedPalette) -> BakedModel {
        bake_model(&self.voxel_grid(), palette, &(&self.settings).into())
            .expect("voxel texture atlas exceeds the configured maximum size")
    }

    pub(crate) fn remesh(
        &self,
        palette: &BakedPalette,
    ) -> (Option<MeshAtlas>, Option<f32>, Option<Image>) {
        let baked = self.bake(palette);
        let image = baked.volume.as_ref().map(super::cloud::create_cloud_image);
        (baked.surface, baked.average_ior, image)
    }

    fn voxel_grid(&self) -> VoxelGrid {
        VoxelGrid {
            dimensions: self.dimensions,
            model_dimensions: self
                .dimensions
                .map(|dimension| dimension.saturating_sub(self.padding())),
            voxels: self.voxels.clone(),
            padding: self.padding(),
        }
    }
}
