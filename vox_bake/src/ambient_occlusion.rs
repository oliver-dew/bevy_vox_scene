use block_mesh::{RIGHT_HANDED_Y_UP_CONFIG, UnorientedQuad};

use crate::{AtlasRect, BakedImage, BakedQuad, VisibleVoxel, VoxelVisibility, paint::linearize};

/// One weighted voxel-residency sample in an ambient-occlusion stencil.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AmbientOcclusionSample {
    /// Offset from the sampled surface corner.
    pub offset: [i32; 3],
    /// Distance-weighted contribution to occlusion.
    pub weight: f32,
}

/// Return the signed unit normal for a shared Y-up face index.
pub fn face_normal(face_index: usize) -> [i32; 3] {
    RIGHT_HANDED_Y_UP_CONFIG.faces[face_index]
        .signed_normal()
        .to_array()
}

/// Clamp an AO intensity to the supported non-negative finite range.
pub fn sanitized_ambient_occlusion_intensity(intensity: f32) -> f32 {
    if intensity.is_finite() {
        intensity.max(0.0)
    } else {
        0.0
    }
}

/// Build the distance-weighted sampling stencil for one oriented block face.
pub fn ambient_occlusion_stencil(face_index: usize, depth: usize) -> Vec<AmbientOcclusionSample> {
    const COVERAGE_SAMPLES_PER_AXIS: i32 = 4;
    let face = &RIGHT_HANDED_Y_UP_CONFIG.faces[face_index];
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
                        let radial_squared = sample_u * sample_u + sample_v * sample_v;
                        if radial_squared <= (layer * layer) as f32 {
                            let normal_distance = layer as f32 - 0.5;
                            weight += (normal_distance * normal_distance + radial_squared)
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

/// Evaluate voxel-residency AO at one surface corner.
pub fn ambient_occlusion_at(
    sample_position: [i32; 3],
    stencil: &[AmbientOcclusionSample],
    intensity: f32,
    voxels: &[VisibleVoxel],
    dimensions: [u32; 3],
) -> u8 {
    let mut occupied_weight = 0.0;
    let mut total_weight = 0.0;
    for sample in stencil {
        total_weight += sample.weight;
        let position = std::array::from_fn(|axis| sample_position[axis] + sample.offset[axis]);
        if position
            .iter()
            .enumerate()
            .any(|(axis, coordinate)| *coordinate < 0 || *coordinate >= dimensions[axis] as i32)
        {
            continue;
        }
        let position = position.map(|coordinate| coordinate as u32);
        if voxels[linearize(position, dimensions)].visibility != VoxelVisibility::Empty {
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

/// Paint AO values for one packed quad, extending edge samples into padding.
#[allow(clippy::too_many_arguments)]
pub fn paint_ambient_occlusion_island(
    atlas: &mut BakedImage,
    packed: AtlasRect,
    quad: &BakedQuad,
    voxels: &[VisibleVoxel],
    dimensions: [u32; 3],
    padding: u32,
    stencil: &[AmbientOcclusionSample],
    intensity: f32,
) {
    let face = &RIGHT_HANDED_Y_UP_CONFIG.faces[quad.face_index];
    let block_quad = UnorientedQuad {
        minimum: quad.minimum,
        width: quad.width,
        height: quad.height,
    };
    let corners = face.quad_corners(&block_quad);
    let u_axis = ((corners[1] - corners[0]) / quad.width)
        .as_ivec3()
        .to_array();
    let v_axis = ((corners[2] - corners[0]) / quad.height)
        .as_ivec3()
        .to_array();
    let origin = corners[0].as_ivec3().to_array();
    let width = quad.width + 1;
    let height = quad.height + 1;
    for padded_y in 0..height + 2 * padding {
        let source_y = padded_y.saturating_sub(padding).min(height - 1) as i32;
        for padded_x in 0..width + 2 * padding {
            let source_x = padded_x.saturating_sub(padding).min(width - 1) as i32;
            let corner = std::array::from_fn(|axis| {
                origin[axis] + u_axis[axis] * source_x + v_axis[axis] * source_y
            });
            let index = ((packed.y + padded_y) * atlas.width + packed.x + padded_x) as usize;
            atlas.pixels[index] =
                ambient_occlusion_at(corner, stencil, intensity, voxels, dimensions);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stencil_is_symmetric_and_distance_weighted() {
        for face_index in 0..6 {
            let nearest = ambient_occlusion_stencil(face_index, 1);
            assert_eq!(nearest.len(), 4);
            assert!(
                nearest
                    .iter()
                    .all(|sample| { (sample.weight - nearest[0].weight).abs() < f32::EPSILON })
            );

            let deeper = ambient_occlusion_stencil(face_index, 2);
            let minimum = deeper
                .iter()
                .map(|sample| sample.weight)
                .reduce(f32::min)
                .unwrap();
            let maximum = deeper
                .iter()
                .map(|sample| sample.weight)
                .reduce(f32::max)
                .unwrap();
            assert!(maximum > minimum);
        }
    }

    #[test]
    fn intensity_sanitization_rejects_negative_and_non_finite_values() {
        assert_eq!(sanitized_ambient_occlusion_intensity(-1.0), 0.0);
        assert_eq!(sanitized_ambient_occlusion_intensity(f32::NAN), 0.0);
        assert_eq!(sanitized_ambient_occlusion_intensity(1.5), 1.5);
    }
}
