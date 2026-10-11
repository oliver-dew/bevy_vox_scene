//! Compatibility wrapper for offline GLB export.

pub use vox_bake::{ExportError, ExportedGlb, VoxBakeSettings};

use crate::{VoxLoaderSettings, VoxelModel, VoxelPalette};

/// Convert MagicaVoxel bytes into one or more complete GLB files.
///
/// New offline tooling can depend on `vox_bake` directly. This wrapper keeps
/// the existing `bevy_vox_scene` API available for runtime users.
pub fn export_vox_to_glb(
    bytes: &[u8],
    settings: &VoxLoaderSettings,
) -> Result<ExportedGlb, ExportError> {
    vox_bake::export_vox_to_glb(bytes, &VoxBakeSettings::from(settings))
}

/// Export a generated voxel model and palette as a complete GLB file.
pub fn export_voxel_model_to_glb(
    model: &VoxelModel,
    palette: &VoxelPalette,
) -> Result<ExportedGlb, ExportError> {
    vox_bake::export_model_to_glb(&model.grid, &palette.baked, &model.bake_settings)
}

/// Export generated voxel frames and a shared palette as one GLB flipbook animation.
pub fn export_voxel_animation_to_glb(
    frames: &[VoxelModel],
    palette: &VoxelPalette,
) -> Result<ExportedGlb, ExportError> {
    let Some(settings) = frames.first().map(|frame| &frame.bake_settings) else {
        return Err(ExportError::EmptyAnimation);
    };
    if let Some((frame, _)) = frames
        .iter()
        .enumerate()
        .skip(1)
        .find(|(_, model)| model.bake_settings != *settings)
    {
        return Err(ExportError::AnimationSettingsMismatch { frame });
    }
    vox_bake::export_animation_to_glb(
        frames.iter().map(|frame| &frame.grid),
        &palette.baked,
        settings,
    )
}

#[cfg(test)]
mod tests {
    use bevy::math::{IVec3, UVec3};

    use super::{export_voxel_animation_to_glb, export_voxel_model_to_glb};
    use crate::{VoxBakeSettings, Voxel, VoxelModel, VoxelPalette};

    #[test]
    fn exports_generated_model() {
        let mut model = VoxelModel::new(UVec3::ONE, VoxBakeSettings::default());
        model.set_voxel(IVec3::ZERO, Voxel(1)).unwrap();
        let palette = VoxelPalette::from_colors(vec![bevy::color::palettes::css::RED.into()], true);

        let exported = export_voxel_model_to_glb(&model, &palette).unwrap();

        assert_eq!(&exported.bytes[..4], b"glTF");
        assert_eq!(exported.statistics.models, 1);
        assert_eq!(exported.statistics.meshes, 1);
    }

    #[test]
    fn exports_generated_animation() {
        let mut first = VoxelModel::new(UVec3::ONE, VoxBakeSettings::default());
        first.set_voxel(IVec3::ZERO, Voxel(1)).unwrap();
        let mut second = VoxelModel::new(UVec3::new(2, 1, 1), VoxBakeSettings::default());
        second.set_voxel(IVec3::ZERO, Voxel(1)).unwrap();
        let palette = VoxelPalette::from_colors(vec![bevy::color::palettes::css::RED.into()], true);

        let exported = export_voxel_animation_to_glb(&[first, second], &palette).unwrap();

        assert_eq!(&exported.bytes[..4], b"glTF");
        assert_eq!(exported.statistics.models, 2);
        assert_eq!(exported.statistics.meshes, 2);
    }
}
