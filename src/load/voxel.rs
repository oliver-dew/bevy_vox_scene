use bevy::{math::UVec3, render::mesh::Mesh, utils::HashMap};
use block_mesh::{MergeVoxel, Voxel as BlockyVoxel, VoxelVisibility};
use dot_vox::Model;
use ndshape::{RuntimeShape, Shape};

/// A Voxel. The value is its index in the Magica Voxel palette (1-255), with 0 reserved for [`Voxel::EMPTY`].
#[derive(Clone, PartialEq, Debug)]
pub struct Voxel(pub u8);

impl Voxel {
    /// The value reserved for an empty space.
    pub const EMPTY: Voxel = Voxel(0);
}

/// A Voxel. Material indices run 0-254, with 255 reserved for [`RawVoxel::EMPTY`].
#[derive(Clone, PartialEq, Debug)]
pub(crate) struct RawVoxel(pub u8);

impl RawVoxel {
    /// The value reserved for an empty space.
    pub const EMPTY: RawVoxel = RawVoxel(255);
}

impl From<Voxel> for RawVoxel {
    fn from(value: Voxel) -> Self {
        Self(((value.0 as i16 - 1) % 256 as i16) as u8)
    }
}

impl Into<Voxel> for RawVoxel {
    fn into(self) -> Voxel {
        Voxel(((self.0 as i16 + 1) % 256 as i16) as u8)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct VisibleVoxel {
    pub index: u8,
    visibility: VoxelVisibility,
}

impl BlockyVoxel for VisibleVoxel {
    fn get_visibility(&self) -> VoxelVisibility {
        self.visibility
    }
}

impl MergeVoxel for VisibleVoxel {
    type MergeValue = VisibleVoxel;

    fn merge_value(&self) -> Self::MergeValue {
        *self
    }
}

/// The voxel data used to create a mesh and a material.
///
/// Note that all coordinates are in Bevy's right-handed Y-up space
pub(crate) struct VoxelData {
    pub shape: RuntimeShape<u32, 3>,
    pub voxels: Vec<RawVoxel>,
    pub ior_for_voxel: HashMap<u8, f32>,
    mesh_outer_faces: bool,
}

impl VoxelData {
    /// The size of the voxel model.
    pub(crate) fn size(&self) -> UVec3 {
        let raw_size: UVec3 = self.shape.as_array().into();
        raw_size - UVec3::splat(self.padding())
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
        super::mesh::mesh_model(&visible_voxels, &self)
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

/// Ingest Magica Voxel data and perform coordinate conversion from MV's left-handed Z-up to bevy's right-handed Y-up
pub(crate) fn load_from_model(
    model: &Model,
    ior_for_voxel: &HashMap<u8, f32>,
    mesh_outer_faces: bool,
) -> VoxelData {
    let padding: u32 = if mesh_outer_faces { 2 } else { 0 };
    let shape = RuntimeShape::<u32, 3>::new([
        model.size.x + padding,
        model.size.z + padding,
        model.size.y + padding,
    ]);
    let mut voxels = vec![RawVoxel::EMPTY; shape.size() as usize];

    let leading_padding = padding / 2;

    model.voxels.iter().for_each(|voxel| {
        let index = shape.linearize([
            (model.size.x - 1) - voxel.x as u32 + leading_padding,
            voxel.z as u32 + leading_padding,
            voxel.y as u32 + leading_padding,
        ]) as usize;
        voxels[index] = RawVoxel(voxel.i);
    });

    VoxelData {
        shape,
        voxels,
        ior_for_voxel: ior_for_voxel.clone(),
        mesh_outer_faces,
    }
}
