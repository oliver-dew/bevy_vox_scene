use bevy::{
    math::{IVec3, UVec3},
    render::mesh::Mesh,
};
use block_mesh::VoxelVisibility;
use ndshape::{RuntimeShape, Shape};
use std::fmt::Debug;

use crate::VoxLoaderSettings;

use super::{voxel::VisibleVoxel, RawVoxel};

/// The voxel data used to create a mesh and a material.
#[derive(Clone)]
pub struct VoxelData {
    pub(crate) shape: RuntimeShape<u32, 3>,
    pub(crate) voxels: Vec<RawVoxel>,
    pub(crate) settings: VoxLoaderSettings,
}

impl Default for VoxelData {
    fn default() -> Self {
        Self {
            shape: RuntimeShape::<u32, 3>::new([0, 0, 0]),
            voxels: Default::default(),
            settings: VoxLoaderSettings::default(),
        }
    }
}

impl Debug for VoxelData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VoxelData")
            .field("shape", &self.shape.as_array())
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
        let shape = RuntimeShape::<u32, 3>::new((size + padding).into());
        let size = shape.size() as usize;
        Self {
            shape,
            voxels: vec![RawVoxel::EMPTY; size],
            settings,
        }
    }
    /// The size of the voxel model, not including the padding that may have been added if the outer faces are being meshed.
    pub(crate) fn _size(&self) -> IVec3 {
        let raw_size: UVec3 = self.shape.as_array().into();
        let padded = raw_size - UVec3::splat(self.padding());
        IVec3::try_from(padded).unwrap_or(IVec3::ZERO)
    }

    /// If the outer faces are to be meshed, the mesher requires 1 voxel of padding around the edge of the model
    pub(crate) fn padding(&self) -> u32 {
        if self.settings.mesh_outer_faces {
            2
        } else {
            0
        }
    }

    pub(crate) fn remesh(&self, ior_for_voxel: &[Option<f32>]) -> (Mesh, Option<f32>) {
        let (visible_voxels, average_ior) = self.visible_voxels(ior_for_voxel);
        (super::mesh::mesh_model(&visible_voxels, self), average_ior)
    }

    /// Returns the [`VoxelVisibility`] of each Voxel, and, if the model contains
    /// translucent voxels, the average Index of Refraction.
    pub(crate) fn visible_voxels(
        &self,
        ior_for_voxel: &[Option<f32>],
    ) -> (Vec<VisibleVoxel>, Option<f32>) {
        let mut refraction_indices: Vec<f32> = Vec::new();
        let voxels: Vec<VisibleVoxel> = self
            .voxels
            .iter()
            .map(|v| VisibleVoxel {
                index: v.0,
                visibility: if *v == RawVoxel::EMPTY {
                    VoxelVisibility::Empty
                } else if let Some(ior) = ior_for_voxel[v.0 as usize] {
                    refraction_indices.push(ior);
                    VoxelVisibility::Translucent
                } else {
                    VoxelVisibility::Opaque
                },
            })
            .collect();
        let average_ior: Option<f32> = if refraction_indices.is_empty() {
            None
        } else {
            let ior = refraction_indices
                .iter()
                .cloned()
                .reduce(|acc, e| acc + e)
                .unwrap_or(0.0)
                / refraction_indices.len() as f32;
            Some(ior)
        };
        (voxels, average_ior)
    }
}
