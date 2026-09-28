use super::VoxLoaderSettings;
use crate::model::VoxelData;
use vox_bake::VoxelGrid;

impl VoxelData {
    pub(crate) fn from_grid(grid: VoxelGrid, settings: VoxLoaderSettings) -> VoxelData {
        VoxelData {
            dimensions: grid.dimensions,
            voxels: grid.voxels,
            settings,
        }
    }
}
