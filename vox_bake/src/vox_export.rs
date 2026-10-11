use std::{fmt, io};

use dot_vox::{
    Color, DEFAULT_INDEX_MAP, Dict, DotVoxData, Frame, Layer, Material, Model, SceneNode,
    ShapeModel, Size, Voxel,
};

use crate::{BakedMaterial, BakedPalette, RawVoxel, VoxelGrid};

/// An error encountered while exporting a voxel grid to MagicaVoxel format.
#[derive(Debug)]
pub enum VoxWriteError {
    /// A generated animation contained no frames.
    EmptyAnimation,
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
    export_animation_to_vox(std::iter::once(grid), palette)
}

/// Export engine-independent voxel grids as a MagicaVoxel shape animation.
///
/// Every grid becomes one model frame, in iterator order. Frame indices are written to the shape
/// node; playback rate is not represented by the VOX format.
pub fn export_animation_to_vox<'a>(
    grids: impl IntoIterator<Item = &'a VoxelGrid>,
    palette: &BakedPalette,
) -> Result<Vec<u8>, VoxWriteError> {
    let models = grids
        .into_iter()
        .map(vox_model)
        .collect::<Result<Vec<_>, _>>()?;
    if models.is_empty() {
        return Err(VoxWriteError::EmptyAnimation);
    }
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
                layer_id: 0,
            },
            SceneNode::Shape {
                attributes: Dict::new(),
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

fn vox_model(grid: &VoxelGrid) -> Result<Model, VoxWriteError> {
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

    let [size_x, size_y, size_z] = grid.model_dimensions;
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
                voxels.push(Voxel {
                    x: (size_x - 1 - x) as u8,
                    y: z as u8,
                    z: y as u8,
                    i: voxel.0,
                });
            }
        }
    }

    Ok(Model {
        size: Size {
            x: size_x,
            y: size_z,
            z: size_y,
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
    use super::{VoxWriteError, export_animation_to_vox, export_model_to_vox};
    use crate::{BakedMaterial, BakedPalette, RawVoxel, VoxelGrid};

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
        assert_eq!(decoded.scenes.len(), 2);
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

        let bytes = export_animation_to_vox([&first, &second], &palette).unwrap();
        let decoded = dot_vox::load_bytes(&bytes).unwrap();
        let dot_vox::SceneNode::Shape { models, .. } = &decoded.scenes[1] else {
            panic!("second scene node should be the animated shape");
        };

        assert_eq!(decoded.models.len(), 2);
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].frame_index(), Some(0));
        assert_eq!(models[1].frame_index(), Some(1));
    }

    #[test]
    fn rejects_empty_animations() {
        let palette = BakedPalette::new(Vec::new(), true);
        assert!(matches!(
            export_animation_to_vox(std::iter::empty(), &palette),
            Err(VoxWriteError::EmptyAnimation)
        ));
    }
}
