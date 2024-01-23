use bevy::{
    asset::{Asset, Handle},
    pbr::StandardMaterial,
    reflect::TypePath,
    render::mesh::Mesh,
    utils::HashMap,
};
use block_mesh::VoxelVisibility;
use ndshape::RuntimeShape;

use self::voxel::VisibleVoxel;
pub use self::{queryable::VoxelQueryable, voxel::Voxel};
pub(crate) use voxel::RawVoxel;

pub(super) mod mesh;
pub(super) mod modify;
pub(super) mod queryable;
mod voxel;

/// Asset containing the voxel data for a model, as well as handles to the mesh derived from that data and the material
#[derive(Asset, TypePath, Default)]
pub struct VoxelModel {
    /// The voxel data used to generate the mesh
    pub(crate) data: VoxelData,
    /// Handle to the model's mesh
    pub(crate) mesh: Handle<Mesh>,
    /// Handle to the model's material
    pub(crate) material: Handle<StandardMaterial>,
}

/// The voxel data used to create a mesh and a material.
///
/// Note that all coordinates are in Bevy's right-handed Y-up space
pub(crate) struct VoxelData {
    pub shape: RuntimeShape<u32, 3>,
    pub voxels: Vec<RawVoxel>,
    pub ior_for_voxel: HashMap<u8, f32>,
    pub mesh_outer_faces: bool,
}

impl Default for VoxelData {
    fn default() -> Self {
        Self {
            shape: RuntimeShape::<u32, 3>::new([0, 0, 0]),
            voxels: Default::default(),
            ior_for_voxel: Default::default(),
            mesh_outer_faces: true,
        }
    }
}

impl VoxelData {
    /// If the outer faces are to be meshed, the mesher requires 1 voxel of padding around the edge of the model
    pub(crate) fn padding(&self) -> u32 {
        if self.mesh_outer_faces {
            2
        } else {
            0
        }
    }

    pub(crate) fn remesh(&self) -> Mesh {
        let (visible_voxels, _) = self.visible_voxels();
        mesh::mesh_model(&visible_voxels, self)
    }

    /// Returns the [`VoxelVisibility`] of each Voxel, and, if the model contains
    /// translucent voxels, the average Index of Refraction.
    pub(crate) fn visible_voxels(&self) -> (Vec<VisibleVoxel>, Option<f32>) {
        let mut refraction_indices: Vec<f32> = Vec::new();
        let voxels: Vec<VisibleVoxel> = self
            .voxels
            .iter()
            .map(|v| VisibleVoxel {
                index: v.0,
                visibility: if *v == RawVoxel::EMPTY {
                    VoxelVisibility::Empty
                } else if let Some(ior) = self.ior_for_voxel.get(&v.0) {
                    refraction_indices.push(*ior);
                    VoxelVisibility::Translucent
                } else {
                    VoxelVisibility::Opaque
                },
            })
            .collect();
        let average_ior: Option<f32> = if refraction_indices.is_empty() {
            None
        } else {
            let ior = 1.0
                + (refraction_indices
                    .iter()
                    .cloned()
                    .reduce(|acc, e| acc + e)
                    .unwrap_or(0.0)
                    / refraction_indices.len() as f32);
            Some(ior)
        };
        (voxels, average_ior)
    }
}
