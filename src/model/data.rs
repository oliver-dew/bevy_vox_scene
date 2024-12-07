use bevy::{
    image::Image,
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

    pub(crate) fn remesh(
        &self,
        ior_for_voxel: &[Option<f32>],
        density_for_voxel: &[Option<f32>],
    ) -> (Option<Mesh>, Option<f32>, Option<Image>) {
        let (visible_voxels, average_ior, needs_meshing) =
            self.visible_voxels(ior_for_voxel, density_for_voxel);
        let (cloud_voxels, has_cloud) = self.cloud_voxels(density_for_voxel);
        let maybe_mesh = if needs_meshing {
            Some(super::mesh::mesh_model(&visible_voxels, self))
        } else {
            None
        };
        let maybe_image = if has_cloud {
            Some(super::cloud::create_cloud_image(&cloud_voxels, self))
        } else {
            None
        };
        (maybe_mesh, average_ior, maybe_image)
    }

    /// Returns the [`VoxelVisibility`] of each Voxel, and, if the model contains
    /// translucent voxels, the average Index of Refraction.
    pub(crate) fn visible_voxels(
        &self,
        ior_for_voxel: &[Option<f32>],
        density_for_voxel: &[Option<f32>],
    ) -> (Vec<VisibleVoxel>, Option<f32>, bool) {
        // TODO: return a "has solid/ translucent voxels" bool to decide whether to mesh
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
                } else if density_for_voxel[v.0 as usize].is_some() {
                    VoxelVisibility::Empty
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
        let needs_meshing = voxels
            .iter()
            .any(|&v| v.visibility != VoxelVisibility::Empty);
        (voxels, average_ior, needs_meshing)
    }

    pub(crate) fn cloud_voxels(&self, density_for_voxel: &[Option<f32>]) -> (Vec<f32>, bool) {
        let mut has_cloud: bool = false;
        let max_bound = self.shape.as_array().map(|v| v - 1);
        let densities: Vec<f32> = self
            .voxels
            .iter()
            .enumerate()
            .filter_map(|(index, v)| {
                // remove the outer layer of voxels that the loader adds
                let coords = self.shape.delinearize(index as u32);
                if coords.contains(&0) {
                    return None;
                }
                if coords[0] == max_bound[0]
                    || coords[1] == max_bound[1]
                    || coords[2] == max_bound[2]
                {
                    return None;
                }
                if let Some(density) = density_for_voxel[v.0 as usize] {
                    has_cloud = true;
                    Some(density * 10.)
                } else {
                    Some(0.0)
                }
            })
            .collect();
        (densities, has_cloud)
    }
}
