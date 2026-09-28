//! Compatibility wrapper for offline GLB export.

pub use vox_bake::{ExportError, ExportedGlb, VoxBakeSettings};

use crate::VoxLoaderSettings;

/// Convert MagicaVoxel bytes into one or more complete GLB files.
///
/// New offline tooling can depend on `vox_bake` directly. This wrapper keeps
/// the existing `bevy_vox_scene` API available for runtime users.
pub fn export_vox_to_glb(
    bytes: &[u8],
    settings: &VoxLoaderSettings,
) -> Result<Vec<ExportedGlb>, ExportError> {
    vox_bake::export_vox_to_glb(bytes, &VoxBakeSettings::from(settings))
}
