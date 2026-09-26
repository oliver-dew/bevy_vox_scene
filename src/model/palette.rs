use bevy::{
    asset::{Handle, RenderAssetUsages},
    color::{Color, ColorToComponents, ColorToPacked, LinearRgba},
    image::{Image, ImageSampler},
    math::FloatExt,
    mesh::UvChannel,
    pbr::StandardMaterial,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use dot_vox::DotVoxData;

use super::mesh::{AmbientOcclusionAtlas, MeshAtlas};

/// Container for all of the [`VoxelElement`]s that can be used in a [`super::VoxelModel`]
#[derive(Clone, Debug)]
pub struct VoxelPalette {
    pub(crate) elements: Vec<VoxelElement>,
    pub(crate) indices_of_refraction: Vec<Option<f32>>,
    pub(crate) density_for_voxel: Vec<Option<f32>>,
    /// If true, uses SRGB for colors. Uses Linear colors if false.
    pub(crate) uses_srgb: bool,
}

/// A material for a type of voxel brick modelled with physical properties such as color, roughness and so on.
#[derive(Clone, Debug)]
pub struct VoxelElement {
    /// The base color of the voxel
    pub color: Color,
    /// The emissive strength of the voxel. This will be multiplied by the [`VoxelElement::color`] to create the emissive color
    pub emission: f32,
    /// The perceptual roughness of the voxel on a scale of 0.0 to 1.0
    pub roughness: f32,
    /// The metalness of the voxel on a scale of 0.0 to 1.0
    pub metalness: f32,
    /// The translucency or transmissiveness of the voxel on a scale of 0.0 to 1.0, with 0.0 being fully opaque and 1.0 being fully translucent
    pub translucency: f32,
    /// The index of refraction of translucent voxels. Has no effect if [`VoxelElement::translucency`] is 0.0
    pub refraction_index: f32,
    /// The density of cloud materials.
    pub density: f32,
}

impl Default for VoxelElement {
    fn default() -> Self {
        Self {
            color: Color::LinearRgba(LinearRgba::RED),
            emission: 0.0,
            roughness: 0.5,
            metalness: 0.0,
            translucency: 0.0,
            refraction_index: 1.5,
            density: 0.0,
        }
    }
}

impl VoxelPalette {
    /// Create a new [`VoxelPalette`] from the supplied [`VoxelElement`]s
    pub fn new(mut elements: Vec<VoxelElement>, uses_srgb: bool) -> Self {
        elements.resize_with(256, VoxelElement::default);
        let indices_of_refraction: Vec<Option<f32>> = elements
            .iter()
            .map(|e| {
                if e.translucency > 0.0 {
                    Some(e.refraction_index)
                } else {
                    None
                }
            })
            .collect();
        let density_for_voxel = elements
            .iter()
            .map(|e| {
                if e.density > 0.0 {
                    Some(e.density)
                } else {
                    None
                }
            })
            .collect();
        VoxelPalette {
            elements,
            indices_of_refraction,
            density_for_voxel,
            uses_srgb,
        }
    }

    /// Create a new [`VoxelPalette`] from the supplied [`Color`]s
    pub fn from_colors(colors: Vec<Color>, uses_srgb: bool) -> Self {
        VoxelPalette::new(
            colors
                .iter()
                .map(|color| VoxelElement {
                    color: *color,
                    ..Default::default()
                })
                .collect(),
            uses_srgb,
        )
    }

    /// Create a new [`VoxelPalette`] by interpolating between the [`VoxelElement`] in the gradient stops
    pub fn from_gradient(stops: &[(u8, VoxelElement)], uses_srgb: bool) -> Self {
        let mut elements = vec![VoxelElement::default(); 256];

        if stops.is_empty() {
            return VoxelPalette::new(elements, uses_srgb);
        }

        // Normalize stops to avoid divide-by-zero and ordering bugs.
        // If the same stop appears multiple times, keep the last entry.
        let mut normalized = stops.to_vec();
        normalized.sort_by_key(|(stop, _)| *stop);
        let mut deduped: Vec<(u8, VoxelElement)> = Vec::with_capacity(normalized.len());
        for (stop, element) in normalized {
            if let Some((last_stop, last_element)) = deduped.last_mut()
                && *last_stop == stop
            {
                *last_element = element;
                continue;
            }
            deduped.push((stop, element));
        }

        let (first_stop, first_element) = &deduped[0];
        for index in 0..=*first_stop as usize {
            elements[index] = first_element.clone();
        }

        for window in deduped.windows(2) {
            let (start_stop, start_element) = (&window[0].0, &window[0].1);
            let (end_stop, end_element) = (&window[1].0, &window[1].1);
            if end_stop <= start_stop {
                continue;
            }
            let distance = (end_stop - start_stop) as f32;
            for i in *start_stop..=*end_stop {
                let fraction = (i - start_stop) as f32 / distance;
                elements[i as usize] = VoxelElement {
                    color: Color::LinearRgba(
                        start_element
                            .color
                            .to_linear()
                            .lerp(end_element.color.to_linear(), fraction),
                    ),
                    emission: start_element.emission.lerp(end_element.emission, fraction),
                    roughness: start_element
                        .roughness
                        .lerp(end_element.roughness, fraction),
                    metalness: start_element
                        .metalness
                        .lerp(end_element.metalness, fraction),
                    translucency: start_element
                        .translucency
                        .lerp(end_element.translucency, fraction),
                    refraction_index: start_element
                        .refraction_index
                        .lerp(end_element.refraction_index, fraction),
                    density: start_element.density.lerp(end_element.density, fraction),
                };
            }
        }

        let (last_stop, last_element) = &deduped[deduped.len() - 1];
        for index in *last_stop as usize..256 {
            elements[index] = last_element.clone();
        }

        VoxelPalette::new(elements, uses_srgb)
    }

    pub(crate) fn from_data(
        data: &DotVoxData,
        diffuse_roughness: f32,
        emission_strength: f32,
        uses_srgb: bool,
    ) -> Self {
        VoxelPalette::new(
            data.palette
                .iter()
                .zip(data.materials.iter())
                .map(|(color, material)| VoxelElement {
                    color: if uses_srgb {
                        Color::srgba_u8(color.r, color.g, color.b, color.a)
                    } else {
                        Color::linear_rgba(
                            color.r as f32 / 255.,
                            color.g as f32 / 255.,
                            color.b as f32 / 255.,
                            color.a as f32 / 255.,
                        )
                    },
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

    pub(crate) fn create_atlas_material(
        &self,
        atlas: &MeshAtlas,
        get_handle: impl FnMut(&str, Image) -> Handle<Image>,
    ) -> StandardMaterial {
        self._create_material(
            atlas.width,
            atlas.height,
            &atlas.palette_indices,
            &atlas.used_palette_indices,
            atlas.ambient_occlusion.as_ref(),
            get_handle,
        )
    }

    fn _create_material(
        &self,
        width: u32,
        height: u32,
        palette_indices: &[u8],
        used_palette_indices: &[u8],
        ambient_occlusion: Option<&AmbientOcclusionAtlas>,
        mut get_handle: impl FnMut(&str, Image) -> Handle<Image>,
    ) -> StandardMaterial {
        let image_size = Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let color_data: Vec<u8> = palette_indices
            .iter()
            .flat_map(|index| {
                let element = &self.elements[*index as usize];
                if self.uses_srgb {
                    element.color.to_srgba().to_u8_array()
                } else {
                    element.color.to_linear().to_u8_array()
                }
            })
            .collect();
        let emission_data: Vec<f32> = palette_indices
            .iter()
            .map(|index| self.elements[*index as usize].emission)
            .collect();
        let roughness_data: Vec<f32> = palette_indices
            .iter()
            .map(|index| self.elements[*index as usize].roughness)
            .collect();
        let metalness_data: Vec<f32> = palette_indices
            .iter()
            .map(|index| self.elements[*index as usize].metalness)
            .collect();
        #[cfg(feature = "pbr_transmission_textures")]
        let translucency_data: Vec<f32> = palette_indices
            .iter()
            .map(|index| self.elements[*index as usize].translucency)
            .collect();

        let used_elements: Vec<&VoxelElement> = used_palette_indices
            .iter()
            .map(|index| &self.elements[*index as usize])
            .collect();
        let first = used_elements.first().copied().unwrap_or(&self.elements[0]);
        let varies = |get: fn(&VoxelElement) -> f32| {
            used_elements
                .iter()
                .any(|element| (get(element) - get(first)).abs() >= 0.001)
        };
        let first_color = first.color.to_linear();
        let base_color_varies = used_elements
            .iter()
            .any(|element| !rgba_approx_eq(element.color.to_linear(), first_color));
        let first_emissive = first_color * first.emission;
        let emissive_varies = used_elements.iter().any(|element| {
            !rgba_approx_eq(element.color.to_linear() * element.emission, first_emissive)
        });
        let has_roughness = varies(|element| element.roughness);
        let has_metalness = varies(|element| element.metalness);
        let has_roughness_metalness = has_roughness || has_metalness;
        let has_translucency = varies(|element| element.translucency);

        let base_color_texture = base_color_varies.then(|| {
            get_handle(
                "material_color",
                nearest_image(Image::new(
                    image_size,
                    TextureDimension::D2,
                    color_data,
                    if self.uses_srgb {
                        TextureFormat::Rgba8UnormSrgb
                    } else {
                        TextureFormat::Rgba8Unorm
                    },
                    RenderAssetUsages::RENDER_WORLD,
                )),
            )
        });

        let emissive_texture = if emissive_varies {
            let emission_bytes: Vec<u8> = emission_data
                .iter()
                .zip(palette_indices)
                .flat_map(|(emission, index)| {
                    (self.elements[*index as usize].color.to_linear() * *emission)
                        .to_f32_array()
                        .iter()
                        .flat_map(|c| c.to_le_bytes())
                        .collect::<Vec<u8>>()
                })
                .collect();
            Some(get_handle(
                "material_emission",
                nearest_image(Image::new(
                    image_size,
                    TextureDimension::D2,
                    emission_bytes,
                    TextureFormat::Rgba32Float,
                    RenderAssetUsages::RENDER_WORLD,
                )),
            ))
        } else {
            None
        };

        let metallic_roughness_texture: Option<Handle<Image>> = if has_roughness_metalness {
            #[cfg(feature = "webgpu")]
            let (raw, format) = {
                let raw: Vec<u8> = roughness_data
                    .iter()
                    .zip(metalness_data.iter())
                    .flat_map(|(rough, metal)| {
                        [
                            0u8,
                            (rough * u8::MAX as f32) as u8,
                            (metal * u8::MAX as f32) as u8,
                            0u8,
                        ]
                    })
                    .collect();
                (raw, TextureFormat::Rgba8Unorm)
            };
            #[cfg(not(feature = "webgpu"))]
            let (raw, format) = {
                let raw: Vec<u8> = roughness_data
                    .iter()
                    .zip(metalness_data.iter())
                    .flat_map(|(rough, metal)| {
                        let output: Vec<u8> = [0.0, *rough, *metal, 0.0]
                            .iter()
                            .flat_map(|b| ((b * u16::MAX as f32) as u16).to_le_bytes())
                            .collect();
                        output
                    })
                    .collect();
                (raw, TextureFormat::Rgba16Unorm)
            };
            let handle = get_handle(
                "material_metallic_roughness",
                nearest_image(Image::new(
                    image_size,
                    TextureDimension::D2,
                    raw,
                    format,
                    RenderAssetUsages::RENDER_WORLD,
                )),
            );
            Some(handle)
        } else {
            None
        };

        let occlusion_texture = ambient_occlusion.map(|atlas| {
            get_handle(
                "material_occlusion",
                linear_image(Image::new(
                    Extent3d {
                        width: atlas.width,
                        height: atlas.height,
                        depth_or_array_layers: 1,
                    },
                    TextureDimension::D2,
                    atlas.data.clone(),
                    TextureFormat::R8Unorm,
                    RenderAssetUsages::RENDER_WORLD,
                )),
            )
        });

        #[cfg(feature = "pbr_transmission_textures")]
        let specular_transmission_texture: Option<Handle<Image>> = if has_translucency {
            #[cfg(feature = "webgpu")]
            let (raw, format) = {
                let raw: Vec<u8> = translucency_data
                    .iter()
                    .map(|t| (t * u8::MAX as f32) as u8)
                    .collect();
                (raw, TextureFormat::R8Unorm)
            };
            #[cfg(not(feature = "webgpu"))]
            let (raw, format) = {
                let raw: Vec<u8> = translucency_data
                    .iter()
                    .flat_map(|t| ((t * u16::MAX as f32) as u16).to_le_bytes())
                    .collect();
                (raw, TextureFormat::R16Unorm)
            };
            let handle = get_handle(
                "material_specular_transmission",
                nearest_image(Image::new(
                    image_size,
                    TextureDimension::D2,
                    raw,
                    format,
                    RenderAssetUsages::RENDER_WORLD,
                )),
            );
            Some(handle)
        } else {
            None
        };

        StandardMaterial {
            base_color: if base_color_varies {
                Color::WHITE
            } else {
                first.color
            },
            base_color_texture,
            emissive: if emissive_varies {
                LinearRgba::WHITE
            } else {
                first_emissive
            },
            emissive_texture,
            perceptual_roughness: if has_roughness_metalness {
                1.0
            } else {
                first.roughness
            },
            metallic: if has_roughness_metalness {
                1.0
            } else {
                first.metalness
            },
            metallic_roughness_texture,
            occlusion_channel: if ambient_occlusion.is_some() {
                UvChannel::Uv1
            } else {
                UvChannel::Uv0
            },
            occlusion_texture,
            specular_transmission: if has_translucency {
                1.0
            } else {
                first.translucency
            },
            #[cfg(feature = "pbr_transmission_textures")]
            specular_transmission_texture,
            ..Default::default()
        }
    }
}

fn nearest_image(mut image: Image) -> Image {
    image.sampler = ImageSampler::nearest();
    image
}

fn linear_image(mut image: Image) -> Image {
    image.sampler = ImageSampler::linear();
    image
}

fn rgba_approx_eq(left: LinearRgba, right: LinearRgba) -> bool {
    left.to_f32_array()
        .iter()
        .zip(right.to_f32_array())
        .all(|(left, right)| (left - right).abs() < 0.001)
}

trait Lerpable {
    fn lerp(&self, rhs: Self, amount: f32) -> Self;
}

impl Lerpable for LinearRgba {
    fn lerp(&self, rhs: Self, amount: f32) -> Self {
        let lhs = self.to_f32_array();
        let rhs = rhs.to_f32_array();
        let mixed: [f32; 4] = std::array::from_fn(|i| lhs[i].lerp(rhs[i], amount));
        LinearRgba::from_f32_array(mixed)
    }
}

#[cfg(test)]
mod tests {
    use super::{VoxelPalette, nearest_image};
    use crate::model::mesh::{AmbientOcclusionAtlas, MeshAtlas};
    use bevy::{
        asset::{Assets, RenderAssetUsages},
        color::palettes::css::WHITE,
        image::{Image, ImageSampler},
        mesh::{Mesh, UvChannel},
        render::render_resource::{PrimitiveTopology, TextureFormat},
    };

    #[test]
    fn ambient_occlusion_uses_a_linear_r8_texture() {
        let palette = VoxelPalette::from_colors(vec![WHITE.into()], true);
        let atlas = MeshAtlas {
            mesh: Mesh::new(
                PrimitiveTopology::TriangleList,
                RenderAssetUsages::RENDER_WORLD,
            ),
            width: 2,
            height: 1,
            palette_indices: vec![0, 0],
            used_palette_indices: vec![0],
            ambient_occlusion: Some(AmbientOcclusionAtlas {
                width: 1,
                height: 2,
                data: vec![u8::MAX, 128],
            }),
        };
        let mut images = Assets::<Image>::default();
        let material = palette.create_atlas_material(&atlas, |_, image| images.add(image));
        let image = images
            .get(
                material
                    .occlusion_texture
                    .as_ref()
                    .expect("occlusion texture"),
            )
            .expect("occlusion image");

        assert_eq!(image.texture_descriptor.format, TextureFormat::R8Unorm);
        assert_eq!(image.texture_descriptor.size.width, 1);
        assert_eq!(image.texture_descriptor.size.height, 2);
        assert_eq!(image.sampler, ImageSampler::linear());
        assert_eq!(image.data.as_deref(), Some([u8::MAX, 128].as_slice()));
        assert_eq!(material.occlusion_channel, UvChannel::Uv1);

        // Keep nearest sampling behavior covered for the other generated maps.
        assert_eq!(
            nearest_image(Image::default()).sampler,
            ImageSampler::nearest()
        );
    }
}
