use bevy::{
    math::{BVec3, UVec3, Vec3},
    render::mesh::Mesh,
    transform::components::GlobalTransform,
    utils::HashMap,
};
use block_mesh::{MergeVoxel, Voxel as BlockyVoxel, VoxelVisibility};
use dot_vox::Model;
use ndshape::{RuntimeShape, Shape};

/// A Voxel. The value is its index in the Magica Voxel palette (1-255), with 0 reserved fpr [`Voxel::EMPTY`].
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
pub struct VoxelData {
    pub(crate) shape: RuntimeShape<u32, 3>,
    pub(crate) voxels: Vec<RawVoxel>,
    pub(crate) ior_for_voxel: HashMap<u8, f32>,
    mesh_outer_faces: bool,
}

impl VoxelData {
    pub(crate) fn padding(&self) -> u32 {
        if self.mesh_outer_faces {
            2
        } else {
            0
        }
    }

    /// The size of the voxel model.
    pub fn size(&self) -> UVec3 {
        let raw_size: UVec3 = self.shape.as_array().into();
        raw_size - UVec3::splat(self.padding())
    }

    /// Converts a global point to a point in voxel coordinates
    ///
    /// ### Arguments
    /// * `global_point` - the point in global space to convert
    /// * `global_xform` - the [`bevy::transform::components::GlobalTransform`] of the entity that owns this [`crate::VoxelModelInstance`]
    ///
    /// ### Returns
    /// A voxel coordinate if the `global_point` lies within the voxel model's bounds, or None if it is outside
    pub fn global_point_to_voxel_space(
        &self,
        global_point: Vec3,
        global_xform: &GlobalTransform,
    ) -> Option<UVec3> {
        let local_position = global_xform
            .affine()
            .inverse()
            .transform_point3(global_point);
        self.local_point_to_voxel_space(local_position)
    }

    /// Converts a local point to a point in voxel coordinates
    ///
    /// ### Arguments
    /// * `local_point` - the point in the local space of the entity that owns this [`crate::VoxelModelInstance`]
    ///
    /// ### Returns
    /// A voxel coordinate if the `local_point` lies within the voxel model's bounds, or None if it is outside
    pub fn local_point_to_voxel_space(&self, local_point: Vec3) -> Option<UVec3> {
        let size = self.size();
        let half_extents = Vec3::new(size.x as f32, size.y as f32, size.z as f32) * 0.5;
        if local_point.abs().greater_than_or_equal(half_extents).any() {
            return None;
        };
        let voxel_postition = local_point + half_extents;
        Some(UVec3::new(
            voxel_postition.x as u32,
            voxel_postition.y as u32,
            voxel_postition.z as u32,
        ))
    }

    /// Returns the [`Voxel`] at the point (given in voxel space)
    ///
    /// ### Arguments
    /// * `position` - the position in voxel space
    ///
    /// ### Returns
    /// the voxel at this point. If the point lies outside the bounds of the model, it will return [`Voxel::EMPTY`].
    pub fn get_voxel_at_point(&self, position: UVec3) -> Voxel {
        let leading_padding = UVec3::splat(self.padding() / 2);
        let index = self.shape.linearize((position + leading_padding).into()) as usize;
        let voxel: Voxel = self
            .voxels
            .get(index)
            .unwrap_or(&RawVoxel::EMPTY)
            .clone()
            .into();
        voxel
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

trait BitwiseComparable {
    fn less_than(&self, other: Vec3) -> BVec3;

    fn greater_than_or_equal(&self, other: Vec3) -> BVec3;
}

impl BitwiseComparable for Vec3 {
    fn less_than(&self, other: Vec3) -> BVec3 {
        BVec3::new(self.x < other.x, self.y < other.y, self.z < other.z)
    }

    fn greater_than_or_equal(&self, other: Vec3) -> BVec3 {
        !self.less_than(other)
    }
}
