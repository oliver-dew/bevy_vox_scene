use super::VoxLoaderSettings;
use crate::model::VoxelData;
use vox_bake::{VoxBakeSettings, VoxelGrid};

impl VoxelData {
    pub(crate) fn from_grid(grid: VoxelGrid, settings: VoxLoaderSettings) -> VoxelData {
        let bake_settings = VoxBakeSettings::from(&settings);
        VoxelData {
            grid,
            bake_settings,
            retain_voxel_data: settings.retain_voxel_data,
            mesh_asset_usage: settings.mesh_asset_usage,
        }
    }
}
