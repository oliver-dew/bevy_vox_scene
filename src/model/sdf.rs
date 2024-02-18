use bevy::math::{Quat, UVec3, Vec3};

use crate::{Voxel, VoxelData};

/// A 3d signed distance field
pub struct SDF {
    distance: Box<dyn Fn(Vec3) -> f32 + Send + Sync + 'static>,
}

impl SDF {
    /// Returns a new field with the supplied distance function
    pub fn new<F: Fn(Vec3) -> f32 + Send + Sync + 'static>(distance: F) -> Self {
        Self {
            distance: Box::new(distance),
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

    /// Add operation (logical OR)
    pub fn add(self, other: SDF) -> Self {
        Self::new(move |point| self.distance(point).min(other.distance(point)))
    }

    /// Subtract operation (logical AND NOT)
    pub fn subtract(self, other: SDF) -> Self {
        Self::new(move |point| self.distance(point).max(-other.distance(point)))
    }

    /// Intersect operation (logical AND)
    pub fn intersect(self, other: SDF) -> Self {
        Self::new(move |point| self.distance(point).max(other.distance(point)))
    }

    /// Translates the input to the field
    pub fn translate(self, delta: Vec3) -> Self {
        Self::new(move |point| self.distance(point + delta))
    }

    /// Rotates the input to the field
    pub fn rotate(self, rotation: Quat) -> Self {
        let inverse = rotation.inverse();
        Self::new(move |point| self.distance(inverse.mul_vec3(point)))
    }

    /// Warps the input to the field using the supplied function
    pub fn warp<F: Fn(Vec3) -> Vec3 + Send + Sync + 'static>(self, warp: F) -> Self {
        Self::new(move |point| self.distance(warp(point)))
    }

    /// Distorts the signed distance using the supplied funtion
    pub fn distort<F: Fn(f32, Vec3) -> f32 + Send + Sync + 'static>(self, distort: F) -> Self {
        Self::new(move |point| distort(self.distance(point), point))
    }

    /// Converts the SDF to [`VoxelData`] by sampling it at each position.
    pub fn map_to_voxels<F: Fn(f32, Vec3) -> Voxel>(self, size: UVec3, map: F) -> VoxelData {
        let mut data = VoxelData::new(size, true);
        let half_extent = Vec3::new(size.x as f32, size.y as f32, size.z as f32) * 0.5;
        for x in 0..size.x {
            for y in 0..size.y {
                for z in 0..size.z {
                    let pos = Vec3::new(x as f32, y as f32, z as f32);
                    let sdf_pos = pos - half_extent;
                    let distance = self.distance(sdf_pos);
                    let voxel = map(distance, sdf_pos);
                    let _ = data.set_voxel(voxel, pos);
                }
            }
        }
        data
    }

    /// Converts the SDF to [`VoxelData`] by filling every cell that is less than 0 with `fill`.
    pub fn voxelize(self, size: UVec3, fill: Voxel) -> VoxelData {
        self.map_to_voxels(size, |distance, _| {
            if distance < 0.0 {
                fill.clone()
            } else {
                Voxel::EMPTY
            }
        })
    }
}
