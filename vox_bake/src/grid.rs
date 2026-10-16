use dot_vox::Model;
use serde::{Deserialize, Serialize};

use crate::RawVoxel;

/// Owned, engine-independent voxel grid in right-handed Y-up coordinates.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VoxelGrid {
    /// Grid dimensions including optional one-voxel padding on every face.
    pub dimensions: [u32; 3],
    /// Model dimensions excluding meshing padding.
    pub model_dimensions: [u32; 3],
    /// Linear voxels in X-major order.
    #[serde(with = "raw_voxels")]
    pub voxels: Vec<RawVoxel>,
    /// Total padding across each axis: two when outer faces are meshed, otherwise zero.
    pub padding: u32,
}

impl VoxelGrid {
    /// Create an empty grid for a model of the supplied dimensions.
    pub fn empty(model_dimensions: [u32; 3], mesh_outer_faces: bool) -> Self {
        let padding = if mesh_outer_faces { 2 } else { 0 };
        let dimensions = model_dimensions.map(|dimension| dimension + padding);
        let len = dimensions.iter().product::<u32>() as usize;
        Self {
            dimensions,
            model_dimensions,
            voxels: vec![RawVoxel::EMPTY; len],
            padding,
        }
    }

    /// Parse one MagicaVoxel model and convert it from left-handed Z-up to right-handed Y-up.
    pub fn from_vox_model(model: &Model, mesh_outer_faces: bool) -> Self {
        let model_dimensions = [model.size.x, model.size.z, model.size.y];
        let mut grid = Self::empty(model_dimensions, mesh_outer_faces);
        let leading_padding = grid.padding / 2;
        for voxel in &model.voxels {
            let position = [
                (model.size.x - 1) - voxel.x as u32 + leading_padding,
                voxel.z as u32 + leading_padding,
                voxel.y as u32 + leading_padding,
            ];
            let index = linearize(position, grid.dimensions);
            grid.voxels[index] = RawVoxel(voxel.i);
        }
        grid
    }

    /// Return the linear index for a grid coordinate.
    pub fn linearize(&self, position: [u32; 3]) -> usize {
        linearize(position, self.dimensions)
    }
}

/// Convert an X-major three-dimensional coordinate into a linear index.
pub fn linearize(position: [u32; 3], dimensions: [u32; 3]) -> usize {
    (position[0] + dimensions[0] * (position[1] + dimensions[1] * position[2])) as usize
}

mod raw_voxels {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    use crate::RawVoxel;

    pub fn serialize<S: Serializer>(voxels: &[RawVoxel], serializer: S) -> Result<S::Ok, S::Error> {
        voxels
            .iter()
            .map(|voxel| voxel.0)
            .collect::<Vec<_>>()
            .serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Vec<RawVoxel>, D::Error> {
        Vec::<u8>::deserialize(deserializer)
            .map(|voxels| voxels.into_iter().map(RawVoxel).collect())
    }
}

#[cfg(test)]
mod tests {
    use dot_vox::{Model, Size, Voxel};

    use super::VoxelGrid;
    use crate::RawVoxel;

    #[test]
    fn vox_model_coordinates_are_converted_once_to_right_handed_y_up() {
        let model = Model {
            size: Size { x: 4, y: 5, z: 6 },
            voxels: vec![Voxel {
                x: 1,
                y: 2,
                z: 3,
                i: 17,
            }],
        };

        let grid = VoxelGrid::from_vox_model(&model, true);

        assert_eq!(grid.model_dimensions, [4, 6, 5]);
        assert_eq!(grid.dimensions, [6, 8, 7]);
        assert_eq!(grid.padding, 2);
        // MagicaVoxel X is mirrored, Z becomes Y, and Y becomes Z. Outer-face
        // meshing adds one leading cell on each axis.
        assert_eq!(grid.voxels[grid.linearize([3, 4, 3])], RawVoxel(17));
        assert_eq!(
            grid.voxels
                .iter()
                .filter(|voxel| **voxel != RawVoxel::EMPTY)
                .count(),
            1
        );
    }
}
