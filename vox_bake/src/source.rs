//! MagicaVoxel source parsing and source-format types.

pub use dot_vox::{DotVoxData, Frame, Layer, Model, SceneNode};

/// Parse a complete MagicaVoxel file from memory.
pub fn parse_vox(bytes: &[u8]) -> Result<DotVoxData, String> {
    dot_vox::load_bytes(bytes).map_err(Into::into)
}
