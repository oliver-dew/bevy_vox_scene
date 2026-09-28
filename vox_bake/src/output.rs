use serde::{Deserialize, Serialize};

use crate::TextureMipPolicy;

/// Triangle mesh data produced by voxel baking, independent of a rendering engine.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BakedMesh {
    /// Vertex positions in right-handed Y-up space.
    pub positions: Vec<[f32; 3]>,
    /// Per-vertex surface normals.
    pub normals: Vec<[f32; 3]>,
    /// Primary material texture coordinates.
    pub uv0: Vec<[f32; 2]>,
    /// Optional ambient-occlusion texture coordinates.
    pub uv1: Option<Vec<[f32; 2]>>,
    /// Triangle indices.
    pub indices: Vec<u32>,
}

/// Pixel encoding and color-space interpretation for a baked image.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BakedPixelFormat {
    /// One linear unsigned-normalized channel.
    R8Linear,
    /// Four linear unsigned-normalized channels.
    Rgba8Linear,
    /// Four sRGB-encoded color channels.
    Rgba8Srgb,
}

/// Owned 2D image data produced during baking.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BakedImage {
    /// Image width in texels.
    pub width: u32,
    /// Image height in texels.
    pub height: u32,
    /// Pixel encoding and color-space interpretation.
    pub format: BakedPixelFormat,
    /// Tightly packed pixel bytes in row-major order.
    pub pixels: Vec<u8>,
}

/// Engine-independent mesh and texture-atlas result for one voxel model.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BakedMeshAtlas {
    /// Surface geometry.
    pub mesh: BakedMesh,
    /// Material atlas width.
    pub width: u32,
    /// Material atlas height.
    pub height: u32,
    /// Palette index stored for each material-atlas texel.
    pub palette_indices: Vec<u8>,
    /// Palette indices referenced by this model.
    pub used_palette_indices: Vec<u8>,
    /// Optional linear ambient-occlusion atlas.
    pub ambient_occlusion: Option<BakedImage>,
    /// Mip layout used when packing the material atlas.
    pub mip_policy: TextureMipPolicy,
}

/// Engine-independent 3D density field for voxel cloud materials.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BakedVolume {
    /// Width, height, and depth in voxels.
    pub dimensions: [u32; 3],
    /// Density values in the source model's linear voxel order.
    pub densities: Vec<f32>,
}
