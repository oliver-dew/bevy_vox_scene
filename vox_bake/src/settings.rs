use serde::{Deserialize, Serialize};

/// Settings that affect engine-independent voxel baking output.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct VoxBakeSettings {
    /// Timeline rate used for MagicaVoxel transform frame indices.
    #[serde(default = "default_animation_frames_per_second")]
    pub animation_frames_per_second: f32,
    /// Length of each side of a voxel.
    pub voxel_size: f32,
    /// Whether faces on the outside boundary of a model are generated.
    pub mesh_outer_faces: bool,
    /// Mesh-origin offset expressed as a proportion of model size.
    pub mesh_offset: UnitOffset,
    /// Multiplier applied to emissive materials.
    pub emission_strength: f32,
    /// Whether source palette colors are interpreted as sRGB.
    pub uses_srgb: bool,
    /// Roughness assigned to diffuse MagicaVoxel materials.
    pub diffuse_roughness: f32,
    /// Texture layout used by baked voxel meshes.
    #[serde(default)]
    pub texture_mode: VoxelTextureMode,
}

impl Default for VoxBakeSettings {
    fn default() -> Self {
        Self {
            animation_frames_per_second: default_animation_frames_per_second(),
            voxel_size: 1.0,
            mesh_outer_faces: true,
            mesh_offset: UnitOffset::CENTER,
            emission_strength: 20.0,
            uses_srgb: true,
            diffuse_roughness: 0.8,
            texture_mode: VoxelTextureMode::default(),
        }
    }
}

/// Default rate used to convert MagicaVoxel frame indices to seconds.
pub const fn default_animation_frames_per_second() -> f32 {
    8.0
}

/// Controls voxel-residency ambient occlusion baked into packed texture atlases.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct AmbientOcclusionSettings {
    /// Height and radius of the sampling cone. Zero disables AO; values above 8 are clamped.
    pub depth: usize,
    /// Multiplier applied to normalized occlusion. Negative and non-finite values act as zero.
    pub intensity: f32,
}

impl Default for AmbientOcclusionSettings {
    fn default() -> Self {
        Self {
            depth: 2,
            intensity: 1.0,
        }
    }
}

/// Selects the per-model texture layout used by voxel meshes.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub enum VoxelTextureMode {
    /// Merge coplanar faces regardless of color and bake their voxel data into packed islands.
    Packed(TexturePackSettings),
    /// Use a 16x16 palette texture and merge faces only when their palette indices match.
    Palette,
}

impl Default for VoxelTextureMode {
    fn default() -> Self {
        Self::Packed(TexturePackSettings::default())
    }
}

/// Controls the layout of a packed per-model texture atlas.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct TexturePackSettings {
    /// Texels added around each face island and filled by extending its edge texels.
    #[serde(default = "default_atlas_padding")]
    pub padding: u32,
    /// Atlas dimensions are rounded up to a multiple of this value.
    #[serde(default = "default_atlas_alignment")]
    pub alignment: u32,
    /// Maximum width or height of the atlas.
    #[serde(default = "default_max_atlas_dimension")]
    pub max_dimension: u32,
    /// Optional mipmap generation. Limited mip chains increase island padding and alignment.
    #[serde(default)]
    pub mip_policy: TextureMipPolicy,
    /// Optional voxel-residency ambient occlusion baked into a second packed atlas.
    #[serde(default)]
    pub ambient_occlusion: Option<AmbientOcclusionSettings>,
    /// Log atlas dimensions, occupancy, packing overhead, and geometry reduction.
    #[serde(default)]
    pub log_diagnostics: bool,
}

impl Default for TexturePackSettings {
    fn default() -> Self {
        Self {
            padding: default_atlas_padding(),
            alignment: default_atlas_alignment(),
            max_dimension: default_max_atlas_dimension(),
            mip_policy: TextureMipPolicy::Disabled,
            ambient_occlusion: None,
            log_diagnostics: false,
        }
    }
}

/// Controls mipmap generation for packed texture atlases.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextureMipPolicy {
    /// Do not generate mipmaps. This preserves the existing atlas layout and memory use.
    #[default]
    Disabled,
    /// Generate mip levels through `max_lod`. Values above 4 are clamped to 4.
    Limited {
        /// Highest generated mip level; level 0 is the original atlas.
        max_lod: u8,
    },
}

impl TextureMipPolicy {
    /// Return the maximum generated mip level, clamped to the supported range.
    pub fn max_lod(self) -> u8 {
        match self {
            Self::Disabled => 0,
            Self::Limited { max_lod } => max_lod.min(4),
        }
    }

    /// Return the texel alignment required by this mip policy.
    pub fn alignment(self) -> u32 {
        1 << self.max_lod()
    }
}

/// An offset applied to mesh positions, expressed as a proportion of model size.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct UnitOffset([f32; 3]);

impl UnitOffset {
    /// Do not offset the mesh, leaving its origin at the minimum of its bounds.
    pub const ZERO: Self = Self([0.0, 0.0, 0.0]);
    /// Center the model around its origin.
    pub const CENTER: Self = Self([0.5, 0.5, 0.5]);
    /// Center the model horizontally with its base at the origin.
    pub const CENTER_BASE: Self = Self([0.5, 0.0, 0.5]);

    /// Return the offset as XYZ components.
    pub const fn to_array(&self) -> [f32; 3] {
        self.0
    }
}

const fn default_atlas_padding() -> u32 {
    1
}
const fn default_atlas_alignment() -> u32 {
    4
}
const fn default_max_atlas_dimension() -> u32 {
    8192
}
