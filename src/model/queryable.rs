use super::{RawVoxel, Voxel, VoxelData, VoxelModel};
use bevy::{
    math::{BVec3, IVec3, UVec3, Vec3},
    transform::components::GlobalTransform,
};
use ndshape::Shape;

#[derive(Debug, Clone, PartialEq)]
pub struct OutOfBoundsError;

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
        let half_extents = self.size().as_vec3() * 0.5;
        let voxel_postition = local_point + half_extents;
        voxel_postition.as_ivec3()
    }

    /// Converts a voxel coordinate to a point in local space
    ///
    /// ### Arguments
    /// * `voxel_coord` - a voxel coordinate
    ///
    /// ### Returns
    /// the point in the local space of the entity that owns this [`crate::VoxelModelInstance`]
    fn voxel_coord_to_local_space(&self, voxel_coord: IVec3) -> Vec3 {
        let half_extents = self.size().as_vec3() * 0.5;
        voxel_coord.as_vec3() - half_extents
    }

    /// If the voxel-space `point` is within the bounds of the model, it will be returned as a [`bevy::math::UVec3`].
    fn point_in_model(&self, point: IVec3) -> Result<UVec3, OutOfBoundsError> {
        if point.greater_than_or_equal(self.size()).any() {
            return Err(OutOfBoundsError);
        };
        UVec3::try_from(point).map_err(|_| OutOfBoundsError)
    }
    /// Returns the [`Voxel`] at the point (given in voxel space)
    ///
    /// ### Arguments
    /// * `position` - the position in voxel space
    ///
    /// ### Returns
    /// the voxel at this point. If the point lies outside the bounds of the model, it will return [`OutOfBoundsError`].
    fn get_voxel_at_point(&self, position: IVec3) -> Result<Voxel, OutOfBoundsError>;
}

impl VoxelQueryable for VoxelModel {
    /// The size of the voxel model.
    fn size(&self) -> IVec3 {
        self.data.size()
    }

    fn get_voxel_at_point(&self, position: IVec3) -> Result<Voxel, OutOfBoundsError> {
        self.data.get_voxel_at_point(position)
    }
}

impl VoxelQueryable for VoxelData {
    fn size(&self) -> IVec3 {
        self._size()
    }

    fn get_voxel_at_point(&self, position: IVec3) -> Result<Voxel, OutOfBoundsError> {
        let position = self.point_in_model(position)?;
        let leading_padding = UVec3::splat(self.padding() / 2);
        let index = self.shape.linearize((position + leading_padding).into()) as usize;
        let raw_voxel = self.voxels.get(index).ok_or(OutOfBoundsError)?;
        let voxel: Voxel = raw_voxel.clone().into();
        Ok(voxel)
    }
}

impl VoxelData {
    /// Writes a voxel to a point in the model
    ///
    /// ### Arguments
    /// * `voxel` - the [`Voxel`] to be written
    /// * `point` - the position at which the voxel will be written, in voxel space
    ///
    /// ### Returns
    /// [`Result::Ok`] if the operation was successful, or [`OutOfBoundsError`] if `point` lies outside the model
    pub fn set_voxel(&mut self, voxel: Voxel, point: Vec3) -> Result<(), OutOfBoundsError> {
        let position = self.point_in_model(point.as_ivec3())?;
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
