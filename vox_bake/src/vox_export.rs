use std::{fmt, io};

use dot_vox::{
    Color, DEFAULT_INDEX_MAP, Dict, DotVoxData, Frame, Layer, Material, Model, SceneNode,
    ShapeModel, Size, Voxel,
};

use crate::{BakedMaterial, BakedPalette, RawVoxel, UnitOffset, VoxelGrid};

/// An error encountered while exporting a voxel grid to MagicaVoxel format.
#[derive(Debug)]
pub enum VoxWriteError {
    /// A generated animation contained no frames.
    EmptyAnimation,
    /// A generated animation frame uses settings incompatible with the first frame.
    AnimationSettingsMismatch {
        /// Zero-based index of the incompatible frame.
        frame: usize,
    },
    /// MagicaVoxel stores each voxel coordinate in one byte.
    DimensionTooLarge {
        /// Axis in the engine-independent XYZ grid.
        axis: usize,
        /// Requested model dimension.
        size: u32,
    },
    /// The grid's dimensions and voxel storage are inconsistent.
    InvalidGrid,
    /// Serializing the VOX data failed.
    Io(io::Error),
}

impl fmt::Display for VoxWriteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyAnimation => {
                formatter.write_str("voxel animation requires at least one frame")
            }
            Self::AnimationSettingsMismatch { frame } => write!(
                formatter,
                "voxel animation frame {frame} uses incompatible settings"
            ),
            Self::DimensionTooLarge { axis, size } => write!(
                formatter,
                "voxel model dimension {size} on axis {axis} exceeds the VOX limit of 256"
            ),
            Self::InvalidGrid => formatter.write_str("voxel grid storage is inconsistent"),
            Self::Io(error) => write!(formatter, "could not write VOX data: {error}"),
        }
    }
}

impl std::error::Error for VoxWriteError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for VoxWriteError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Export one engine-independent voxel grid and palette as MagicaVoxel bytes.
///
/// Meshing padding is omitted, coordinates are converted from right-handed Y-up to VOX's
/// right-handed Z-up convention, and render-only bake settings are not represented.
pub fn export_model_to_vox(
    grid: &VoxelGrid,
    palette: &BakedPalette,
) -> Result<Vec<u8>, VoxWriteError> {
    export_animation_to_vox_with_offset(std::iter::once(grid), palette, &UnitOffset::ZERO)
}

/// Export engine-independent voxel grids as a MagicaVoxel shape animation.
///
/// Frames are aligned around their centers. Use [`export_animation_to_vox_with_offset`] to select
/// another alignment.
pub fn export_animation_to_vox<'a>(
    grids: impl IntoIterator<Item = &'a VoxelGrid>,
    palette: &BakedPalette,
) -> Result<Vec<u8>, VoxWriteError> {
    export_animation_to_vox_with_offset(grids, palette, &UnitOffset::CENTER)
}

/// Export engine-independent voxel grids as a MagicaVoxel shape animation with an explicit origin.
///
/// Every grid becomes one model frame, in iterator order. Frames are placed in a common-sized
/// voxel canvas using `mesh_offset`, because MagicaVoxel shape animation frames share one
/// transform. Frame indices are written to the shape node; playback rate is not represented by
/// the VOX format.
pub fn export_animation_to_vox_with_offset<'a>(
    grids: impl IntoIterator<Item = &'a VoxelGrid>,
    palette: &BakedPalette,
    mesh_offset: &UnitOffset,
) -> Result<Vec<u8>, VoxWriteError> {
    let grids = grids.into_iter().collect::<Vec<_>>();
    if grids.is_empty() {
        return Err(VoxWriteError::EmptyAnimation);
    }
    let common_dimensions = std::array::from_fn(|axis| {
        grids
            .iter()
            .map(|grid| grid.model_dimensions[axis])
            .max()
            .unwrap_or_default()
    });
    let models = grids
        .into_iter()
        .map(|grid| vox_model_in_canvas(grid, common_dimensions, mesh_offset))
        .collect::<Result<Vec<_>, _>>()?;
    let animated = models.len() > 1;
    let shape_models = (0..models.len())
        .map(|index| {
            let mut attributes = Dict::new();
            if animated {
                attributes.insert("_f".into(), index.to_string());
            }
            ShapeModel {
                model_id: index as u32,
                attributes,
            }
        })
        .collect();
    let data = DotVoxData {
        version: 200,
        index_map: DEFAULT_INDEX_MAP.to_vec(),
        models,
        palette: palette
            .materials
            .iter()
            .map(|material| vox_color(material, palette.uses_srgb))
            .collect(),
        materials: palette
            .materials
            .iter()
            .enumerate()
            .map(|(index, material)| vox_material(index, material))
            .collect(),
        scenes: vec![
            SceneNode::Transform {
                attributes: Dict::new(),
                frames: vec![Frame::default()],
                child: 1,
                layer_id: u32::MAX,
            },
            SceneNode::Group {
                attributes: Dict::new(),
                children: vec![2],
            },
            SceneNode::Transform {
                attributes: Dict::new(),
                frames: vec![Frame::new(Dict::from([("_t".into(), "0 0 0".into())]))],
                child: 3,
                layer_id: 0,
            },
            SceneNode::Shape {
                attributes: Dict::from([("_frm_id".into(), "0".into())]),
                models: shape_models,
            },
        ],
        layers: vec![Layer {
            attributes: Dict::new(),
        }],
    };
    let mut bytes = Vec::new();
    data.write_vox(&mut bytes)?;
    Ok(bytes)
}

fn vox_model_in_canvas(
    grid: &VoxelGrid,
    canvas_dimensions: [u32; 3],
    mesh_offset: &UnitOffset,
) -> Result<Model, VoxWriteError> {
    for (axis, size) in grid.model_dimensions.into_iter().enumerate() {
        if size > 256 {
            return Err(VoxWriteError::DimensionTooLarge { axis, size });
        }
        if size.checked_add(grid.padding) != Some(grid.dimensions[axis]) {
            return Err(VoxWriteError::InvalidGrid);
        }
    }
    let expected_len = grid
        .dimensions
        .iter()
        .try_fold(1_usize, |length, size| length.checked_mul(*size as usize));
    if expected_len != Some(grid.voxels.len()) {
        return Err(VoxWriteError::InvalidGrid);
    }

    for (axis, size) in canvas_dimensions.into_iter().enumerate() {
        if size > 256 {
            return Err(VoxWriteError::DimensionTooLarge { axis, size });
        }
    }

    let [size_x, size_y, size_z] = grid.model_dimensions;
    let [canvas_x, canvas_y, canvas_z] = canvas_dimensions;
    let alignment = mesh_offset.to_array();
    let canvas_offset: [u32; 3] = std::array::from_fn(|axis| {
        let available = canvas_dimensions[axis] - grid.model_dimensions[axis];
        let proportion = if alignment[axis].is_finite() {
            alignment[axis].clamp(0.0, 1.0)
        } else {
            0.0
        };
        (available as f32 * proportion).round() as u32
    });
    let leading_padding = grid.padding / 2;
    let mut voxels = Vec::new();
    for x in 0..size_x {
        for y in 0..size_y {
            for z in 0..size_z {
                let source = [
                    x + leading_padding,
                    y + leading_padding,
                    z + leading_padding,
                ];
                let Some(voxel) = grid.voxels.get(grid.linearize(source)) else {
                    return Err(VoxWriteError::InvalidGrid);
                };
                if *voxel == RawVoxel::EMPTY {
                    continue;
                }
                let destination = [
                    x + canvas_offset[0],
                    y + canvas_offset[1],
                    z + canvas_offset[2],
                ];
                voxels.push(Voxel {
                    x: (canvas_x - 1 - destination[0]) as u8,
                    y: destination[2] as u8,
                    z: destination[1] as u8,
                    i: voxel.0,
                });
            }
        }
    }

    Ok(Model {
        size: Size {
            x: canvas_x,
            y: canvas_z,
            z: canvas_y,
        },
        voxels,
    })
}

fn vox_color(material: &BakedMaterial, uses_srgb: bool) -> Color {
    let [r, g, b, a] = material.color;
    let encode = |value: f32| {
        let value = if uses_srgb {
            value
        } else {
            linear_to_srgb(value)
        };
        (value.clamp(0.0, 1.0) * 255.0).round() as u8
    };
    Color {
        r: encode(r),
        g: encode(g),
        b: encode(b),
        a: (a.clamp(0.0, 1.0) * 255.0).round() as u8,
    }
}

fn linear_to_srgb(value: f32) -> f32 {
    if value <= 0.003_130_8 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    }
}

fn vox_material(index: usize, material: &BakedMaterial) -> Material {
    let mut properties = Dict::new();
    let material_type = if material.density > 0.0 {
        properties.insert("_d".into(), (material.density / 10.0).to_string());
        "_media"
    } else if material.translucency > 0.0 {
        properties.insert("_trans".into(), material.translucency.to_string());
        properties.insert("_ri".into(), material.refraction_index.to_string());
        "_glass"
    } else if material.emission > 0.0 {
        properties.insert("_emit".into(), material.emission.to_string());
        "_emit"
    } else if material.metalness > 0.0 {
        properties.insert("_metal".into(), material.metalness.to_string());
        "_metal"
    } else {
        "_diffuse"
    };
    properties.insert("_type".into(), material_type.into());
    properties.insert("_rough".into(), material.roughness.to_string());
    Material {
        id: index as u32 + 1,
        properties,
    }
}

#[cfg(test)]
mod tests {
    use dot_vox::{Size, Voxel};

    use super::{VoxWriteError, export_animation_to_vox_with_offset, export_model_to_vox};
    use crate::{BakedMaterial, BakedPalette, RawVoxel, UnitOffset, VoxelGrid};

    #[test]
    fn exported_model_round_trips_coordinates_and_palette_indices() {
        let mut grid = VoxelGrid::empty([3, 2, 4], true);
        for (position, voxel) in [([0, 0, 0], RawVoxel(2)), ([2, 1, 3], RawVoxel(7))] {
            let padded = position.map(|component| component + 1);
            let index = grid.linearize(padded);
            grid.voxels[index] = voxel;
        }
        let mut materials = vec![BakedMaterial::default(); 256];
        materials[2].color = [1.0, 0.0, 0.0, 1.0];
        materials[2].roughness = 0.25;
        materials[2].translucency = 0.6;
        materials[2].refraction_index = 1.33;
        materials[7].color = [0.0, 1.0, 0.0, 1.0];
        let palette = BakedPalette::new(materials, false);

        let bytes = export_model_to_vox(&grid, &palette).unwrap();
        let decoded = dot_vox::load_bytes(&bytes).unwrap();
        let round_trip = VoxelGrid::from_vox_model(&decoded.models[0], true);

        assert_eq!(round_trip, grid);
        assert_eq!(
            decoded.palette[2],
            dot_vox::Color {
                r: 255,
                g: 0,
                b: 0,
                a: 255
            }
        );
        assert_eq!(
            decoded.palette[7],
            dot_vox::Color {
                r: 0,
                g: 255,
                b: 0,
                a: 255
            }
        );
        let glass = decoded
            .materials
            .iter()
            .find(|material| material.id == 3)
            .unwrap();
        assert_eq!(glass.material_type(), Some("_glass"));
        assert_eq!(glass.roughness(), Some(0.25));
        assert_eq!(glass.transparency(), Some(0.6));
        assert_eq!(decoded.scenes.len(), 4);
    }

    #[test]
    fn rejects_dimensions_that_do_not_fit_vox_coordinates() {
        let grid = VoxelGrid::empty([257, 1, 1], false);
        let palette = BakedPalette::new(Vec::new(), true);

        assert!(matches!(
            export_model_to_vox(&grid, &palette),
            Err(VoxWriteError::DimensionTooLarge { axis: 0, size: 257 })
        ));
    }

    #[test]
    fn exported_animation_round_trips_frame_indices() {
        let first = VoxelGrid::empty([1; 3], true);
        let second = VoxelGrid::empty([2, 1, 1], true);
        let palette = BakedPalette::new(Vec::new(), true);

        let bytes =
            export_animation_to_vox_with_offset([&first, &second], &palette, &UnitOffset::CENTER)
                .unwrap();
        let decoded = dot_vox::load_bytes(&bytes).unwrap();
        let dot_vox::SceneNode::Transform {
            child, layer_id, ..
        } = &decoded.scenes[0]
        else {
            panic!("first scene node should be the root transform");
        };
        assert_eq!((*child, *layer_id), (1, u32::MAX));
        let dot_vox::SceneNode::Group { children, .. } = &decoded.scenes[1] else {
            panic!("second scene node should be the root group");
        };
        assert_eq!(children, &[2]);
        let dot_vox::SceneNode::Transform { child, frames, .. } = &decoded.scenes[2] else {
            panic!("third scene node should be the model transform");
        };
        assert_eq!(*child, 3);
        assert_eq!(frames[0].position(), Some((0, 0, 0).into()));
        let dot_vox::SceneNode::Shape { attributes, models } = &decoded.scenes[3] else {
            panic!("fourth scene node should be the animated shape");
        };

        assert_eq!(decoded.models.len(), 2);
        assert!(decoded.models.iter().all(|model| model.size.x == 2));
        assert_eq!(attributes.get("_frm_id").map(String::as_str), Some("0"));
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].frame_index(), Some(0));
        assert_eq!(models[1].frame_index(), Some(1));
    }

    #[test]
    fn rejects_empty_animations() {
        let palette = BakedPalette::new(Vec::new(), true);
        assert!(matches!(
            export_animation_to_vox_with_offset(std::iter::empty(), &palette, &UnitOffset::CENTER),
            Err(VoxWriteError::EmptyAnimation)
        ));
    }

    #[test]
    fn animation_frames_share_a_centered_canvas() {
        let mut small = VoxelGrid::empty([1, 1, 1], false);
        small.voxels[0] = RawVoxel(1);
        let mut large = VoxelGrid::empty([3, 3, 3], false);
        let large_origin = large.linearize([0, 0, 0]);
        large.voxels[large_origin] = RawVoxel(1);
        let palette = BakedPalette::new(Vec::new(), true);

        let bytes =
            export_animation_to_vox_with_offset([&small, &large], &palette, &UnitOffset::CENTER)
                .unwrap();
        let decoded = dot_vox::load_bytes(&bytes).unwrap();

        assert_eq!(decoded.models[0].size, Size { x: 3, y: 3, z: 3 });
        assert_eq!(decoded.models[1].size, Size { x: 3, y: 3, z: 3 });
        assert_eq!(
            decoded.models[0].voxels[0],
            Voxel {
                x: 1,
                y: 1,
                z: 1,
                i: 1,
            }
        );
    }
}
