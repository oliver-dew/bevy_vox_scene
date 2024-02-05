use bevy::math::{UVec3, Vec3};
use dot_vox::Model;

use crate::model::{RawVoxel, VoxelData};

impl VoxelData {
    /// Ingest Magica Voxel data and perform coordinate conversion from MV's left-handed Z-up to bevy's right-handed Y-up
    pub(super) fn from_model(model: &Model, mesh_outer_faces: bool) -> VoxelData {
        let mut data = VoxelData::new(
            UVec3::new(model.size.x, model.size.z, model.size.y),
            mesh_outer_faces,
        );
        model.voxels.iter().for_each(|voxel| {
            let raw_voxel = RawVoxel(voxel.i);
            let _ = data.set_voxel(
                raw_voxel.into(),
                Vec3::new(
                    ((model.size.x - 1) - voxel.x as u32) as f32,
                    voxel.z as f32,
                    voxel.y as f32,
                ),
            );
        });
        data
    }
}
