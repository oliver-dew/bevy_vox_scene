use block_mesh::{MergeVoxel, Voxel as BlockyVoxel, VoxelVisibility};

/// A Voxel. The value is its index in the Magica Voxel palette (1-255), with 0 reserved for [`Voxel::EMPTY`].
#[derive(Clone, PartialEq, Debug)]
pub struct Voxel(pub u8);

impl Voxel {
    /// The value reserved for an empty space.
    pub const EMPTY: Voxel = Voxel(0);
}

/// A Voxel. Material indices run 0-254, with 255 reserved for [`RawVoxel::EMPTY`].
#[derive(Clone, PartialEq, Debug)]
pub(crate) struct RawVoxel(pub u8);

impl RawVoxel {
    /// The value reserved for an empty space.
    pub const EMPTY: RawVoxel = RawVoxel(255);
}

impl From<Voxel> for RawVoxel {
    fn from(value: Voxel) -> Self {
        Self(((value.0 as i16 - 1) % 256 as i16) as u8)
    }
}

impl Into<Voxel> for RawVoxel {
    fn into(self) -> Voxel {
        Voxel(((self.0 as i16 + 1) % 256 as i16) as u8)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct VisibleVoxel {
    pub index: u8,
    pub visibility: VoxelVisibility,
}

impl BlockyVoxel for VisibleVoxel {
    fn get_visibility(&self) -> VoxelVisibility {
        self.visibility
    }
}

impl MergeVoxel for VisibleVoxel {
    type MergeValue = VisibleVoxel;

    fn merge_value(&self) -> Self::MergeValue {
        *self
    }
}
