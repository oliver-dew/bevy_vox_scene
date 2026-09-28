use block_mesh::{RIGHT_HANDED_Y_UP_CONFIG, UnorientedQuad};

use crate::{AtlasRect, BakedQuad, VisibleVoxel};

/// Paint one packed material island with palette indices, extending edge texels into padding.
#[allow(clippy::too_many_arguments)]
pub fn paint_palette_island(
    atlas: &mut [u8],
    atlas_width: u32,
    packed: AtlasRect,
    quad: &BakedQuad,
    voxels: &[VisibleVoxel],
    dimensions: [u32; 3],
    padding: u32,
    used_palette_indices: &mut [bool; 256],
) {
    let face = &RIGHT_HANDED_Y_UP_CONFIG.faces[quad.face_index];
    let block_quad = UnorientedQuad {
        minimum: quad.minimum,
        width: quad.width,
        height: quad.height,
    };
    let corners = face.quad_corners(&block_quad);
    let u_axis = ((corners[1] - corners[0]) / quad.width).to_array();
    let v_axis = ((corners[2] - corners[0]) / quad.height).to_array();
    for padded_y in 0..quad.height + 2 * padding {
        let source_y = padded_y.saturating_sub(padding).min(quad.height - 1);
        for padded_x in 0..quad.width + 2 * padding {
            let source_x = padded_x.saturating_sub(padding).min(quad.width - 1);
            let position = std::array::from_fn(|axis| {
                quad.minimum[axis] + u_axis[axis] * source_x + v_axis[axis] * source_y
            });
            let palette_index = voxels[linearize(position, dimensions)].index;
            used_palette_indices[palette_index as usize] = true;
            atlas[((packed.y + padded_y) * atlas_width + packed.x + padded_x) as usize] =
                palette_index;
        }
    }
}

pub(crate) fn linearize(position: [u32; 3], dimensions: [u32; 3]) -> usize {
    (position[0] + dimensions[0] * (position[1] + dimensions[1] * position[2])) as usize
}

#[cfg(test)]
mod tests {
    use crate::VoxelVisibility;

    use super::*;

    #[test]
    fn painting_extends_palette_index_through_padding() {
        let dimensions = [3; 3];
        let mut voxels = vec![
            VisibleVoxel {
                index: 255,
                visibility: VoxelVisibility::Empty,
                merge_index: 0,
            };
            27
        ];
        voxels[linearize([1; 3], dimensions)] = VisibleVoxel {
            index: 7,
            visibility: VoxelVisibility::Opaque,
            merge_index: 0,
        };
        let quad = crate::greedy_mesh_quads(&voxels, dimensions)[0];
        let mut atlas = vec![0; 9];
        let mut used = [false; 256];
        paint_palette_island(
            &mut atlas,
            3,
            AtlasRect { x: 0, y: 0 },
            &quad,
            &voxels,
            dimensions,
            1,
            &mut used,
        );
        assert_eq!(atlas, vec![7; 9]);
        assert!(used[7]);
    }
}
