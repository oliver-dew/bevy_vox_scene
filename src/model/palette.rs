use bevy::{
    asset::{Assets, Handle, LoadContext},
    color::{Color, ColorToComponents, ColorToPacked, LinearRgba},
    math::FloatExt,
    pbr::StandardMaterial,
    render::{
        render_asset::RenderAssetUsages,
        render_resource::{Extent3d, TextureDimension, TextureFormat},
        texture::Image,
    },
};
use dot_vox::DotVoxData;

/// Container for all of the [`VoxelElement`]s that can be used in a [`super::VoxelModel`]
#[derive(Clone, Debug)]
pub struct VoxelPalette {
    pub(crate) elements: Vec<VoxelElement>,
    pub(crate) emission: MaterialProperty,
    pub(crate) metalness: MaterialProperty,
    pub(crate) roughness: MaterialProperty,
    pub(crate) transmission: MaterialProperty,
    pub(crate) indices_of_refraction: Vec<Option<f32>>,
}

#[derive(PartialEq, Clone, Debug)]
pub(crate) enum MaterialProperty {
    VariesPerElement,
    Constant(f32),
}

impl MaterialProperty {
    fn from_slice(slice: &[f32]) -> Self {
        let max_element = slice.max_element();
        if max_element - slice.min_element() < 0.001 {
            MaterialProperty::Constant(max_element)
        } else {
            MaterialProperty::VariesPerElement
        }
    }
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
        }
    }
}

impl VoxelPalette {
    /// Create a new [`VoxelPalette`] from the supplied [`VoxelElement`]s
    pub fn new(mut elements: Vec<VoxelElement>) -> Self {
        let emission_data: Vec<f32> = elements.iter().map(|e| e.emission).collect();
        let roughness_data: Vec<f32> = elements.iter().map(|e| e.roughness).collect();
        let metalness_data: Vec<f32> = elements.iter().map(|e| e.metalness).collect();
        let translucency_data: Vec<f32> = elements.iter().map(|e| e.translucency).collect();

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
        VoxelPalette {
            elements,
            emission: MaterialProperty::from_slice(&emission_data),
            metalness: MaterialProperty::from_slice(&metalness_data),
            roughness: MaterialProperty::from_slice(&roughness_data),
            transmission: MaterialProperty::from_slice(&translucency_data),
            indices_of_refraction,
        }
    }

    /// Create a new [`VoxelPalette`] from the supplied [`Color`]s
    pub fn from_colors(colors: Vec<Color>) -> Self {
        VoxelPalette::new(
            colors
                .iter()
                .map(|color| VoxelElement {
                    color: *color,
                    ..Default::default()
                })
                .collect(),
        )
    }

    /// Create a new [`VoxelPalette`] by interpolating between the [`VoxelElement`] in the gradient stops
    pub fn from_gradient(stops: &[(u8, VoxelElement)]) -> Self {
        let mut elements = vec![VoxelElement::default(); 256];
        for (index, (stop, element)) in stops.iter().enumerate() {
            let default = (u8::MAX - 1, element.clone());
            let (next_stop, next_element) = stops.get(index + 1).unwrap_or(&default);
            let distance = (next_stop - stop) as f32;
            for i in *stop..*next_stop {
                let fraction = (i - stop) as f32 / distance;
                elements[i as usize] = VoxelElement {
                    color: Color::LinearRgba(
                        element
                            .color
                            .to_linear()
                            .lerp(next_element.color.to_linear(), fraction),
                    ),
                    emission: element.emission.lerp(next_element.emission, fraction),
                    roughness: element.roughness.lerp(next_element.roughness, fraction),
                    metalness: element.metalness.lerp(next_element.metalness, fraction),
                    translucency: element
                        .translucency
                        .lerp(next_element.translucency, fraction),
                    refraction_index: element
                        .refraction_index
                        .lerp(next_element.refraction_index, fraction),
                };
            }
        }
        VoxelPalette::new(elements)
    }

    pub(crate) fn from_data(
        data: &DotVoxData,
        diffuse_roughness: f32,
        emission_strength: f32,
    ) -> Self {
        VoxelPalette::new(
            data.palette
                .iter()
                .zip(data.materials.iter())
                .map(|(color, material)| VoxelElement {
                    color: Color::linear_rgba(
                        color.r as f32 / 255.,
                        color.g as f32 / 255.,
                        color.b as f32 / 255.,
                        color.a as f32 / 255.,
                    ), //srgba_u8(color.r, color.g, color.b, color.a),
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
                })
                .collect(),
        )
    }

    pub(crate) fn create_material_in_load_context(
        &self,
        load_context: &mut LoadContext,
    ) -> StandardMaterial {
        self._create_material(|name, image| load_context.add_labeled_asset(name.to_string(), image))
    }

    pub(crate) fn create_material(&self, images: &mut Assets<Image>) -> StandardMaterial {
        self._create_material(|_, image| images.add(image))
    }

    fn _create_material(
        &self,
        mut get_handle: impl FnMut(&str, Image) -> Handle<Image>,
    ) -> StandardMaterial {
        let image_size = Extent3d {
            width: 16,
            height: 16,
            depth_or_array_layers: 1,
        };
        let color_data: Vec<u8> = self
            .elements
            .iter()
            .flat_map(|e| e.color.to_linear().to_u8_array())
            .collect();
        let emission_data: Vec<f32> = self.elements.iter().map(|e| e.emission).collect();
        let roughness_data: Vec<f32> = self.elements.iter().map(|e| e.roughness).collect();
        let metalness_data: Vec<f32> = self.elements.iter().map(|e| e.metalness).collect();
        #[cfg(feature = "pbr_transmission_textures")]
        let translucency_data: Vec<f32> = self.elements.iter().map(|e| e.translucency).collect();

        let has_emission = match self.emission {
            MaterialProperty::VariesPerElement => true,
            MaterialProperty::Constant(emission) => emission > 0.0,
        };
        let has_roughness = self.roughness == MaterialProperty::VariesPerElement;
        let has_metalness = self.metalness == MaterialProperty::VariesPerElement;
        let has_roughness_metalness = has_roughness || has_metalness;
        let has_translucency = self.transmission == MaterialProperty::VariesPerElement;

        let base_color_texture = Some(get_handle(
            "material_color",
            Image::new(
                image_size,
                TextureDimension::D2,
                color_data,
                TextureFormat::Rgba8UnormSrgb,
                RenderAssetUsages::default(),
            ),
        ));

        let emissive_texture = if has_emission {
            let emission_bytes: Vec<u8> = emission_data
                .iter()
                .zip(self.elements.iter().map(|e| e.color))
                .flat_map(|(emission, color)| {
                    (color.to_linear() * *emission)
                        .to_f32_array()
                        .iter()
                        .flat_map(|c| c.to_le_bytes())
                        .collect::<Vec<u8>>()
                })
                .collect();
            Some(get_handle(
                "material_emission",
                Image::new(
                    image_size,
                    TextureDimension::D2,
                    emission_bytes,
                    TextureFormat::Rgba32Float,
                    RenderAssetUsages::default(),
                ),
            ))
        } else {
            None
        };

        let metallic_roughness_texture: Option<Handle<Image>> = if has_roughness_metalness {
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
            let handle = get_handle(
                "material_metallic_roughness",
                Image::new(
                    image_size,
                    TextureDimension::D2,
                    raw,
                    TextureFormat::Rgba16Unorm,
                    RenderAssetUsages::default(),
                ),
            );
            Some(handle)
        } else {
            None
        };

        #[cfg(feature = "pbr_transmission_textures")]
        let specular_transmission_texture: Option<Handle<Image>> = if has_translucency {
            let raw: Vec<u8> = translucency_data
                .iter()
                .flat_map(|t| ((t * u16::MAX as f32) as u16).to_le_bytes())
                .collect();
            let handle = get_handle(
                "material_specular_transmission",
                Image::new(
                    image_size,
                    TextureDimension::D2,
                    raw,
                    TextureFormat::R16Unorm,
                    RenderAssetUsages::default(),
                ),
            );
            Some(handle)
        } else {
            None
        };

        StandardMaterial {
            base_color_texture,
            emissive: if has_emission {
                LinearRgba::WHITE
            } else {
                LinearRgba::BLACK
            },
            emissive_texture,
            perceptual_roughness: match (has_roughness_metalness, &self.roughness) {
                (true, _) | (_, MaterialProperty::VariesPerElement) => 1.0,
                (false, MaterialProperty::Constant(roughness)) => *roughness,
            },
            metallic: match (has_roughness_metalness, &self.metalness) {
                (true, _) | (false, MaterialProperty::VariesPerElement) => 1.0,
                (false, MaterialProperty::Constant(metalness)) => *metalness,
            },
            metallic_roughness_texture,
            specular_transmission: match self.transmission {
                MaterialProperty::Constant(transmission) => transmission,
                MaterialProperty::VariesPerElement => 1.0,
            },
            #[cfg(feature = "pbr_transmission_textures")]
            specular_transmission_texture,
            ..Default::default()
        }
    }
}

trait VecComparable<T> {
    fn max_element(&self) -> T;

    fn min_element(&self) -> T;
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
impl VecComparable<f32> for Vec<f32> {
    fn max_element(&self) -> f32 {
        self.iter()
            .cloned()
            .max_by(|a, b| a.partial_cmp(b).expect("tried to compare NaN"))
            .unwrap()
    }

    fn min_element(&self) -> f32 {
        self.iter()
            .cloned()
            .min_by(|a, b| a.partial_cmp(b).expect("tried to compare NaN"))
            .unwrap()
    }
}

impl VecComparable<f32> for &[f32] {
    fn max_element(&self) -> f32 {
        self.iter()
            .cloned()
            .max_by(|a, b| a.partial_cmp(b).expect("tried to compare NaN"))
            .unwrap()
    }

    fn min_element(&self) -> f32 {
        self.iter()
            .cloned()
            .min_by(|a, b| a.partial_cmp(b).expect("tried to compare NaN"))
            .unwrap()
    }
}
