use crate::{VoxelModel, VoxelPalette};

/// Export a generated voxel model and palette as MagicaVoxel bytes.
pub fn export_voxel_model_to_vox(
    model: &VoxelModel,
    palette: &VoxelPalette,
) -> Result<Vec<u8>, vox_bake::VoxWriteError> {
    vox_bake::export_model_to_vox(&model.grid, &palette.baked)
}

/// Export generated voxel frames and a shared palette as a MagicaVoxel shape animation.
pub fn export_voxel_animation_to_vox(
    frames: &[VoxelModel],
    palette: &VoxelPalette,
) -> Result<Vec<u8>, vox_bake::VoxWriteError> {
    let mesh_offset = frames
        .first()
        .map(|frame| &frame.bake_settings.mesh_offset)
        .unwrap_or(&vox_bake::UnitOffset::CENTER);
    vox_bake::export_animation_to_vox_with_offset(
        frames.iter().map(|frame| &frame.grid),
        &palette.baked,
        mesh_offset,
    )
}

#[cfg(test)]
mod tests {
    use bevy::math::{IVec3, UVec3};

    use super::{export_voxel_animation_to_vox, export_voxel_model_to_vox};
    use crate::{VoxBakeSettings, Voxel, VoxelModel, VoxelPalette};

    #[test]
    fn exports_generated_model() {
        let mut model = VoxelModel::new(UVec3::ONE, VoxBakeSettings::default());
        model.set_voxel(IVec3::ZERO, Voxel(1)).unwrap();
        let palette = VoxelPalette::from_colors(vec![bevy::color::palettes::css::RED.into()], true);

        let bytes = export_voxel_model_to_vox(&model, &palette).unwrap();
        let decoded = vox_bake::source::parse_vox(&bytes).unwrap();

        assert_eq!(decoded.models.len(), 1);
        assert_eq!(decoded.models[0].voxels.len(), 1);
    }

    #[test]
    fn exports_generated_animation() {
        let frames = [
            VoxelModel::new(UVec3::ONE, VoxBakeSettings::default()),
            VoxelModel::new(UVec3::new(2, 1, 1), VoxBakeSettings::default()),
        ];
        let palette = VoxelPalette::from_colors(Vec::new(), true);

        let bytes = export_voxel_animation_to_vox(&frames, &palette).unwrap();
        let decoded = vox_bake::source::parse_vox(&bytes).unwrap();

        assert_eq!(decoded.models.len(), 2);
    }
}
