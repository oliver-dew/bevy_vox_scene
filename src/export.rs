//! Offline conversion of MagicaVoxel files to binary glTF.

use std::{borrow::Cow, collections::BTreeMap};

use bevy::{
    color::{Color, ColorToPacked, LinearRgba},
    math::Mat4,
    mesh::{Indices, Mesh, VertexAttributeValues},
};
use dot_vox::{DotVoxData, SceneNode};
use gltf::{binary, json};
use json::{
    Index,
    validation::{Checked::Valid, USize64},
};
use thiserror::Error;

use crate::{
    VoxLoaderSettings, VoxelData,
    load::{find_model_names, transform_from_frame},
    model::{VoxelPalette, mesh::MeshAtlas},
};

/// One GLB produced by [`export_vox_to_glb`].
#[derive(Clone, Debug)]
pub struct ExportedGlb {
    /// Zero-based frame index. Static files contain one result with index zero.
    pub frame_index: usize,
    /// Complete binary glTF file contents.
    pub bytes: Vec<u8>,
}

/// An error encountered while converting a `.vox` file to GLB.
#[derive(Debug, Error)]
pub enum ExportError {
    /// The source bytes were not a valid MagicaVoxel file.
    #[error("invalid VOX file: {0}")]
    InvalidVox(String),
    /// The source file has no scene graph root.
    #[error("VOX file contains no scene graph root")]
    MissingScene,
    /// Cloud volumes have no portable glTF representation.
    #[error("model {0:?} contains cloud voxels, which cannot currently be exported to glTF")]
    UnsupportedCloud(String),
    /// A generated mesh did not contain an expected attribute.
    #[error("mesh {mesh:?} has unsupported or missing attribute {attribute}")]
    MeshAttribute {
        /// Model name.
        mesh: String,
        /// Attribute name.
        attribute: &'static str,
    },
    /// A generated image could not be encoded.
    #[error("could not encode PNG: {0}")]
    Png(#[from] png::EncodingError),
    /// The glTF JSON document could not be serialized.
    #[error("could not serialize glTF JSON: {0}")]
    Json(#[from] json::Error),
    /// The GLB container could not be written.
    #[error("could not write GLB: {0}")]
    Glb(#[from] gltf::Error),
    /// The generated GLB exceeded the format's 32-bit size limit.
    #[error("generated GLB exceeds the format's 4 GiB size limit")]
    TooLarge,
}

struct BakedModel {
    name: String,
    data: VoxelData,
    atlas: Option<MeshAtlas>,
    average_ior: Option<f32>,
}

/// Convert MagicaVoxel bytes into one or more complete GLB files.
///
/// Static input returns one item. If any shape contains multiple voxel models,
/// each animation frame is returned as a separate scene snapshot in frame order.
/// Callers are responsible for naming the files; the CLI uses `-001`, `-002`, …
/// suffixes for animated output.
pub fn export_vox_to_glb(
    bytes: &[u8],
    settings: &VoxLoaderSettings,
) -> Result<Vec<ExportedGlb>, ExportError> {
    let file = dot_vox::load_bytes(bytes).map_err(|error| ExportError::InvalidVox(error.into()))?;
    let root_scene = file.scenes.first().ok_or(ExportError::MissingScene)?;
    let palette = VoxelPalette::from_data(
        &file,
        settings.diffuse_roughness,
        settings.emission_strength,
        settings.uses_srgb,
    );
    let mut names = vec![None; file.models.len()];
    find_model_names(&mut names, &file.scenes, root_scene, None);
    let models = names
        .into_iter()
        .zip(&file.models)
        .enumerate()
        .map(|(index, (name, model))| {
            let name = name.unwrap_or_else(|| format!("model-{index}"));
            let data = VoxelData::from_model(model, settings.clone());
            let (visible, average_ior, has_mesh) =
                data.visible_voxels(&palette.indices_of_refraction, &palette.density_for_voxel);
            let (_, has_cloud) = data.cloud_voxels(&palette.density_for_voxel);
            if has_cloud {
                return Err(ExportError::UnsupportedCloud(name));
            }
            Ok(BakedModel {
                name,
                atlas: has_mesh.then(|| crate::model::mesh::mesh_model(&visible, &data)),
                data,
                average_ior,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let frame_count = animation_frame_count(&file).max(1);
    (0..frame_count)
        .map(|frame_index| {
            Ok(ExportedGlb {
                frame_index,
                bytes: write_frame(&file, root_scene, &models, &palette, frame_index, settings)?,
            })
        })
        .collect()
}

fn animation_frame_count(file: &DotVoxData) -> usize {
    file.scenes
        .iter()
        .map(|node| match node {
            SceneNode::Shape { models, .. } => models.len(),
            SceneNode::Transform { frames, .. } => frames.len(),
            SceneNode::Group { .. } => 1,
        })
        .max()
        .unwrap_or(1)
}

fn write_frame(
    file: &DotVoxData,
    root_scene: &SceneNode,
    models: &[BakedModel],
    palette: &VoxelPalette,
    frame_index: usize,
    settings: &VoxLoaderSettings,
) -> Result<Vec<u8>, ExportError> {
    let mut writer = GltfWriter::new();
    let mut gltf_meshes = Vec::with_capacity(models.len());
    for model in models {
        gltf_meshes.push(match &model.atlas {
            Some(atlas) => Some(writer.add_model(model, atlas, palette)?),
            None => None,
        });
    }

    let roots = writer.add_scene_node(
        &file.scenes,
        root_scene,
        models,
        &gltf_meshes,
        &file.layers,
        frame_index,
        settings.voxel_size,
        true,
    );
    let scene = writer.root.push(json::Scene {
        name: Some(if frame_index == 0 {
            "Voxel scene".into()
        } else {
            format!("Voxel scene frame {}", frame_index + 1)
        }),
        nodes: roots,
        extensions: None,
        extras: None,
    });
    writer.root.scene = Some(scene);
    writer.finish()
}

struct GltfWriter {
    root: json::Root,
    bin: Vec<u8>,
    buffer: Index<json::Buffer>,
    nearest_sampler: Index<json::texture::Sampler>,
    linear_sampler: Index<json::texture::Sampler>,
}

impl GltfWriter {
    fn new() -> Self {
        let mut root = json::Root::default();
        root.asset.generator = Some(format!("bevy_vox_scene {}", env!("CARGO_PKG_VERSION")));
        let buffer = root.push(json::Buffer {
            byte_length: USize64(0),
            name: Some("GLB buffer".into()),
            uri: None,
            extensions: None,
            extras: None,
        });
        let nearest_sampler = root.push(json::texture::Sampler {
            mag_filter: Some(Valid(json::texture::MagFilter::Nearest)),
            min_filter: Some(Valid(json::texture::MinFilter::Nearest)),
            name: Some("Voxel nearest".into()),
            wrap_s: Valid(json::texture::WrappingMode::ClampToEdge),
            wrap_t: Valid(json::texture::WrappingMode::ClampToEdge),
            extensions: None,
            extras: None,
        });
        let linear_sampler = root.push(json::texture::Sampler {
            mag_filter: Some(Valid(json::texture::MagFilter::Linear)),
            min_filter: Some(Valid(json::texture::MinFilter::Linear)),
            name: Some("AO linear".into()),
            wrap_s: Valid(json::texture::WrappingMode::ClampToEdge),
            wrap_t: Valid(json::texture::WrappingMode::ClampToEdge),
            extensions: None,
            extras: None,
        });
        Self {
            root,
            bin: Vec::new(),
            buffer,
            nearest_sampler,
            linear_sampler,
        }
    }

    fn add_model(
        &mut self,
        model: &BakedModel,
        atlas: &MeshAtlas,
        palette: &VoxelPalette,
    ) -> Result<Index<json::Mesh>, ExportError> {
        let positions = mesh_vec3(&atlas.mesh, Mesh::ATTRIBUTE_POSITION, &model.name)?;
        let normals = mesh_vec3(&atlas.mesh, Mesh::ATTRIBUTE_NORMAL, &model.name)?;
        let uvs = mesh_vec2(&atlas.mesh, Mesh::ATTRIBUTE_UV_0, &model.name)?;
        let ao_uvs = atlas
            .ambient_occlusion
            .as_ref()
            .map(|_| mesh_vec2(&atlas.mesh, Mesh::ATTRIBUTE_UV_1, &model.name))
            .transpose()?;
        let indices = match atlas.mesh.indices() {
            Some(Indices::U32(values)) => values.clone(),
            Some(Indices::U16(values)) => values.iter().map(|value| u32::from(*value)).collect(),
            None => {
                return Err(ExportError::MeshAttribute {
                    mesh: model.name.clone(),
                    attribute: "indices",
                });
            }
        };

        let (min, max) = bounds(&positions);
        let position_accessor =
            self.add_f32_accessor(&positions, json::accessor::Type::Vec3, Some((min, max)));
        let normal_accessor = self.add_f32_accessor(&normals, json::accessor::Type::Vec3, None);
        let uv_accessor = self.add_f32_accessor(&uvs, json::accessor::Type::Vec2, None);
        let ao_accessor = ao_uvs
            .as_ref()
            .map(|values| self.add_f32_accessor(values, json::accessor::Type::Vec2, None));
        let index_accessor = self.add_indices(&indices);
        let material = self.add_material(model, atlas, palette)?;
        let mut attributes = BTreeMap::new();
        attributes.insert(Valid(json::mesh::Semantic::Positions), position_accessor);
        attributes.insert(Valid(json::mesh::Semantic::Normals), normal_accessor);
        attributes.insert(Valid(json::mesh::Semantic::TexCoords(0)), uv_accessor);
        if let Some(accessor) = ao_accessor {
            attributes.insert(Valid(json::mesh::Semantic::TexCoords(1)), accessor);
        }
        let primitive = json::mesh::Primitive {
            attributes,
            indices: Some(index_accessor),
            material: Some(material),
            mode: Valid(json::mesh::Mode::Triangles),
            extensions: None,
            extras: None,
            targets: None,
        };
        Ok(self.root.push(json::Mesh {
            name: Some(model.name.clone()),
            primitives: vec![primitive],
            extensions: None,
            extras: None,
            weights: None,
        }))
    }

    fn add_material(
        &mut self,
        model: &BakedModel,
        atlas: &MeshAtlas,
        palette: &VoxelPalette,
    ) -> Result<Index<json::Material>, ExportError> {
        let rgba = atlas
            .palette_indices
            .iter()
            .flat_map(|index| {
                palette.elements[*index as usize]
                    .color
                    .to_srgba()
                    .to_u8_array()
            })
            .collect::<Vec<_>>();
        let base = self.add_png_texture(
            &format!("{} base color", model.name),
            atlas.width,
            atlas.height,
            &rgba,
            png::ColorType::Rgba,
            self.nearest_sampler.clone(),
        )?;
        let mr = atlas
            .palette_indices
            .iter()
            .flat_map(|index| {
                let element = &palette.elements[*index as usize];
                [
                    0,
                    (element.roughness.clamp(0.0, 1.0) * 255.0).round() as u8,
                    (element.metalness.clamp(0.0, 1.0) * 255.0).round() as u8,
                    255,
                ]
            })
            .collect::<Vec<_>>();
        let metallic_roughness = self.add_png_texture(
            &format!("{} metallic roughness", model.name),
            atlas.width,
            atlas.height,
            &mr,
            png::ColorType::Rgba,
            self.nearest_sampler.clone(),
        )?;
        let info = |index, tex_coord| json::texture::Info {
            index,
            tex_coord,
            extensions: None,
            extras: None,
        };

        let used = atlas
            .used_palette_indices
            .iter()
            .map(|index| &palette.elements[*index as usize])
            .collect::<Vec<_>>();
        let emissive_strength = used
            .iter()
            .map(|element| element.emission)
            .fold(0.0_f32, f32::max);
        let emissive_texture = if emissive_strength > 0.0 {
            let pixels = atlas
                .palette_indices
                .iter()
                .flat_map(|index| {
                    let element = &palette.elements[*index as usize];
                    let linear = element.color.to_linear() * (element.emission / emissive_strength);
                    Color::LinearRgba(LinearRgba::new(linear.red, linear.green, linear.blue, 1.0))
                        .to_srgba()
                        .to_u8_array()
                })
                .collect::<Vec<_>>();
            Some(self.add_png_texture(
                &format!("{} emissive", model.name),
                atlas.width,
                atlas.height,
                &pixels,
                png::ColorType::Rgba,
                self.nearest_sampler.clone(),
            )?)
        } else {
            None
        };

        let transmission_max = used
            .iter()
            .map(|element| element.translucency)
            .fold(0.0_f32, f32::max);
        let transmission_texture = if transmission_max > 0.0 {
            let pixels = atlas
                .palette_indices
                .iter()
                .map(|index| {
                    (palette.elements[*index as usize]
                        .translucency
                        .clamp(0.0, 1.0)
                        * 255.0)
                        .round() as u8
                })
                .collect::<Vec<_>>();
            Some(self.add_png_texture(
                &format!("{} transmission", model.name),
                atlas.width,
                atlas.height,
                &pixels,
                png::ColorType::Grayscale,
                self.nearest_sampler.clone(),
            )?)
        } else {
            None
        };
        let occlusion_texture = if let Some(ao) = &atlas.ambient_occlusion {
            Some(self.add_png_texture(
                &format!("{} occlusion", model.name),
                ao.width,
                ao.height,
                &ao.data,
                png::ColorType::Grayscale,
                self.linear_sampler.clone(),
            )?)
        } else {
            None
        };

        let mut extensions = json::extensions::material::Material::default();
        let mut extension_names = Vec::new();
        if emissive_strength > 1.0 {
            extensions.emissive_strength = Some(json::extensions::material::EmissiveStrength {
                emissive_strength: json::extensions::material::EmissiveStrengthFactor(
                    emissive_strength,
                ),
            });
            extension_names.push("KHR_materials_emissive_strength");
        }
        if transmission_max > 0.0 {
            extensions.transmission = Some(json::extensions::material::Transmission {
                transmission_factor: json::extensions::material::TransmissionFactor(1.0),
                transmission_texture: transmission_texture.map(|texture| info(texture, 0)),
                extras: None,
            });
            if let Some(ior) = model.average_ior {
                extensions.ior = Some(json::extensions::material::Ior {
                    ior: json::extensions::material::IndexOfRefraction(ior),
                    extras: None,
                });
                extensions.volume = Some(json::extensions::material::Volume {
                    thickness_factor: json::extensions::material::ThicknessFactor(
                        model.data._size().min_element().max(0) as f32
                            * model.data.settings.voxel_size,
                    ),
                    // `gltf-json` represents the specification's default infinite
                    // attenuation distance as `f32::INFINITY`, which serde_json
                    // serializes as `null`. Use the largest finite value so the
                    // generated extension remains valid JSON with equivalent intent.
                    attenuation_distance: json::extensions::material::AttenuationDistance(f32::MAX),
                    ..Default::default()
                });
                extension_names.extend(["KHR_materials_ior", "KHR_materials_volume"]);
            }
            extension_names.push("KHR_materials_transmission");
        }
        for name in extension_names {
            if !self
                .root
                .extensions_used
                .iter()
                .any(|existing| existing == name)
            {
                self.root.extensions_used.push(name.into());
            }
        }

        let material = json::Material {
            name: Some(format!("{} material", model.name)),
            pbr_metallic_roughness: json::material::PbrMetallicRoughness {
                base_color_texture: Some(info(base, 0)),
                metallic_roughness_texture: Some(info(metallic_roughness, 0)),
                ..Default::default()
            },
            emissive_factor: if emissive_texture.is_some() {
                json::material::EmissiveFactor([emissive_strength.min(1.0); 3])
            } else {
                Default::default()
            },
            emissive_texture: emissive_texture.map(|texture| info(texture, 0)),
            occlusion_texture: occlusion_texture.map(|texture| json::material::OcclusionTexture {
                index: texture,
                strength: json::material::StrengthFactor(1.0),
                tex_coord: 1,
                extensions: None,
                extras: None,
            }),
            extensions: (emissive_strength > 1.0 || transmission_max > 0.0).then_some(extensions),
            ..Default::default()
        };
        Ok(self.root.push(material))
    }

    fn add_f32_accessor<const N: usize>(
        &mut self,
        values: &[[f32; N]],
        type_: json::accessor::Type,
        bounds: Option<([f32; 3], [f32; 3])>,
    ) -> Index<json::Accessor> {
        let bytes = values
            .iter()
            .flat_map(|value| value.iter().flat_map(|component| component.to_le_bytes()))
            .collect::<Vec<_>>();
        let view = self.add_view(&bytes, Some(json::buffer::Target::ArrayBuffer));
        self.root.push(json::Accessor {
            buffer_view: Some(view),
            byte_offset: Some(USize64(0)),
            count: USize64::from(values.len()),
            component_type: Valid(json::accessor::GenericComponentType(
                json::accessor::ComponentType::F32,
            )),
            type_: Valid(type_),
            min: bounds.map(|(min, _)| json::Value::from(Vec::from(min))),
            max: bounds.map(|(_, max)| json::Value::from(Vec::from(max))),
            name: None,
            normalized: false,
            sparse: None,
            extensions: None,
            extras: None,
        })
    }

    fn add_indices(&mut self, values: &[u32]) -> Index<json::Accessor> {
        let bytes = values
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect::<Vec<_>>();
        let view = self.add_view(&bytes, Some(json::buffer::Target::ElementArrayBuffer));
        self.root.push(json::Accessor {
            buffer_view: Some(view),
            byte_offset: Some(USize64(0)),
            count: USize64::from(values.len()),
            component_type: Valid(json::accessor::GenericComponentType(
                json::accessor::ComponentType::U32,
            )),
            type_: Valid(json::accessor::Type::Scalar),
            min: None,
            max: None,
            name: None,
            normalized: false,
            sparse: None,
            extensions: None,
            extras: None,
        })
    }

    fn add_png_texture(
        &mut self,
        name: &str,
        width: u32,
        height: u32,
        pixels: &[u8],
        color: png::ColorType,
        sampler: Index<json::texture::Sampler>,
    ) -> Result<Index<json::Texture>, ExportError> {
        let mut encoded = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut encoded, width, height);
            encoder.set_color(color);
            encoder.set_depth(png::BitDepth::Eight);
            let mut png_writer = encoder.write_header()?;
            png_writer.write_image_data(pixels)?;
        }
        let view = self.add_view(&encoded, None);
        let image = self.root.push(json::Image {
            buffer_view: Some(view),
            mime_type: Some(json::image::MimeType("image/png".into())),
            name: Some(name.into()),
            uri: None,
            extensions: None,
            extras: None,
        });
        Ok(self.root.push(json::Texture {
            name: Some(name.into()),
            sampler: Some(sampler),
            source: image,
            extensions: None,
            extras: None,
        }))
    }

    fn add_view(
        &mut self,
        bytes: &[u8],
        target: Option<json::buffer::Target>,
    ) -> Index<json::buffer::View> {
        align_four(&mut self.bin, 0);
        let offset = self.bin.len();
        self.bin.extend_from_slice(bytes);
        let length = bytes.len();
        align_four(&mut self.bin, 0);
        self.root.push(json::buffer::View {
            buffer: self.buffer.clone(),
            byte_length: USize64::from(length),
            byte_offset: Some(USize64::from(offset)),
            byte_stride: None,
            name: None,
            target: target.map(Valid),
            extensions: None,
            extras: None,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn add_scene_node(
        &mut self,
        graph: &[SceneNode],
        node: &SceneNode,
        models: &[BakedModel],
        meshes: &[Option<Index<json::Mesh>>],
        layers: &[dot_vox::Layer],
        frame_index: usize,
        scale: f32,
        is_root: bool,
    ) -> Vec<Index<json::Node>> {
        match node {
            SceneNode::Group { children, .. } => children
                .iter()
                .filter_map(|child| graph.get(*child as usize))
                .flat_map(|child| {
                    self.add_scene_node(
                        graph,
                        child,
                        models,
                        meshes,
                        layers,
                        frame_index,
                        scale,
                        false,
                    )
                })
                .collect(),
            SceneNode::Shape { models: frames, .. } => {
                let Some(shape_model) = frames.get(frame_index.min(frames.len().saturating_sub(1)))
                else {
                    return Vec::new();
                };
                let model_index = shape_model.model_id as usize;
                let Some(Some(mesh)) = meshes.get(model_index) else {
                    return Vec::new();
                };
                vec![self.root.push(json::Node {
                    mesh: Some(mesh.clone()),
                    name: models.get(model_index).map(|model| model.name.clone()),
                    ..Default::default()
                })]
            }
            SceneNode::Transform {
                attributes,
                frames,
                child,
                layer_id,
            } => {
                let hidden = attributes.get("_hidden").is_some_and(|value| value == "1")
                    || layers
                        .get(*layer_id as usize)
                        .is_some_and(dot_vox::Layer::hidden);
                if hidden {
                    return Vec::new();
                }
                let children = graph.get(*child as usize).map_or_else(Vec::new, |child| {
                    self.add_scene_node(
                        graph,
                        child,
                        models,
                        meshes,
                        layers,
                        frame_index,
                        scale,
                        false,
                    )
                });
                let matrix = if is_root {
                    Mat4::IDENTITY
                } else {
                    frames
                        .get(frame_index.min(frames.len().saturating_sub(1)))
                        .map_or(Mat4::IDENTITY, |frame| transform_from_frame(frame, scale))
                };
                vec![self.root.push(json::Node {
                    children: (!children.is_empty()).then_some(children),
                    matrix: (matrix != Mat4::IDENTITY).then_some(matrix.to_cols_array()),
                    name: attributes.get("_name").cloned(),
                    ..Default::default()
                })]
            }
        }
    }

    fn finish(mut self) -> Result<Vec<u8>, ExportError> {
        self.root.buffers[0].byte_length = USize64::from(self.bin.len());
        let mut json_bytes = json::serialize::to_vec(&self.root)?;
        align_four(&mut json_bytes, b' ');
        align_four(&mut self.bin, 0);
        let length = 12usize
            .checked_add(8 + json_bytes.len())
            .and_then(|value| value.checked_add(8 + self.bin.len()))
            .ok_or(ExportError::TooLarge)?;
        let header_length = u32::try_from(length).map_err(|_| ExportError::TooLarge)?;
        let glb = binary::Glb {
            header: binary::Header {
                magic: *b"glTF",
                version: 2,
                length: header_length,
            },
            json: Cow::Owned(json_bytes),
            bin: Some(Cow::Owned(self.bin)),
        };
        glb.to_vec().map_err(ExportError::from)
    }
}

fn mesh_vec3(
    mesh: &Mesh,
    attribute: bevy::mesh::MeshVertexAttribute,
    name: &str,
) -> Result<Vec<[f32; 3]>, ExportError> {
    match mesh.attribute(attribute) {
        Some(VertexAttributeValues::Float32x3(values)) => Ok(values.clone()),
        _ => Err(ExportError::MeshAttribute {
            mesh: name.into(),
            attribute: "vec3 vertex attribute",
        }),
    }
}

fn mesh_vec2(
    mesh: &Mesh,
    attribute: bevy::mesh::MeshVertexAttribute,
    name: &str,
) -> Result<Vec<[f32; 2]>, ExportError> {
    match mesh.attribute(attribute) {
        Some(VertexAttributeValues::Float32x2(values)) => Ok(values.clone()),
        _ => Err(ExportError::MeshAttribute {
            mesh: name.into(),
            attribute: "vec2 vertex attribute",
        }),
    }
}

fn bounds(points: &[[f32; 3]]) -> ([f32; 3], [f32; 3]) {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for point in points {
        for axis in 0..3 {
            min[axis] = min[axis].min(point[axis]);
            max[axis] = max[axis].max(point[axis]);
        }
    }
    (min, max)
}

fn align_four(bytes: &mut Vec<u8>, value: u8) {
    while !bytes.len().is_multiple_of(4) {
        bytes.push(value);
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, io::Cursor};

    use super::{ExportError, export_vox_to_glb};
    use crate::{AmbientOcclusionSettings, VoxLoaderSettings, VoxelTextureMode};

    fn validate_glb(bytes: &[u8]) -> gltf::Gltf {
        let gltf = gltf::Gltf::from_slice(bytes).expect("generated GLB should parse and validate");
        let blob = gltf
            .blob
            .as_deref()
            .expect("GLB should contain a binary chunk");
        let buffer = gltf
            .document
            .buffers()
            .next()
            .expect("GLB should declare its binary buffer");
        assert_eq!(buffer.length(), blob.len());
        assert_eq!(gltf.document.buffers().count(), 1);
        assert!(gltf.document.default_scene().is_some());

        for view in gltf.document.views() {
            assert_eq!(view.offset() % 4, 0, "buffer view should be aligned");
            assert!(view.offset() + view.length() <= blob.len());
        }
        for accessor in gltf.document.accessors() {
            let view = accessor
                .view()
                .expect("exported accessors should have views");
            assert!(accessor.offset() + accessor.count() * accessor.size() <= view.length());
        }
        for image in gltf.document.images() {
            let gltf::image::Source::View { view, mime_type } = image.source() else {
                panic!("exported images should be embedded buffer views")
            };
            assert_eq!(mime_type, "image/png");
            let encoded = &blob[view.offset()..view.offset() + view.length()];
            let mut reader = png::Decoder::new(Cursor::new(encoded))
                .read_info()
                .expect("embedded image should have a valid PNG header");
            let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
            reader
                .next_frame(&mut pixels)
                .expect("embedded PNG should decode");
        }
        for mesh in gltf.document.meshes() {
            for primitive in mesh.primitives() {
                assert_eq!(primitive.mode(), gltf::mesh::Mode::Triangles);
                assert!(primitive.indices().is_some());
                assert!(primitive.get(&gltf::Semantic::Positions).is_some());
                assert!(primitive.get(&gltf::Semantic::Normals).is_some());
                assert!(primitive.get(&gltf::Semantic::TexCoords(0)).is_some());
                assert!(primitive.material().index().is_some());
            }
        }
        gltf
    }

    fn png_dimensions(gltf: &gltf::Gltf) -> Vec<(u32, u32)> {
        let blob = gltf.blob.as_deref().unwrap();
        gltf.document
            .images()
            .map(|image| {
                let gltf::image::Source::View { view, .. } = image.source() else {
                    panic!("expected embedded image")
                };
                let encoded = &blob[view.offset()..view.offset() + view.length()];
                let reader = png::Decoder::new(Cursor::new(encoded)).read_info().unwrap();
                (reader.info().width, reader.info().height)
            })
            .collect()
    }

    #[test]
    fn exports_valid_glb() {
        let bytes = include_bytes!("../assets/deer.vox");
        let exports = export_vox_to_glb(bytes, &VoxLoaderSettings::default()).unwrap();
        assert_eq!(exports.len(), 4);
        assert_eq!(
            exports
                .iter()
                .map(|export| export.frame_index)
                .collect::<Vec<_>>(),
            [0, 1, 2, 3]
        );
        for export in exports {
            let gltf = validate_glb(&export.bytes);
            assert!(gltf.document.meshes().next().is_some());
        }
    }

    #[test]
    fn export_is_byte_deterministic() {
        let bytes = include_bytes!("../assets/deer.vox");
        let first = export_vox_to_glb(bytes, &VoxLoaderSettings::default()).unwrap();
        let second = export_vox_to_glb(bytes, &VoxLoaderSettings::default()).unwrap();
        assert_eq!(
            first.iter().map(|export| &export.bytes).collect::<Vec<_>>(),
            second
                .iter()
                .map(|export| &export.bytes)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn animation_frames_are_distinct_static_snapshots() {
        let exports = export_vox_to_glb(
            include_bytes!("../assets/deer.vox"),
            &VoxLoaderSettings::default(),
        )
        .unwrap();
        assert_eq!(exports.len(), 4);
        assert!(
            exports
                .windows(2)
                .all(|pair| pair[0].bytes != pair[1].bytes)
        );
        for (index, export) in exports.iter().enumerate() {
            let gltf = validate_glb(&export.bytes);
            assert_eq!(gltf.document.animations().count(), 0);
            let scene_name = gltf.document.default_scene().unwrap().name().unwrap();
            if index == 0 {
                assert_eq!(scene_name, "Voxel scene");
            } else {
                assert_eq!(scene_name, format!("Voxel scene frame {}", index + 1));
            }
        }
    }

    #[test]
    fn palette_mode_embeds_16_by_16_material_textures() {
        let settings = VoxLoaderSettings {
            texture_mode: VoxelTextureMode::Palette,
            ..Default::default()
        };
        let exports = export_vox_to_glb(include_bytes!("../assets/deer.vox"), &settings).unwrap();
        let gltf = validate_glb(&exports[0].bytes);
        let dimensions = png_dimensions(&gltf);
        assert!(!dimensions.is_empty());
        assert!(dimensions.iter().all(|dimensions| *dimensions == (16, 16)));
        assert!(gltf.document.materials().all(|material| {
            let pbr = material.pbr_metallic_roughness();
            pbr.base_color_texture().is_some() && pbr.metallic_roughness_texture().is_some()
        }));
    }

    #[test]
    fn exports_emissive_and_transmissive_material_extensions() {
        let exports = export_vox_to_glb(
            include_bytes!("../assets/study.vox"),
            &VoxLoaderSettings::default(),
        )
        .unwrap();
        assert_eq!(exports.len(), 1);
        let gltf = validate_glb(&exports[0].bytes);
        let used = gltf.document.extensions_used().collect::<Vec<_>>();
        for extension in [
            "KHR_materials_emissive_strength",
            "KHR_materials_transmission",
            "KHR_materials_ior",
            "KHR_materials_volume",
        ] {
            assert!(used.contains(&extension), "missing {extension}");
        }
        assert!(gltf.document.materials().any(|material| {
            material.emissive_texture().is_some()
                && material
                    .emissive_strength()
                    .is_some_and(|strength| strength > 1.0)
        }));
        assert!(gltf.document.materials().any(|material| {
            material.transmission().is_some()
                && material.ior().is_some()
                && material.volume().is_some()
        }));
    }

    #[test]
    fn preserves_named_hierarchy_and_shared_mesh_references() {
        let exports = export_vox_to_glb(
            include_bytes!("../assets/study.vox"),
            &VoxLoaderSettings::default(),
        )
        .unwrap();
        let gltf = validate_glb(&exports[0].bytes);
        let names = gltf
            .document
            .nodes()
            .filter_map(|node| node.name())
            .collect::<Vec<_>>();
        assert!(names.contains(&"workstation"));
        assert!(names.contains(&"workstation/desk"));
        assert!(names.contains(&"tank/wall"));

        let mut reference_counts = HashMap::new();
        for mesh in gltf.document.nodes().filter_map(|node| node.mesh()) {
            *reference_counts.entry(mesh.index()).or_insert(0usize) += 1;
        }
        assert!(reference_counts.values().any(|count| *count > 1));
    }

    #[test]
    fn rejects_cloud_volumes_with_the_model_name() {
        let error = export_vox_to_glb(
            include_bytes!("../assets/cloud.vox"),
            &VoxLoaderSettings::default(),
        )
        .unwrap_err();
        assert!(matches!(error, ExportError::UnsupportedCloud(_)));
        assert!(error.to_string().contains("cloud"));
    }

    #[test]
    fn exports_ambient_occlusion_texture_on_uv1() {
        let bytes = include_bytes!("../assets/deer.vox");
        let settings = VoxLoaderSettings {
            ambient_occlusion: Some(AmbientOcclusionSettings {
                depth: 1,
                intensity: 0.75,
            }),
            ..Default::default()
        };
        let exports = export_vox_to_glb(bytes, &settings).unwrap();
        let gltf = validate_glb(&exports[0].bytes);
        assert!(gltf.document.materials().any(|material| {
            material
                .occlusion_texture()
                .is_some_and(|texture| texture.tex_coord() == 1)
        }));
        assert!(gltf.document.meshes().any(|mesh| {
            mesh.primitives()
                .any(|primitive| primitive.get(&gltf::Semantic::TexCoords(1)).is_some())
        }));
    }
}
