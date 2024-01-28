use bevy::{
    asset::{Asset, Assets, Handle},
    ecs::system::ResMut,
    math::{IVec3, UVec3},
    pbr::StandardMaterial,
    reflect::TypePath,
    render::mesh::Mesh,
    utils::HashMap,
};
use block_mesh::VoxelVisibility;
use ndshape::{RuntimeShape, Shape};

use self::voxel::VisibleVoxel;
pub use self::voxel::Voxel;
pub(crate) use voxel::RawVoxel;

pub(super) mod mesh;
#[cfg(feature = "modify_voxels")]
pub(super) mod modify;
#[cfg(feature = "modify_voxels")]
pub(super) mod queryable;
#[cfg(feature = "modify_voxels")]
pub use self::queryable::VoxelQueryable;
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

impl VoxelModel {
    /// Create a new VoxelModel from [`VoxelData`]
    pub fn new(
        data: VoxelData,
        meshes: &mut ResMut<Assets<Mesh>>,
        material: Handle<StandardMaterial>,
    ) -> Self {
        let mesh = data.remesh();
        VoxelModel {
            data,
            mesh: meshes.add(mesh),
            material,
        }
    }
}

/// The voxel data used to create a mesh and a material.
pub struct VoxelData {
    pub(crate) shape: RuntimeShape<u32, 3>,
    pub(crate) voxels: Vec<RawVoxel>,
    pub(crate) ior_for_voxel: HashMap<u8, f32>,
    pub(crate) mesh_outer_faces: bool,
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
    /// The size of the voxel model.
    pub(crate) fn _size(&self) -> IVec3 {
        let raw_size: UVec3 = self.shape.as_array().into();
        let padded = raw_size - UVec3::splat(self.padding());
        IVec3::try_from(padded).unwrap_or(IVec3::ZERO)
    }

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
