use super::{RawVoxel, Voxel, VoxelData, VoxelModel};
use bevy::{
    math::{BVec3, IVec3, UVec3, Vec3},
    transform::components::GlobalTransform,
};
use thiserror::Error;

/// A voxel coordinate lies outside a model's bounds.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
#[error("voxel coordinate {point} is outside model bounds {size}")]
pub struct OutOfBoundsError {
    /// The requested voxel coordinate.
    pub point: IVec3,
    /// The model dimensions in voxels.
    pub size: IVec3,
}

/// Methods for converting from global and local to voxel-space coordinates, getting the size of a voxel model, and the voxel at a given point
pub trait VoxelQueryable {
    /// The size of the voxel model.
    fn size(&self) -> IVec3;

    /// The size of the voxel model taking into account the voxel_size
    fn model_size(&self) -> Vec3;

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
    fn local_point_to_voxel_space(&self, local_point: Vec3) -> IVec3;

    /// Converts a voxel coordinate to a point in local space
    ///
    /// ### Arguments
    /// * `voxel_coord` - a voxel coordinate
    ///
    /// ### Returns
    /// the point in the local space of the entity that owns this [`crate::VoxelModelInstance`]
    fn voxel_coord_to_local_space(&self, voxel_coord: IVec3) -> Vec3;

    /// If the voxel-space `point` is within the bounds of the model, it will be returned as a [`bevy::math::UVec3`].
    fn point_in_model(&self, point: IVec3) -> Result<UVec3, OutOfBoundsError> {
        if point.greater_than_or_equal(self.size()).any() {
            return Err(OutOfBoundsError {
                point,
                size: self.size(),
            });
        };
        UVec3::try_from(point).map_err(|_| OutOfBoundsError {
            point,
            size: self.size(),
        })
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

    fn model_size(&self) -> Vec3 {
        self.data.model_size()
    }

    fn local_point_to_voxel_space(&self, local_point: Vec3) -> IVec3 {
        self.data.local_point_to_voxel_space(local_point)
    }

    fn voxel_coord_to_local_space(&self, voxel_coord: IVec3) -> Vec3 {
        self.data.voxel_coord_to_local_space(voxel_coord)
    }

    fn get_voxel_at_point(&self, position: IVec3) -> Result<Voxel, OutOfBoundsError> {
        self.data.get_voxel_at_point(position)
    }
}

impl VoxelQueryable for VoxelData {
    fn size(&self) -> IVec3 {
        self._size()
    }

    fn model_size(&self) -> Vec3 {
        self._size().as_vec3() * self.bake_settings.voxel_size
    }

    fn local_point_to_voxel_space(&self, local_point: Vec3) -> IVec3 {
        let half_extents = self.size().as_vec3() * 0.5;
        let voxel_postition = (local_point / self.bake_settings.voxel_size) + half_extents;
        voxel_postition.as_ivec3()
    }

    fn voxel_coord_to_local_space(&self, voxel_coord: IVec3) -> Vec3 {
        let half_extents = self.size().as_vec3() * 0.5;
        (voxel_coord.as_vec3() - half_extents) * self.bake_settings.voxel_size
    }

    fn get_voxel_at_point(&self, position: IVec3) -> Result<Voxel, OutOfBoundsError> {
        let position = self.point_in_model(position)?;
        let leading_padding = UVec3::splat(self.padding() / 2);
        let index = vox_bake::linearize((position + leading_padding).into(), self.grid.dimensions);
        let raw_voxel = self.grid.voxels.get(index).ok_or(OutOfBoundsError {
            point: position.as_ivec3(),
            size: self.size(),
        })?;
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
    pub fn set_voxel(&mut self, point: IVec3, voxel: Voxel) -> Result<(), OutOfBoundsError> {
        let point = self.point_in_model(point)?;
        let leading_padding = UVec3::splat(self.padding() / 2);
        let index = vox_bake::linearize((point + leading_padding).into(), self.grid.dimensions);
        let raw_voxel: RawVoxel = voxel.into();
        let size = self.size();
        let target = self.grid.voxels.get_mut(index).ok_or(OutOfBoundsError {
            point: point.as_ivec3(),
            size,
        })?;
        *target = raw_voxel;
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

#[cfg(test)]
mod tests {
    use bevy::math::{IVec3, UVec3, Vec3};

    use crate::{SDF, VoxLoaderSettings, Voxel, VoxelQueryable};

    #[cfg(feature = "generate_voxels")]
    #[test]
    fn test_voxel_queryable() {
        let mut settings = VoxLoaderSettings::default();
        settings.voxel_size = 0.25;
        let mut data = SDF::cuboid(Vec3::splat(2.0)).voxelize(UVec3::splat(4), settings, Voxel(1));
        assert_eq!(data.model_size(), Vec3::splat(1.0));
        assert!(data.point_in_model(IVec3::new(3, 0, 0)).is_ok());
        assert!(data.point_in_model(IVec3::new(4, 0, 0)).is_err());
        let error = data
            .set_voxel(IVec3::new(4, 0, 0), Voxel(2))
            .expect_err("out-of-bounds write should fail");
        assert_eq!(error.point, IVec3::new(4, 0, 0));
        assert_eq!(error.size, IVec3::splat(4));
        assert_eq!(
            data.local_point_to_voxel_space(Vec3::ZERO),
            IVec3::new(2, 2, 2)
        );
    }
}
