use super::{RawVoxel, Voxel, VoxelModel};
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

impl VoxelModel {
    /// The size of the voxel model.
    pub fn size(&self) -> IVec3 {
        self._size()
    }

    /// The size of the voxel model taking into account the voxel_size
    pub fn model_size(&self) -> Vec3 {
        self._size().as_vec3() * self.bake_settings.voxel_size
    }

    /// Converts a global point to a point in voxel coordinates
    ///
    /// ### Arguments
    /// * `global_point` - the point in global space to convert
    /// * `global_xform` - the [`bevy::transform::components::GlobalTransform`] of the entity that owns this [`crate::VoxelModelInstance`]
    ///
    /// ### Returns
    /// A voxel coordinate
    pub fn global_point_to_voxel_space(
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
    pub fn local_point_to_voxel_space(&self, local_point: Vec3) -> IVec3 {
        let model_offset =
            self.size().as_vec3() * Vec3::from(self.bake_settings.mesh_offset.to_array());
        let voxel_position = (local_point / self.bake_settings.voxel_size) + model_offset;
        voxel_position.floor().as_ivec3()
    }

    /// Converts a voxel coordinate to a point in local space
    ///
    /// ### Arguments
    /// * `voxel_coord` - a voxel coordinate
    ///
    /// ### Returns
    /// the point in the local space of the entity that owns this [`crate::VoxelModelInstance`]
    pub fn voxel_coord_to_local_space(&self, voxel_coord: IVec3) -> Vec3 {
        let model_offset =
            self.size().as_vec3() * Vec3::from(self.bake_settings.mesh_offset.to_array());
        (voxel_coord.as_vec3() - model_offset) * self.bake_settings.voxel_size
    }

    /// If the voxel-space `point` is within the bounds of the model, it will be returned as a [`bevy::math::UVec3`].
    pub fn point_in_model(&self, point: IVec3) -> Result<UVec3, OutOfBoundsError> {
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
    pub fn get_voxel_at_point(&self, position: IVec3) -> Result<Voxel, OutOfBoundsError> {
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

    use crate::{SDF, UnitOffset, VoxBakeSettings, Voxel, VoxelModel};

    #[cfg(feature = "generate_voxels")]
    #[test]
    fn test_voxel_queryable() {
        let mut settings = VoxBakeSettings::default();
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

    #[test]
    fn coordinate_conversion_respects_mesh_offset_and_negative_bounds() {
        let settings = VoxBakeSettings {
            voxel_size: 0.5,
            mesh_offset: UnitOffset::CENTER_BASE,
            ..Default::default()
        };
        let model = VoxelModel::new(UVec3::new(4, 6, 8), settings);

        assert_eq!(
            model.local_point_to_voxel_space(Vec3::ZERO),
            IVec3::new(2, 0, 4)
        );
        assert_eq!(
            model.voxel_coord_to_local_space(IVec3::ZERO),
            Vec3::new(-1.0, 0.0, -2.0)
        );
        assert_eq!(
            model.local_point_to_voxel_space(Vec3::new(-1.01, -0.01, -2.01)),
            IVec3::new(-1, -1, -1)
        );
    }
}
