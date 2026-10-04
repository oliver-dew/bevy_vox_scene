use dot_vox::{Color, DotVoxData, Material};
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
    let mut materials = data
        .palette
        .iter()
        .map(|color| bake_vox_material(color, None, diffuse_roughness, emission_strength))
        .collect::<Vec<_>>();

    for material in &data.materials {
        let Some(index) = material.id.checked_sub(1).map(|id| id as usize) else {
            continue;
        };
        let Some(color) = data.palette.get(index) else {
            continue;
        };
        materials[index] =
            bake_vox_material(color, Some(material), diffuse_roughness, emission_strength);
    }

    BakedPalette::new(materials, uses_srgb)
}

fn bake_vox_material(
    color: &Color,
    material: Option<&Material>,
    diffuse_roughness: f32,
    emission_strength: f32,
) -> BakedMaterial {
    let material_type = material.and_then(Material::material_type);
    BakedMaterial {
        color: [
            color.r as f32 / 255.0,
            color.g as f32 / 255.0,
            color.b as f32 / 255.0,
            color.a as f32 / 255.0,
        ],
        emission: material
            .and_then(|material| {
                material.emission().or_else(|| {
                    (material_type == Some("_emit")).then(|| material.weight().unwrap_or(1.0))
                })
            })
            .unwrap_or(0.0)
            * (material.and_then(Material::radiant_flux).unwrap_or(0.0) + 1.0)
            * emission_strength,
        roughness: if material_type.is_none() || material_type == Some("_diffuse") {
            diffuse_roughness
        } else {
            material.and_then(Material::roughness).unwrap_or(0.0)
        },
        metalness: material.and_then(Material::metalness).unwrap_or(0.0),
        translucency: material
            .and_then(|material| material.transparency().or_else(|| material.opacity()))
            .unwrap_or(0.0),
        refraction_index: if material_type == Some("_glass") {
            material
                .and_then(|material| property_f32(material, "_ri"))
                .unwrap_or_else(|| {
                    1.0 + material.and_then(Material::refractive_index).unwrap_or(0.0)
                })
        } else {
            0.0
        },
        density: if material_type == Some("_media") {
            material.and_then(Material::density).unwrap_or(0.0) * 10.0
        } else {
            0.0
        },
    }
}

fn property_f32(material: &Material, name: &str) -> Option<f32> {
    material.properties.get(name)?.parse().ok()
}

#[cfg(test)]
mod tests {
    use dot_vox::{Color, Dict, DotVoxData, Material};

    use super::*;

    fn source_with_materials(palette: Vec<Color>, materials: Vec<Material>) -> DotVoxData {
        DotVoxData {
            version: 200,
            index_map: (0..=255).collect(),
            models: Vec::new(),
            palette,
            materials,
            scenes: Vec::new(),
            layers: Vec::new(),
        }
    }

    fn material(id: u32, properties: &[(&str, &str)]) -> Material {
        Material {
            id,
            properties: properties
                .iter()
                .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
                .collect::<Dict>(),
        }
    }

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

    #[test]
    fn sparse_material_ids_are_applied_to_their_palette_entries() {
        let source = source_with_materials(
            vec![
                Color {
                    r: 10,
                    g: 20,
                    b: 30,
                    a: 255,
                },
                Color {
                    r: 40,
                    g: 50,
                    b: 60,
                    a: 255,
                },
                Color {
                    r: 70,
                    g: 80,
                    b: 90,
                    a: 255,
                },
            ],
            vec![material(
                3,
                &[("_type", "_emit"), ("_weight", "0.5"), ("_flux", "1")],
            )],
        );

        let palette = analyze_vox_palette(&source, 0.8, 20.0, true);

        assert_eq!(palette.materials[0].srgba_u8(true), [10, 20, 30, 255]);
        assert_eq!(palette.materials[1].srgba_u8(true), [40, 50, 60, 255]);
        assert_eq!(palette.materials[2].srgba_u8(true), [70, 80, 90, 255]);
        assert_eq!(palette.materials[0].roughness, 0.8);
        assert_eq!(palette.materials[1].emission, 0.0);
        assert_eq!(palette.materials[2].emission, 20.0);
    }

    #[test]
    fn modern_glass_prefers_transmission_and_absolute_ior_properties() {
        let source = source_with_materials(
            vec![Color {
                r: 255,
                g: 255,
                b: 255,
                a: 255,
            }],
            vec![material(
                1,
                &[
                    ("_type", "_glass"),
                    ("_trans", "0.7"),
                    ("_alpha", "0.2"),
                    ("_ri", "1.333"),
                    ("_ior", "0.1"),
                ],
            )],
        );

        let palette = analyze_vox_palette(&source, 0.8, 20.0, true);

        assert_eq!(palette.materials[0].translucency, 0.7);
        assert_eq!(palette.materials[0].refraction_index, 1.333);
    }
}
