use block_mesh::{MergeVoxel, Voxel as BlockyVoxel, VoxelVisibility as BlockMeshVisibility};

use crate::{BakedVolume, VoxelTextureMode};

/// Compact voxel storage using zero-based palette indices, with 255 reserved for empty space.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RawVoxel(pub u8);

impl RawVoxel {
    /// The value reserved for empty space.
    pub const EMPTY: Self = Self(255);
}

/// Whether a voxel contributes an opaque surface, a translucent surface, or no surface.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum VoxelVisibility {
    /// No surface is generated.
    #[default]
    Empty,
    /// An opaque surface is generated.
    Opaque,
    /// A translucent surface is generated.
    Translucent,
}

impl From<VoxelVisibility> for BlockMeshVisibility {
    fn from(value: VoxelVisibility) -> Self {
        match value {
            VoxelVisibility::Empty => Self::Empty,
            VoxelVisibility::Opaque => Self::Opaque,
            VoxelVisibility::Translucent => Self::Translucent,
        }
    }
}

/// A voxel classified for surface meshing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VisibleVoxel {
    /// Zero-based palette index inherited from the source voxel.
    pub index: u8,
    /// Whether the voxel contributes an opaque or translucent surface.
    pub visibility: VoxelVisibility,
    /// Value used to decide whether adjacent voxel faces can be merged.
    pub merge_index: u8,
}

impl BlockyVoxel for VisibleVoxel {
    fn get_visibility(&self) -> BlockMeshVisibility {
        self.visibility.into()
    }
}

impl MergeVoxel for VisibleVoxel {
    type MergeValue = (BlockMeshVisibility, u8);

    fn merge_value(&self) -> Self::MergeValue {
        (self.visibility.into(), self.merge_index)
    }
}

/// Surface classification result for a linear voxel buffer.
#[derive(Clone, Debug, PartialEq)]
pub struct ClassifiedVoxels {
    /// Voxels annotated for the surface mesher.
    pub voxels: Vec<VisibleVoxel>,
    /// Mean index of refraction across translucent voxels, when present.
    pub average_ior: Option<f32>,
    /// Whether at least one voxel contributes surface geometry.
    pub has_surface: bool,
    /// Whether at least one voxel contributes cloud volume.
    pub has_cloud: bool,
}

/// Classify raw voxels for surface meshing using per-palette material properties.
pub fn classify_voxels(
    voxels: &[RawVoxel],
    texture_mode: VoxelTextureMode,
    ior_for_voxel: &[Option<f32>],
    density_for_voxel: &[Option<f32>],
) -> ClassifiedVoxels {
    let mut refraction_sum = 0.0;
    let mut refraction_count = 0;
    let mut has_cloud = false;
    let voxels = voxels
        .iter()
        .map(|voxel| {
            let index = voxel.0;
            let merge_index = match texture_mode {
                VoxelTextureMode::Packed(_) => 0,
                VoxelTextureMode::Palette => index,
            };
            let visibility = if *voxel == RawVoxel::EMPTY {
                VoxelVisibility::Empty
            } else if let Some(ior) = ior_for_voxel[index as usize] {
                refraction_sum += ior;
                refraction_count += 1;
                VoxelVisibility::Translucent
            } else if density_for_voxel[index as usize].is_some() {
                has_cloud = true;
                VoxelVisibility::Empty
            } else {
                VoxelVisibility::Opaque
            };
            VisibleVoxel {
                index,
                visibility,
                merge_index,
            }
        })
        .collect::<Vec<_>>();
    let average_ior = (refraction_count > 0).then_some(refraction_sum / refraction_count as f32);
    let has_surface = voxels
        .iter()
        .any(|voxel| voxel.visibility != VoxelVisibility::Empty);
    ClassifiedVoxels {
        voxels,
        average_ior,
        has_surface,
        has_cloud,
    }
}

/// Extract a cloud-density volume from a voxel buffer with one padding voxel on every face.
pub fn extract_cloud_volume(
    voxels: &[RawVoxel],
    dimensions: [u32; 3],
    density_for_voxel: &[Option<f32>],
) -> Option<BakedVolume> {
    if dimensions.iter().any(|dimension| *dimension < 2) {
        return None;
    }
    let inner_dimensions = dimensions.map(|dimension| dimension - 2);
    let mut densities = Vec::with_capacity(inner_dimensions.iter().product::<u32>() as usize);
    for z in 1..dimensions[2] - 1 {
        for y in 1..dimensions[1] - 1 {
            for x in 1..dimensions[0] - 1 {
                let voxel = voxels[(x + dimensions[0] * (y + dimensions[1] * z)) as usize];
                let density = density_for_voxel[voxel.0 as usize];
                densities.push(density.unwrap_or(0.0));
            }
        }
    }
    Some(BakedVolume {
        dimensions: inner_dimensions,
        densities,
    })
}
