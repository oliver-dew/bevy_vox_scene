pub(crate) use vox_bake::RawVoxel;

/// A Voxel. The value is its index in the Magica Voxel palette (1-255), with 0 reserved for [`Voxel::EMPTY`].
#[derive(Clone, PartialEq, Debug)]
pub struct Voxel(pub u8);

impl Voxel {
    /// The value reserved for an empty space.
    pub const EMPTY: Voxel = Voxel(0);
}

impl From<Voxel> for RawVoxel {
    fn from(value: Voxel) -> Self {
        Self(((value.0 as i16 - 1) % 256_i16) as u8)
    }
}

impl From<RawVoxel> for Voxel {
    fn from(value: RawVoxel) -> Voxel {
        Voxel(((value.0 as i16 + 1) % 256_i16) as u8)
    }
}

#[cfg(test)]
mod tests {
    use super::{RawVoxel, Voxel};

    #[test]
    fn test_raw_voxel_conversion() {
        let raw = RawVoxel::EMPTY;
        let vox: Voxel = raw.into();
        assert_eq!(
            vox,
            Voxel::EMPTY,
            "RawVoxel(255) should have wrapped around to Voxel(0)"
        );
        let back_to_raw = RawVoxel::from(vox);
        assert_eq!(
            back_to_raw,
            RawVoxel::EMPTY,
            "Voxel(0) should have wrapped around to RawVoxel(255)"
        );
    }
}
