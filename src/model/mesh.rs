use std::collections::BTreeMap;

use bevy::{
    asset::RenderAssetUsages,
    math::Vec3,
    mesh::{Indices, Mesh, VertexAttributeValues},
    render::render_resource::PrimitiveTopology,
};
use block_mesh::{
    GreedyQuadsBuffer, OrientedBlockFace, RIGHT_HANDED_Y_UP_CONFIG, UnorientedQuad,
    VoxelVisibility, greedy_quads,
};
use ndshape::Shape;
use rectangle_pack::{
    GroupedRectsToPlace, RectToInsert, TargetBin, contains_smallest_box, pack_rects,
    volume_heuristic,
};

use super::{VoxelData, queryable::VoxelQueryable, voxel::VisibleVoxel};
use crate::VoxelTextureMode;

pub(crate) struct MeshAtlas {
    pub(crate) mesh: Mesh,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) palette_indices: Vec<u8>,
    pub(crate) used_palette_indices: Vec<u8>,
    pub(crate) ambient_occlusion: Option<AmbientOcclusionAtlas>,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct AmbientOcclusionAtlas {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) data: Vec<u8>,
}

#[derive(Clone, Copy, Debug)]
struct Island {
    quad: UnorientedQuad,
    face_index: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PackedRect {
    x: u32,
    y: u32,
}

#[derive(Clone, Copy, Debug)]
struct AmbientOcclusionSample {
    offset: [i32; 3],
    weight: f32,
}

pub(crate) fn mesh_model(voxels: &[VisibleVoxel], data: &VoxelData) -> MeshAtlas {
    let mut greedy_quads_buffer = GreedyQuadsBuffer::new(data.shape.size() as usize);
    let quads_config = RIGHT_HANDED_Y_UP_CONFIG;
    greedy_quads(
        voxels,
        &data.shape,
        [0; 3],
        data.shape.as_array().map(|x| x - 1),
        &quads_config.faces,
        &mut greedy_quads_buffer,
    );

    let islands: Vec<Island> = greedy_quads_buffer
        .quads
        .groups
        .iter()
        .enumerate()
        .flat_map(|(face_index, group)| {
            group
                .iter()
                .copied()
                .map(move |quad| Island { quad, face_index })
        })
        .collect();
    let requested_ambient_occlusion = data.settings.ambient_occlusion.and_then(|settings| {
        let intensity = sanitized_intensity(settings.intensity);
        (settings.depth > 0 && intensity > 0.0).then_some((settings.depth.min(8), intensity))
    });
    let pack_settings = match data.settings.texture_mode {
        VoxelTextureMode::Packed(mut settings) => {
            if requested_ambient_occlusion.is_some() {
                settings.padding = settings.padding.max(1);
            }
            Some(settings)
        }
        VoxelTextureMode::Palette => None,
    };
    let ambient_occlusion_settings = pack_settings.and(requested_ambient_occlusion);
    let ambient_occlusion_stencils = ambient_occlusion_settings.map(|(depth, _)| {
        quads_config
            .faces
            .iter()
            .map(|face| ambient_occlusion_stencil(face, depth))
            .collect::<Vec<_>>()
    });
    let (atlas_width, atlas_height, packed_rects) = match pack_settings {
        Some(settings) => pack_islands(
            &islands,
            settings.padding,
            settings.alignment,
            settings.max_dimension,
        )
        .expect("voxel mesh atlas exceeds the maximum texture size"),
        None => (16, 16, vec![PackedRect { x: 0, y: 0 }; islands.len()]),
    };
    let ambient_occlusion_layout = ambient_occlusion_settings.map(|_| {
        let settings = pack_settings.expect("ambient occlusion requires packed textures");
        pack_islands_with_size(
            &islands,
            settings.padding,
            settings.alignment,
            settings.max_dimension,
            |island| (island.quad.width + 1, island.quad.height + 1),
        )
        .expect("voxel ambient occlusion atlas exceeds the maximum texture size")
    });

    let offset = data.model_size() * data.settings.mesh_offset.0;
    let leading_padding = (data.padding() / 2) as f32 * data.settings.voxel_size;
    let position_offset = offset + Vec3::splat(leading_padding);
    let num_vertices = islands.len() * 4;

    let mut indices = Vec::with_capacity(islands.len() * 6);
    let mut positions = Vec::with_capacity(num_vertices);
    let mut normals = Vec::with_capacity(num_vertices);
    let mut uvs = Vec::with_capacity(num_vertices);
    let mut ambient_occlusion_uvs =
        ambient_occlusion_settings.map(|_| Vec::with_capacity(num_vertices));
    let mut palette_indices = match pack_settings {
        Some(_) => vec![0; (atlas_width * atlas_height) as usize],
        None => (0..=u8::MAX).collect(),
    };
    let mut used_palette_indices = [false; 256];
    let mut ambient_occlusion =
        ambient_occlusion_layout
            .as_ref()
            .map(|(width, height, _)| AmbientOcclusionAtlas {
                width: *width,
                height: *height,
                data: vec![u8::MAX; (*width * *height) as usize],
            });

    for (island_index, (island, packed)) in islands.iter().zip(&packed_rects).enumerate() {
        let face = &quads_config.faces[island.face_index];
        indices.extend_from_slice(&face.quad_mesh_indices(positions.len() as u32));
        positions.extend_from_slice(
            &face
                .quad_mesh_positions(&island.quad, data.settings.voxel_size)
                .map(|position| {
                    [
                        position[0] - position_offset.x,
                        position[1] - position_offset.y,
                        position[2] - position_offset.z,
                    ]
                }),
        );
        normals.extend_from_slice(&face.quad_mesh_normals());

        match pack_settings {
            Some(settings) => {
                let content_x = packed.x + settings.padding;
                let content_y = packed.y + settings.padding;
                let u0 = content_x as f32 / atlas_width as f32;
                let v0 = content_y as f32 / atlas_height as f32;
                let u1 = (content_x + island.quad.width) as f32 / atlas_width as f32;
                let v1 = (content_y + island.quad.height) as f32 / atlas_height as f32;
                uvs.extend_from_slice(&[[u0, v0], [u1, v0], [u0, v1], [u1, v1]]);
                paint_island(
                    &mut palette_indices,
                    atlas_width,
                    packed,
                    island,
                    face,
                    voxels,
                    data,
                    settings.padding,
                    &mut used_palette_indices,
                );

                if let (Some(ao), Some(ao_uvs), Some((_, _, ao_rects)), Some(stencils)) = (
                    ambient_occlusion.as_mut(),
                    ambient_occlusion_uvs.as_mut(),
                    ambient_occlusion_layout.as_ref(),
                    ambient_occlusion_stencils.as_ref(),
                ) {
                    let ao_packed = &ao_rects[island_index];
                    let ao_x = ao_packed.x + settings.padding;
                    let ao_y = ao_packed.y + settings.padding;
                    let u0 = (ao_x as f32 + 0.5) / ao.width as f32;
                    let v0 = (ao_y as f32 + 0.5) / ao.height as f32;
                    let u1 = (ao_x as f32 + island.quad.width as f32 + 0.5) / ao.width as f32;
                    let v1 = (ao_y as f32 + island.quad.height as f32 + 0.5) / ao.height as f32;
                    ao_uvs.extend_from_slice(&[[u0, v0], [u1, v0], [u0, v1], [u1, v1]]);
                    paint_ambient_occlusion_island(
                        ao,
                        ao_packed,
                        island,
                        face,
                        voxels,
                        data,
                        settings.padding,
                        &stencils[island.face_index],
                        ambient_occlusion_settings.map_or(0.0, |(_, intensity)| intensity),
                    );
                }
            }
            None => {
                let palette_index =
                    voxels[data.shape.linearize(island.quad.minimum) as usize].index;
                used_palette_indices[palette_index as usize] = true;
                let u = ((palette_index % 16) as f32 + 0.5) / 16.0;
                let v = ((palette_index / 16) as f32 + 0.5) / 16.0;
                uvs.extend_from_slice(&[[u, v]; 4]);
            }
        }
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        if data.settings.supports_remeshing {
            RenderAssetUsages::default()
        } else {
            RenderAssetUsages::RENDER_WORLD
        },
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_POSITION,
        VertexAttributeValues::Float32x3(positions),
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_NORMAL,
        VertexAttributeValues::Float32x3(normals),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, VertexAttributeValues::Float32x2(uvs));
    if let Some(uvs) = ambient_occlusion_uvs {
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_1, VertexAttributeValues::Float32x2(uvs));
    }
    mesh.insert_indices(Indices::U32(indices));

    MeshAtlas {
        mesh,
        width: atlas_width,
        height: atlas_height,
        palette_indices,
        used_palette_indices: used_palette_indices
            .iter()
            .enumerate()
            .filter_map(|(index, used)| used.then_some(index as u8))
            .collect(),
        ambient_occlusion,
    }
}

fn paint_island(
    atlas: &mut [u8],
    atlas_width: u32,
    packed: &PackedRect,
    island: &Island,
    face: &OrientedBlockFace,
    voxels: &[VisibleVoxel],
    data: &VoxelData,
    padding: u32,
    used_palette_indices: &mut [bool; 256],
) {
    let corners = face.quad_corners(&island.quad);
    let u_axis = ((corners[1] - corners[0]) / island.quad.width).to_array();
    let v_axis = ((corners[2] - corners[0]) / island.quad.height).to_array();

    for padded_y in 0..island.quad.height + 2 * padding {
        let source_y = padded_y.saturating_sub(padding).min(island.quad.height - 1);
        for padded_x in 0..island.quad.width + 2 * padding {
            let source_x = padded_x.saturating_sub(padding).min(island.quad.width - 1);
            let voxel_position = std::array::from_fn(|axis| {
                island.quad.minimum[axis] + u_axis[axis] * source_x + v_axis[axis] * source_y
            });
            let palette_index = voxels[data.shape.linearize(voxel_position) as usize].index;
            used_palette_indices[palette_index as usize] = true;
            let atlas_index = ((packed.y + padded_y) * atlas_width + packed.x + padded_x) as usize;
            atlas[atlas_index] = palette_index;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_ambient_occlusion_island(
    atlas: &mut AmbientOcclusionAtlas,
    packed: &PackedRect,
    island: &Island,
    face: &OrientedBlockFace,
    voxels: &[VisibleVoxel],
    data: &VoxelData,
    padding: u32,
    stencil: &[AmbientOcclusionSample],
    intensity: f32,
) {
    let corners = face.quad_corners(&island.quad);
    let u_axis = ((corners[1] - corners[0]) / island.quad.width)
        .as_ivec3()
        .to_array();
    let v_axis = ((corners[2] - corners[0]) / island.quad.height)
        .as_ivec3()
        .to_array();
    let origin = corners[0].as_ivec3().to_array();
    let width = island.quad.width + 1;
    let height = island.quad.height + 1;

    for padded_y in 0..height + 2 * padding {
        let source_y = padded_y.saturating_sub(padding).min(height - 1) as i32;
        for padded_x in 0..width + 2 * padding {
            let source_x = padded_x.saturating_sub(padding).min(width - 1) as i32;
            let corner = std::array::from_fn(|axis| {
                origin[axis] + u_axis[axis] * source_x + v_axis[axis] * source_y
            });
            let atlas_index = ((packed.y + padded_y) * atlas.width + packed.x + padded_x) as usize;
            atlas.data[atlas_index] =
                ambient_occlusion_at(corner, stencil, intensity, voxels, data);
        }
    }
}

fn sanitized_intensity(intensity: f32) -> f32 {
    if intensity.is_finite() {
        intensity.max(0.0)
    } else {
        0.0
    }
}

fn ambient_occlusion_stencil(
    face: &OrientedBlockFace,
    depth: usize,
) -> Vec<AmbientOcclusionSample> {
    const COVERAGE_SAMPLES_PER_AXIS: i32 = 4;

    let unit_quad = UnorientedQuad {
        minimum: [0; 3],
        width: 1,
        height: 1,
    };
    let corners = face.quad_corners(&unit_quad);
    let u_axis = (corners[1] - corners[0]).as_ivec3().to_array();
    let v_axis = (corners[2] - corners[0]).as_ivec3().to_array();
    let normal = face.signed_normal().to_array();
    let mut samples = Vec::new();

    for layer in 1..=depth as i32 {
        for v in -layer..layer {
            for u in -layer..layer {
                let mut weight = 0.0;
                for sample_v in 0..COVERAGE_SAMPLES_PER_AXIS {
                    for sample_u in 0..COVERAGE_SAMPLES_PER_AXIS {
                        let sample_u =
                            u as f32 + (sample_u as f32 + 0.5) / COVERAGE_SAMPLES_PER_AXIS as f32;
                        let sample_v =
                            v as f32 + (sample_v as f32 + 0.5) / COVERAGE_SAMPLES_PER_AXIS as f32;
                        let radial_distance_squared = sample_u * sample_u + sample_v * sample_v;
                        if radial_distance_squared <= (layer * layer) as f32 {
                            let normal_distance = layer as f32 - 0.5;
                            weight += (normal_distance * normal_distance + radial_distance_squared)
                                .sqrt()
                                .recip();
                        }
                    }
                }
                weight /= (COVERAGE_SAMPLES_PER_AXIS * COVERAGE_SAMPLES_PER_AXIS) as f32;
                if weight == 0.0 {
                    continue;
                }
                let offset = std::array::from_fn(|axis| {
                    normal[axis] * layer - normal[axis].max(0)
                        + u_axis[axis] * u
                        + u_axis[axis].min(0)
                        + v_axis[axis] * v
                        + v_axis[axis].min(0)
                });
                samples.push(AmbientOcclusionSample { offset, weight });
            }
        }
    }
    samples
}

fn ambient_occlusion_at(
    sample_position: [i32; 3],
    stencil: &[AmbientOcclusionSample],
    intensity: f32,
    voxels: &[VisibleVoxel],
    data: &VoxelData,
) -> u8 {
    let shape = data.shape.as_array();
    let mut occupied_weight = 0.0;
    let mut total_weight = 0.0;

    for sample in stencil {
        total_weight += sample.weight;
        let position = std::array::from_fn(|axis| sample_position[axis] + sample.offset[axis]);
        if position
            .iter()
            .enumerate()
            .any(|(axis, coordinate)| *coordinate < 0 || *coordinate >= shape[axis] as i32)
        {
            continue;
        }
        let position = position.map(|coordinate| coordinate as u32);
        if voxels[data.shape.linearize(position) as usize].visibility != VoxelVisibility::Empty {
            occupied_weight += sample.weight;
        }
    }

    let occlusion = if total_weight > 0.0 {
        (occupied_weight / total_weight * intensity).clamp(0.0, 1.0)
    } else {
        0.0
    };
    ((1.0 - occlusion) * u8::MAX as f32).round() as u8
}

fn pack_islands(
    islands: &[Island],
    padding: u32,
    alignment: u32,
    max_dimension: u32,
) -> Option<(u32, u32, Vec<PackedRect>)> {
    pack_islands_with_size(islands, padding, alignment, max_dimension, |island| {
        (island.quad.width, island.quad.height)
    })
}

fn pack_islands_with_size(
    islands: &[Island],
    padding: u32,
    alignment: u32,
    max_dimension: u32,
    size: impl Fn(&Island) -> (u32, u32),
) -> Option<(u32, u32, Vec<PackedRect>)> {
    let alignment = alignment.max(1);
    let mut rectangles: GroupedRectsToPlace<usize, ()> = GroupedRectsToPlace::new();
    let mut total_area = 0_u64;
    let mut minimum_width = 1;
    let mut minimum_height = 1;
    for (id, island) in islands.iter().enumerate() {
        let padded = padding.checked_mul(2)?;
        let (content_width, content_height) = size(island);
        let width = content_width.checked_add(padded)?;
        let height = content_height.checked_add(padded)?;
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
        *width = align((*width).max(minimum_width), alignment).min(max_dimension);
    }
    widths.sort_unstable();
    widths.dedup();

    let mut best: Option<(u64, u32, u32, Vec<PackedRect>)> = None;
    for width in widths {
        if width < minimum_width {
            continue;
        }
        let area_height = total_area.div_ceil(u64::from(width)) as u32;
        let mut height = align(area_height.max(minimum_height), alignment);
        while height <= max_dimension {
            let mut bins = BTreeMap::from([(0_u8, TargetBin::new(width, height, 1))]);
            if let Ok(result) = pack_rects(
                &rectangles,
                &mut bins,
                &volume_heuristic,
                &contains_smallest_box,
            ) {
                let mut packed = vec![PackedRect { x: 0, y: 0 }; islands.len()];
                let mut used_width = 1;
                let mut used_height = 1;
                for (&id, (_, location)) in result.packed_locations() {
                    packed[id] = PackedRect {
                        x: location.x(),
                        y: location.y(),
                    };
                    used_width = used_width.max(location.x() + location.width());
                    used_height = used_height.max(location.y() + location.height());
                }
                used_width = align(used_width, alignment);
                used_height = align(used_height, alignment);
                let area = u64::from(used_width) * u64::from(used_height);
                let aspect = used_width.abs_diff(used_height);
                let replace = best.as_ref().is_none_or(|(best_area, best_w, best_h, _)| {
                    area < *best_area || (area == *best_area && aspect < best_w.abs_diff(*best_h))
                });
                if replace {
                    best = Some((area, used_width, used_height, packed));
                }
                break;
            }
            if height == max_dimension {
                break;
            }
            height = align(
                (height.saturating_mul(3) / 2).max(height.saturating_add(alignment)),
                alignment,
            )
            .min(max_dimension);
        }
    }

    best.map(|(_, width, height, packed)| (width, height, packed))
}

fn align(value: u32, alignment: u32) -> u32 {
    value.div_ceil(alignment).saturating_mul(alignment)
}

#[cfg(test)]
mod tests {
    use super::{Island, ambient_occlusion_stencil, mesh_model, pack_islands};
    use crate::{AmbientOcclusionSettings, VoxLoaderSettings, VoxelTextureMode, model::RawVoxel};
    use bevy::{math::UVec3, mesh::Mesh};
    use block_mesh::{RIGHT_HANDED_Y_UP_CONFIG, UnorientedQuad};
    use ndshape::Shape;

    #[test]
    fn packs_rectangular_islands_deterministically() {
        let islands = [
            Island {
                quad: UnorientedQuad {
                    minimum: [0; 3],
                    width: 10,
                    height: 2,
                },
                face_index: 0,
            },
            Island {
                quad: UnorientedQuad {
                    minimum: [0; 3],
                    width: 3,
                    height: 7,
                },
                face_index: 1,
            },
        ];
        let padding = 1;
        let first = pack_islands(&islands, padding, 4, 8192).expect("islands fit");
        let second = pack_islands(&islands, padding, 4, 8192).expect("islands fit");
        assert_eq!((first.0, first.1), (second.0, second.1));
        assert_eq!(first.2, second.2);
        assert!(first.0 >= 10 + 2 * padding);
        assert!(first.1 >= 2 + 2 * padding);
    }

    #[test]
    fn packed_mode_merges_faces_across_palette_indices() {
        let packed = two_color_model(VoxelTextureMode::default());
        let palette = two_color_model(VoxelTextureMode::Palette);

        assert_eq!(packed.mesh.count_vertices(), 6 * 4);
        assert_eq!(palette.mesh.count_vertices(), 10 * 4);
        assert_eq!(packed.used_palette_indices, vec![0, 1]);
        assert!(packed.palette_indices.contains(&0));
        assert!(packed.palette_indices.contains(&1));
        assert_eq!((palette.width, palette.height), (16, 16));
    }

    #[test]
    fn ambient_occlusion_uses_distance_falloff() {
        let stencil = ambient_occlusion_stencil(&RIGHT_HANDED_Y_UP_CONFIG.faces[0], 2);
        let minimum = stencil
            .iter()
            .map(|sample| sample.weight)
            .reduce(f32::min)
            .expect("stencil sample");
        let maximum = stencil
            .iter()
            .map(|sample| sample.weight)
            .reduce(f32::max)
            .expect("stencil sample");
        assert!(maximum > minimum);
    }

    #[test]
    fn ambient_occlusion_samples_the_four_cells_around_a_corner_symmetrically() {
        for face in &RIGHT_HANDED_Y_UP_CONFIG.faces {
            let stencil = ambient_occlusion_stencil(face, 1);
            assert_eq!(stencil.len(), 4);
            let expected = stencil[0].weight;
            assert!(
                stencil
                    .iter()
                    .all(|sample| (sample.weight - expected).abs() < f32::EPSILON)
            );
        }
    }

    #[test]
    fn ambient_occlusion_darkens_concave_geometry_only_in_packed_mode() {
        let ambient_occlusion = Some(AmbientOcclusionSettings {
            depth: 1,
            intensity: 1.0,
        });
        let packed = model_with_voxels(
            UVec3::new(2, 2, 1),
            &[(UVec3::new(0, 0, 0), 0), (UVec3::new(1, 1, 0), 0)],
            VoxLoaderSettings {
                ambient_occlusion,
                ..Default::default()
            },
        );
        let palette = model_with_voxels(
            UVec3::new(2, 2, 1),
            &[(UVec3::new(0, 0, 0), 0), (UVec3::new(1, 1, 0), 0)],
            VoxLoaderSettings {
                texture_mode: VoxelTextureMode::Palette,
                ambient_occlusion,
                ..Default::default()
            },
        );

        assert!(packed.mesh.attribute(Mesh::ATTRIBUTE_UV_1).is_some());
        let packed_ao = packed.ambient_occlusion.expect("packed AO texture");
        assert!(packed_ao.data.contains(&u8::MAX));
        assert!(packed_ao.data.iter().any(|value| *value < u8::MAX));
        assert!(palette.ambient_occlusion.is_none());
    }

    #[test]
    fn ambient_occlusion_settings_are_sanitized() {
        let build = |settings| {
            model_with_voxels(
                UVec3::ONE,
                &[(UVec3::ZERO, 0)],
                VoxLoaderSettings {
                    ambient_occlusion: Some(settings),
                    ..Default::default()
                },
            )
        };

        assert!(
            build(AmbientOcclusionSettings {
                depth: 0,
                intensity: 1.0,
            })
            .ambient_occlusion
            .is_none()
        );
        assert!(
            build(AmbientOcclusionSettings {
                depth: 1,
                intensity: -1.0,
            })
            .ambient_occlusion
            .is_none()
        );
        assert!(
            build(AmbientOcclusionSettings {
                depth: 1,
                intensity: f32::NAN,
            })
            .ambient_occlusion
            .is_none()
        );
        assert_eq!(
            build(AmbientOcclusionSettings {
                depth: 8,
                intensity: 1.0,
            })
            .ambient_occlusion,
            build(AmbientOcclusionSettings {
                depth: 9,
                intensity: 1.0,
            })
            .ambient_occlusion
        );
    }

    fn two_color_model(texture_mode: VoxelTextureMode) -> super::MeshAtlas {
        let settings = VoxLoaderSettings {
            texture_mode,
            ..Default::default()
        };
        let mut data = super::VoxelData::new(UVec3::new(2, 1, 1), settings);
        for (x, palette_index) in [(1, 0), (2, 1)] {
            let index = data.shape.linearize([x, 1, 1]) as usize;
            data.voxels[index] = RawVoxel(palette_index);
        }
        let empty_properties = vec![None; 256];
        let (visible, _, _) = data.visible_voxels(&empty_properties, &empty_properties);
        mesh_model(&visible, &data)
    }

    fn model_with_voxels(
        size: UVec3,
        voxels: &[(UVec3, u8)],
        settings: VoxLoaderSettings,
    ) -> super::MeshAtlas {
        let mut data = super::VoxelData::new(size, settings);
        let padding = UVec3::splat(data.padding() / 2);
        for (position, palette_index) in voxels {
            let index = data.shape.linearize((*position + padding).into()) as usize;
            data.voxels[index] = RawVoxel(*palette_index);
        }
        let empty_properties = vec![None; 256];
        let (visible, _, _) = data.visible_voxels(&empty_properties, &empty_properties);
        mesh_model(&visible, &data)
    }
}
