use super::{RawVoxel, Voxel, VoxelData, VoxelModel};
use bevy::{
    math::{BVec3, IVec3, UVec3, Vec3},
    transform::components::GlobalTransform,
};
use ndshape::Shape;

/// Methods for converting from global and local to voxel-space coordinates, getting the size of a voxel model, and the voxel at a given point
pub trait VoxelQueryable {
    /// The size of the voxel model.
    fn size(&self) -> IVec3;

    /// Converts a global point to a point in voxel coordinates
    ///
    /// ### Arguments
    /// * `global_point` - the point in global space to convert
    /// * `global_xform` - the [`bevy::transform::components::GlobalTransform`] of the entity that owns this [`crate::VoxelModelInstance`]
    ///
    /// ### Returns
    /// A voxel coordinate
    fn global_point_to_voxel_space(
        &self,
        global_point: Vec3,
        global_xform: &GlobalTransform,
    ) -> IVec3 {
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
    /// A voxel coordinate
    fn local_point_to_voxel_space(&self, local_point: Vec3) -> IVec3 {
        let size = self.size();
        let half_extents = Vec3::new(size.x as f32, size.y as f32, size.z as f32) * 0.5;
        let voxel_postition = local_point + half_extents;
        IVec3::new(
            voxel_postition.x as i32,
            voxel_postition.y as i32,
            voxel_postition.z as i32,
        )
    }

    /// If the voxel-space `point` is within the bounds of the model, it will be returned as a [`bevy::math::UVec3`].
    fn point_in_model(&self, point: IVec3) -> Option<UVec3> {
        if point.greater_than_or_equal(self.size()).any() {
            return None;
        };
        UVec3::try_from(point).ok()
    }
    /// Returns the [`Voxel`] at the point (given in voxel space)
    ///
    /// ### Arguments
    /// * `position` - the position in voxel space
    ///
    /// ### Returns
    /// the voxel at this point. If the point lies outside the bounds of the model, it will return [`None`].
    fn get_voxel_at_point(&self, position: IVec3) -> Option<Voxel>;
}

impl VoxelQueryable for VoxelModel {
    /// The size of the voxel model.
    fn size(&self) -> IVec3 {
        self.data.size()
    }

    fn get_voxel_at_point(&self, position: IVec3) -> Option<Voxel> {
        self.data.get_voxel_at_point(position)
    }
}

impl VoxelQueryable for VoxelData {
    /// The size of the voxel model.
    fn size(&self) -> IVec3 {
        let raw_size: UVec3 = self.shape.as_array().into();
        let padded = raw_size - UVec3::splat(self.padding());
        IVec3::try_from(padded).unwrap_or(IVec3::ZERO)
    }

    fn get_voxel_at_point(&self, position: IVec3) -> Option<Voxel> {
        let position = self.point_in_model(position)?;
        let leading_padding = UVec3::splat(self.padding() / 2);
        let index = self.shape.linearize((position + leading_padding).into()) as usize;
        let raw_voxel = self.voxels.get(index)?;
        let voxel: Voxel = raw_voxel.clone().into();
        Some(voxel)
    }
}

#[derive(Debug, Clone)]
pub struct OutOfBoundsError;

impl VoxelData {
    /// Writes a voxel to a point in the model
    ///
    /// ### Arguments
    /// * `voxel` - the [`Voxel`] to be written
    /// * `point` - the position at which the voxel will be written
    ///
    /// ### Returns
    /// `Ok(())` if the operation was successful, or [`OutOfBoundsError`] if `point` lies outside the model
    pub fn set_voxel(&mut self, voxel: Voxel, point: Vec3) -> Result<(), OutOfBoundsError> {
        let position = self
            .point_in_model(point.as_ivec3())
            .ok_or(OutOfBoundsError)?;
        let leading_padding = UVec3::splat(self.padding() / 2);
        let index = self.shape.linearize((position + leading_padding).into()) as usize;
        let raw_voxel: RawVoxel = voxel.into();
        self.voxels[index] = raw_voxel;
        Ok(())
    }
}
trait BitwiseComparable {
    fn less_than(&self, other: Self) -> BVec3;

    fn greater_than_or_equal(&self, other: Self) -> BVec3;
}

impl BitwiseComparable for IVec3 {
    fn less_than(&self, other: IVec3) -> BVec3 {
        BVec3::new(self.x < other.x, self.y < other.y, self.z < other.z)
    }

    fn greater_than_or_equal(&self, other: IVec3) -> BVec3 {
        !self.less_than(other)
    }
}
