use bevy::utils::HashMap;
use dot_vox::Model;
use ndshape::{RuntimeShape, Shape};

use crate::model::{RawVoxel, VoxelData};

/// Ingest Magica Voxel data and perform coordinate conversion from MV's left-handed Z-up to bevy's right-handed Y-up
pub(super) fn load_from_model(
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
