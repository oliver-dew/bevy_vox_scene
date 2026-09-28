use dot_vox::DotVoxData;
use serde::{Deserialize, Serialize};

/// Engine-independent physical properties for one voxel palette entry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BakedMaterial {
    /// RGBA components in the color space selected by [`BakedPalette::uses_srgb`].
    pub color: [f32; 4],
    /// Emissive multiplier applied to the base color.
    pub emission: f32,
    /// Perceptual roughness on a scale from zero to one.
    pub roughness: f32,
    /// Metalness on a scale from zero to one.
    pub metalness: f32,
    /// Transmission amount on a scale from zero to one.
    pub translucency: f32,
    /// Index of refraction used when translucency is non-zero.
    pub refraction_index: f32,
    /// Density used for cloud-volume materials.
    pub density: f32,
}

impl Default for BakedMaterial {
    fn default() -> Self {
        Self {
            color: [1.0, 0.0, 0.0, 1.0],
            emission: 0.0,
            roughness: 0.5,
            metalness: 0.0,
            translucency: 0.0,
            refraction_index: 1.5,
            density: 0.0,
        }
    }
}

impl BakedMaterial {
    /// Return linear RGBA components regardless of the palette's source color space.
    pub fn linear_rgba(&self, uses_srgb: bool) -> [f32; 4] {
        if uses_srgb {
            [
                srgb_to_linear(self.color[0]),
                srgb_to_linear(self.color[1]),
                srgb_to_linear(self.color[2]),
                self.color[3],
            ]
        } else {
            self.color
        }
    }

    /// Encode this material's color as 8-bit sRGBA.
    pub fn srgba_u8(&self, uses_srgb: bool) -> [u8; 4] {
        let color = if uses_srgb {
            self.color
        } else {
            [
                linear_to_srgb(self.color[0]),
                linear_to_srgb(self.color[1]),
                linear_to_srgb(self.color[2]),
                self.color[3],
            ]
        };
        color.map(float_to_u8)
    }

    /// Encode emissive color relative to a shared maximum emissive strength.
    pub fn emissive_srgba_u8(&self, uses_srgb: bool, maximum: f32) -> [u8; 4] {
        let linear = self.linear_rgba(uses_srgb);
        let multiplier = if maximum > 0.0 {
            self.emission / maximum
        } else {
            0.0
        };
        [
            float_to_u8(linear_to_srgb(linear[0] * multiplier)),
            float_to_u8(linear_to_srgb(linear[1] * multiplier)),
            float_to_u8(linear_to_srgb(linear[2] * multiplier)),
            255,
        ]
    }
}

/// Convert linear RGB to the standard sRGB transfer function.
pub(crate) fn linear_to_srgb(value: f32) -> f32 {
    if value <= 0.003_130_8 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    }
}

fn srgb_to_linear(value: f32) -> f32 {
    if value <= 0.040_45 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

pub(crate) fn float_to_u8(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// An analyzed 256-entry voxel palette and its surface/volume lookup tables.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BakedPalette {
    /// Physical material properties indexed by zero-based voxel palette index.
    pub materials: Vec<BakedMaterial>,
    /// Index of refraction for each translucent palette entry.
    pub indices_of_refraction: Vec<Option<f32>>,
    /// Density for each cloud-volume palette entry.
    pub density_for_voxel: Vec<Option<f32>>,
    /// Whether color components are encoded as sRGB rather than linear values.
    pub uses_srgb: bool,
}

impl BakedPalette {
    /// Construct a complete palette and derive its classification lookup tables.
    pub fn new(mut materials: Vec<BakedMaterial>, uses_srgb: bool) -> Self {
        materials.resize_with(256, BakedMaterial::default);
        let indices_of_refraction = materials
            .iter()
            .map(|material| (material.translucency > 0.0).then_some(material.refraction_index))
            .collect();
        let density_for_voxel = materials
            .iter()
            .map(|material| (material.density > 0.0).then_some(material.density))
            .collect();
        Self {
            materials,
            indices_of_refraction,
            density_for_voxel,
            uses_srgb,
        }
    }
}

/// Interpret MagicaVoxel palette colors and material properties for baking.
pub fn analyze_vox_palette(
    data: &DotVoxData,
    diffuse_roughness: f32,
    emission_strength: f32,
    uses_srgb: bool,
) -> BakedPalette {
    BakedPalette::new(
        data.palette
            .iter()
            .zip(data.materials.iter())
            .map(|(color, material)| BakedMaterial {
                color: [
                    color.r as f32 / 255.0,
                    color.g as f32 / 255.0,
                    color.b as f32 / 255.0,
                    color.a as f32 / 255.0,
                ],
                emission: material.emission().unwrap_or(0.0)
                    * (material.radiant_flux().unwrap_or(0.0) + 1.0)
                    * emission_strength,
                roughness: if material.material_type() == Some("_diffuse") {
                    diffuse_roughness
                } else {
                    material.roughness().unwrap_or(0.0)
                },
                metalness: material.metalness().unwrap_or(0.0),
                translucency: material.opacity().unwrap_or(0.0),
                refraction_index: if material.material_type() == Some("_glass") {
                    1.0 + material.refractive_index().unwrap_or(0.0)
                } else {
                    0.0
                },
                density: if material.material_type() == Some("_media") {
                    material.density().unwrap_or(0.0) * 10.0
                } else {
                    0.0
                },
            })
            .collect(),
        uses_srgb,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_derives_surface_and_volume_lookup_tables() {
        let palette = BakedPalette::new(
            vec![
                BakedMaterial {
                    translucency: 0.5,
                    refraction_index: 1.33,
                    ..Default::default()
                },
                BakedMaterial {
                    density: 0.75,
                    ..Default::default()
                },
            ],
            true,
        );
        assert_eq!(palette.materials.len(), 256);
        assert_eq!(palette.indices_of_refraction[0], Some(1.33));
        assert_eq!(palette.density_for_voxel[1], Some(0.75));
        assert_eq!(palette.indices_of_refraction[1], None);
        assert_eq!(palette.density_for_voxel[0], None);
    }

    #[test]
    fn color_conversion_respects_palette_color_space() {
        let linear = BakedMaterial {
            color: [0.5, 0.25, 0.0, 1.0],
            ..Default::default()
        };
        assert_eq!(linear.srgba_u8(false), [188, 137, 0, 255]);
        let srgb = BakedMaterial {
            color: [128.0 / 255.0, 64.0 / 255.0, 0.0, 1.0],
            ..Default::default()
        };
        assert_eq!(srgb.srgba_u8(true), [128, 64, 0, 255]);
        assert!(srgb.linear_rgba(true)[0] < srgb.color[0]);
    }
}
