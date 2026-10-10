use super::VoxLoaderSettings;
use crate::model::VoxelData;
use vox_bake::{VoxBakeSettings, VoxelGrid};

impl VoxelData {
    pub(crate) fn from_grid(grid: VoxelGrid, settings: VoxLoaderSettings) -> VoxelData {
        let bake_settings = VoxBakeSettings::from(&settings);
        VoxelData {
            grid,
            bake_settings,
            supports_remeshing: settings.supports_remeshing,
        }
    }
}
