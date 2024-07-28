use bevy::math::UVec3;
use dot_vox::Model;

use crate::model::{RawVoxel, VoxelData};

impl VoxelData {
    /// Ingest Magica Voxel data and perform coordinate conversion from MV's left-handed Z-up to bevy's right-handed Y-up
    pub(super) fn from_model(model: &Model, mesh_outer_faces: bool, voxel_size: f32) -> VoxelData {
        let mut data = VoxelData::new(
            UVec3::new(model.size.x, model.size.z, model.size.y),
            mesh_outer_faces,
            voxel_size,
        );
        model.voxels.iter().for_each(|voxel| {
            let raw_voxel = RawVoxel(voxel.i);
            data.set_voxel(
                raw_voxel.into(),
                UVec3::new(
                    (model.size.x - 1) - voxel.x as u32,
                    voxel.z as u32,
                    voxel.y as u32,
                ),
            );
        });
        data
    }
}
