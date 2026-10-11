use bevy::math::{Quat, UVec3, Vec3};
use std::sync::Arc;

use crate::{VoxLoaderSettings, Voxel, VoxelData};

/// A 3d signed distance field
#[derive(Clone)]
pub struct SDF {
    distance: Arc<dyn Fn(Vec3) -> f32 + Send + Sync + 'static>,
    contains: Arc<dyn Fn(Vec3) -> bool + Send + Sync + 'static>,
    excludes: Arc<dyn Fn(Vec3) -> bool + Send + Sync + 'static>,
}

impl SDF {
    /// Returns a new field with the supplied distance function
    pub fn new<F: Fn(Vec3) -> f32 + Send + Sync + 'static>(distance: F) -> Self {
        let distance: Arc<dyn Fn(Vec3) -> f32 + Send + Sync + 'static> = Arc::new(distance);
        let contains_distance = Arc::clone(&distance);
        let excludes_distance = Arc::clone(&distance);
        Self {
            distance,
            contains: Arc::new(move |point| contains_distance(point) < 0.0),
            excludes: Arc::new(move |point| excludes_distance(point) > 0.0),
        }
    }

    fn from_parts<Distance, Contains, Excludes>(
        distance: Distance,
        contains: Contains,
        excludes: Excludes,
    ) -> Self
    where
        Distance: Fn(Vec3) -> f32 + Send + Sync + 'static,
        Contains: Fn(Vec3) -> bool + Send + Sync + 'static,
        Excludes: Fn(Vec3) -> bool + Send + Sync + 'static,
    {
        Self {
            distance: Arc::new(distance),
            contains: Arc::new(contains),
            excludes: Arc::new(excludes),
        }
    }

    /// Sphere primitive
    pub fn sphere(radius: f32) -> Self {
        Self::new(move |point| point.length() - radius)
    }

    /// Cuboid primitive
    pub fn cuboid(half_extent: Vec3) -> Self {
        Self::new(move |point| {
            let q = point.abs() - half_extent;
            q.max(Vec3::splat(0.0)).length() + q.max_element().min(0.0)
        })
    }

    fn distance(&self, point: Vec3) -> f32 {
        (self.distance)(point)
    }

    fn contains(&self, point: Vec3) -> bool {
        (self.contains)(point)
    }

    fn excludes(&self, point: Vec3) -> bool {
        (self.excludes)(point)
    }

    /// Add operation (logical OR)
    pub fn add(self, other: SDF) -> Self {
        let distance_self = self.clone();
        let distance_other = other.clone();
        let excludes_self = self.clone();
        let excludes_other = other.clone();
        Self::from_parts(
            move |point| {
                distance_self
                    .distance(point)
                    .min(distance_other.distance(point))
            },
            move |point| self.contains(point) || other.contains(point),
            move |point| excludes_self.excludes(point) && excludes_other.excludes(point),
        )
    }

    /// Subtract operation (logical AND NOT)
    pub fn subtract(self, other: SDF) -> Self {
        let distance_self = self.clone();
        let distance_other = other.clone();
        let excludes_self = self.clone();
        let excludes_other = other.clone();
        Self::from_parts(
            move |point| {
                distance_self
                    .distance(point)
                    .max(-distance_other.distance(point))
            },
            move |point| self.contains(point) && other.excludes(point),
            move |point| excludes_self.excludes(point) || excludes_other.contains(point),
        )
    }

    /// Intersect operation (logical AND)
    pub fn intersect(self, other: SDF) -> Self {
        let distance_self = self.clone();
        let distance_other = other.clone();
        let excludes_self = self.clone();
        let excludes_other = other.clone();
        Self::from_parts(
            move |point| {
                distance_self
                    .distance(point)
                    .max(distance_other.distance(point))
            },
            move |point| self.contains(point) && other.contains(point),
            move |point| excludes_self.excludes(point) || excludes_other.excludes(point),
        )
    }

    /// Translates the input to the field
    pub fn translate(self, delta: Vec3) -> Self {
        let distance_self = self.clone();
        let excludes_self = self.clone();
        Self::from_parts(
            move |point| distance_self.distance(point + delta),
            move |point| self.contains(point + delta),
            move |point| excludes_self.excludes(point + delta),
        )
    }

    /// Rotates the input to the field
    pub fn rotate(self, rotation: Quat) -> Self {
        let inverse = rotation.inverse();
        let distance_self = self.clone();
        let excludes_self = self.clone();
        Self::from_parts(
            move |point| distance_self.distance(inverse.mul_vec3(point)),
            move |point| self.contains(inverse.mul_vec3(point)),
            move |point| excludes_self.excludes(inverse.mul_vec3(point)),
        )
    }

    /// Warps the input to the field using the supplied function
    pub fn warp<F: Fn(Vec3) -> Vec3 + Send + Sync + 'static>(self, warp: F) -> Self {
        let warp = Arc::new(warp);
        let distance_warp = Arc::clone(&warp);
        let excludes_warp = Arc::clone(&warp);
        let distance_self = self.clone();
        let excludes_self = self.clone();
        Self::from_parts(
            move |point| distance_self.distance(distance_warp(point)),
            move |point| self.contains(warp(point)),
            move |point| excludes_self.excludes(excludes_warp(point)),
        )
    }

    /// Distorts the signed distance using the supplied function
    pub fn distort<F: Fn(f32, Vec3) -> f32 + Send + Sync + 'static>(self, distort: F) -> Self {
        Self::new(move |point| distort(self.distance(point), point))
    }

    /// Converts the SDF to [`VoxelData`] by sampling it at each position.
    pub fn map_to_voxels<F: Fn(f32, Vec3) -> Voxel>(
        self,
        size: UVec3,
        settings: VoxLoaderSettings,
        map: F,
    ) -> VoxelData {
        let mut data = VoxelData::new(size, settings);
        let half_extent = Vec3::new(size.x as f32, size.y as f32, size.z as f32) * 0.5;
        for x in 0..size.x {
            for y in 0..size.y {
                for z in 0..size.z {
                    let pos = UVec3::new(x, y, z);
                    let sdf_pos = pos.as_vec3() - half_extent;
                    let distance = self.distance(sdf_pos);
                    let voxel = map(distance, sdf_pos);
                    data.set_voxel(pos.as_ivec3(), voxel)
                        .expect("sampled point is inside generated voxel data");
                }
            }
        }
        data
    }

    /// Converts the inside of the SDF to [`VoxelData`] without calculating exact distances.
    ///
    /// Boolean operations short-circuit while testing occupancy, so expensive fields on the
    /// right-hand side are evaluated only when their result can affect whether a cell is filled.
    pub fn map_inside_to_voxels<F: Fn(Vec3) -> Voxel>(
        self,
        size: UVec3,
        settings: VoxLoaderSettings,
        map: F,
    ) -> VoxelData {
        let mut data = VoxelData::new(size, settings);
        let half_extent = Vec3::new(size.x as f32, size.y as f32, size.z as f32) * 0.5;
        for x in 0..size.x {
            for y in 0..size.y {
                for z in 0..size.z {
                    let pos = UVec3::new(x, y, z);
                    let sdf_pos = pos.as_vec3() - half_extent;
                    let voxel = if self.contains(sdf_pos) {
                        map(sdf_pos)
                    } else {
                        Voxel::EMPTY
                    };
                    data.set_voxel(pos.as_ivec3(), voxel)
                        .expect("sampled point is inside generated voxel data");
                }
            }
        }
        data
    }

    /// Converts the SDF to [`VoxelData`] by filling every cell that is less than 0 with `fill`.
    pub fn voxelize(self, size: UVec3, settings: VoxLoaderSettings, fill: Voxel) -> VoxelData {
        self.map_inside_to_voxels(size, settings, |_| fill.clone())
    }
}

#[cfg(test)]
mod tests {
    use std::{
        f32::consts::FRAC_PI_2,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
    };

    use bevy::math::{Quat, UVec3, Vec3};

    use crate::{SDF, VoxLoaderSettings, Voxel};

    #[test]
    fn test_sdf_intersect() {
        let box_sphere = SDF::cuboid(Vec3::splat(2.0))
            .intersect(SDF::sphere(2.5))
            .voxelize(UVec3::splat(7), VoxLoaderSettings::default(), Voxel(1));
        let sphere_box = SDF::sphere(2.5)
            .intersect(SDF::cuboid(Vec3::splat(2.0)))
            .voxelize(UVec3::splat(7), VoxLoaderSettings::default(), Voxel(1));
        assert_eq!(box_sphere.grid.voxels, sphere_box.grid.voxels);
    }

    #[test]
    fn test_sdf_subtract() {
        let thin_box = SDF::cuboid(Vec3::new(1.0, 2.0, 2.0)).voxelize(
            UVec3::splat(6),
            VoxLoaderSettings::default(),
            Voxel(1),
        );
        let halved_cube = SDF::cuboid(Vec3::new(2.0, 2.0, 2.0))
            .subtract(SDF::cuboid(Vec3::new(1.0, 2.0, 2.0)).translate(Vec3::X))
            .translate(Vec3::X)
            .voxelize(UVec3::splat(6), VoxLoaderSettings::default(), Voxel(1));
        assert_eq!(thin_box.grid.voxels, halved_cube.grid.voxels);
    }

    #[test]
    fn subtract_short_circuits_occupancy_checks() {
        let inner_evaluations = Arc::new(AtomicUsize::new(0));
        let inner_counter = Arc::clone(&inner_evaluations);
        let outside = SDF::new(|_| 1.0).subtract(SDF::new(move |_| {
            inner_counter.fetch_add(1, Ordering::Relaxed);
            -1.0
        }));

        outside.voxelize(UVec3::ONE, VoxLoaderSettings::default(), Voxel(1));

        assert_eq!(inner_evaluations.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_sdf_rotate() {
        let tall_box = SDF::cuboid(Vec3::new(0.5, 2.5, 0.5)).voxelize(
            UVec3::splat(6),
            VoxLoaderSettings::default(),
            Voxel(1),
        );
        let deep_box_rotated = SDF::cuboid(Vec3::new(0.5, 0.5, 2.5))
            .rotate(Quat::from_axis_angle(Vec3::X, FRAC_PI_2))
            .voxelize(UVec3::splat(6), VoxLoaderSettings::default(), Voxel(1));
        assert_eq!(tall_box.grid.voxels, deep_box_rotated.grid.voxels);
    }
}
