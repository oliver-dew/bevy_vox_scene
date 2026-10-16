use rectangle_pack::{
    GroupedRectsToPlace, RectToInsert, TargetBin, contains_smallest_box, pack_rects,
    volume_heuristic,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Position of a rectangle within a baked texture atlas.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AtlasRect {
    /// Horizontal texel offset from the atlas origin.
    pub x: u32,
    /// Vertical texel offset from the atlas origin.
    pub y: u32,
}

/// Deterministic placement of a set of rectangles in a texture atlas.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackedAtlas {
    /// Used atlas width after applying atlas alignment.
    pub width: u32,
    /// Used atlas height after applying atlas alignment.
    pub height: u32,
    /// Rectangle origins in the same order as the input sizes.
    pub rectangles: Vec<AtlasRect>,
}

/// Generate UV coordinates for one packed material-atlas quad.
pub fn packed_quad_uvs(
    rect: AtlasRect,
    padding: u32,
    content_size: [u32; 2],
    atlas_size: [u32; 2],
) -> [[f32; 2]; 4] {
    let x = rect.x + padding;
    let y = rect.y + padding;
    let u0 = x as f32 / atlas_size[0] as f32;
    let v0 = y as f32 / atlas_size[1] as f32;
    let u1 = (x + content_size[0]) as f32 / atlas_size[0] as f32;
    let v1 = (y + content_size[1]) as f32 / atlas_size[1] as f32;
    [[u0, v0], [u1, v0], [u0, v1], [u1, v1]]
}

/// Generate UV coordinates sampling the center of one texel in a 16×16 palette.
pub fn palette_quad_uvs(palette_index: u8) -> [[f32; 2]; 4] {
    let u = ((palette_index % 16) as f32 + 0.5) / 16.0;
    let v = ((palette_index / 16) as f32 + 0.5) / 16.0;
    [[u, v]; 4]
}

/// Generate corner-sampling UV coordinates for one ambient-occlusion atlas quad.
pub fn ambient_occlusion_quad_uvs(
    rect: AtlasRect,
    padding: u32,
    quad_size: [u32; 2],
    atlas_size: [u32; 2],
) -> [[f32; 2]; 4] {
    let x = rect.x + padding;
    let y = rect.y + padding;
    let u0 = (x as f32 + 0.5) / atlas_size[0] as f32;
    let v0 = (y as f32 + 0.5) / atlas_size[1] as f32;
    let u1 = (x as f32 + quad_size[0] as f32 + 0.5) / atlas_size[0] as f32;
    let v1 = (y as f32 + quad_size[1] as f32 + 0.5) / atlas_size[1] as f32;
    [[u0, v0], [u1, v0], [u0, v1], [u1, v1]]
}

/// Pack content rectangles into a deterministic, aligned atlas.
pub fn pack_atlas_rectangles(
    content_sizes: &[(u32, u32)],
    padding: u32,
    atlas_alignment: u32,
    max_dimension: u32,
    rectangle_alignment: u32,
) -> Option<PackedAtlas> {
    let atlas_alignment = atlas_alignment.max(1);
    let rectangle_alignment = rectangle_alignment.max(1);
    let mut rectangles: GroupedRectsToPlace<usize, ()> = GroupedRectsToPlace::new();
    let mut total_area = 0_u64;
    let mut minimum_width = 1;
    let mut minimum_height = 1;
    for (id, &(content_width, content_height)) in content_sizes.iter().enumerate() {
        let padded = padding.checked_mul(2)?;
        let width = align(content_width.checked_add(padded)?, rectangle_alignment);
        let height = align(content_height.checked_add(padded)?, rectangle_alignment);
        total_area += u64::from(width) * u64::from(height);
        minimum_width = minimum_width.max(width);
        minimum_height = minimum_height.max(height);
        rectangles.push_rect(id, None, RectToInsert::new(width, height, 1));
    }
    let square_width = (total_area as f64).sqrt().ceil() as u32;
    let mut widths = vec![
        minimum_width,
        square_width,
        square_width.saturating_mul(5) / 4,
        square_width.saturating_mul(3) / 2,
        square_width.saturating_mul(2),
        max_dimension,
    ];
    for width in &mut widths {
        *width = align((*width).max(minimum_width), atlas_alignment).min(max_dimension);
    }
    widths.sort_unstable();
    widths.dedup();
    let mut best: Option<(u64, PackedAtlas)> = None;
    for width in widths {
        if width < minimum_width {
            continue;
        }
        let area_height = total_area.div_ceil(u64::from(width)) as u32;
        let mut height = align(area_height.max(minimum_height), atlas_alignment);
        while height <= max_dimension {
            let mut bins = BTreeMap::from([(0_u8, TargetBin::new(width, height, 1))]);
            if let Ok(result) = pack_rects(
                &rectangles,
                &mut bins,
                &volume_heuristic,
                &contains_smallest_box,
            ) {
                let mut packed = vec![AtlasRect { x: 0, y: 0 }; content_sizes.len()];
                let mut used_width = 1;
                let mut used_height = 1;
                for (&id, (_, location)) in result.packed_locations() {
                    packed[id] = AtlasRect {
                        x: location.x(),
                        y: location.y(),
                    };
                    used_width = used_width.max(location.x() + location.width());
                    used_height = used_height.max(location.y() + location.height());
                }
                used_width = align(used_width, atlas_alignment);
                used_height = align(used_height, atlas_alignment);
                let area = u64::from(used_width) * u64::from(used_height);
                let aspect = used_width.abs_diff(used_height);
                let replace = best.as_ref().is_none_or(|(best_area, best_atlas)| {
                    area < *best_area
                        || (area == *best_area
                            && aspect < best_atlas.width.abs_diff(best_atlas.height))
                });
                if replace {
                    best = Some((
                        area,
                        PackedAtlas {
                            width: used_width,
                            height: used_height,
                            rectangles: packed,
                        },
                    ));
                }
                break;
            }
            if height == max_dimension {
                break;
            }
            height = align(
                (height.saturating_mul(3) / 2).max(height.saturating_add(atlas_alignment)),
                atlas_alignment,
            )
            .min(max_dimension);
        }
    }
    best.map(|(_, atlas)| atlas)
}

fn align(value: u32, alignment: u32) -> u32 {
    value.div_ceil(alignment).saturating_mul(alignment)
}
