//! Offline conversion of MagicaVoxel files to binary glTF.

use std::{borrow::Cow, collections::BTreeMap};

use gltf::{binary, json};
use json::{
    Index,
    validation::{Checked::Valid, USize64},
};
use thiserror::Error;

use crate::{
    BakeError, BakedMeshAtlas as MeshAtlas, BakedPalette, BakedScene, BakedSceneGraphNode,
    BakedSceneModel, SceneBakeError, VoxBakeSettings, bake_scene,
};

/// One GLB produced by [`export_vox_to_glb`].
#[derive(Clone, Debug)]
pub struct ExportedGlb {
    /// Complete binary glTF file contents.
    pub bytes: Vec<u8>,
    /// Counts describing the generated frame.
    pub statistics: ExportStatistics,
    /// Lossy conversions applied while producing this frame.
    pub warnings: Vec<ExportWarning>,
}

/// Counts reported for one exported GLB frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ExportStatistics {
    /// Baked voxel models stored in the document.
    pub models: usize,
    /// Surface meshes stored in the document.
    pub meshes: usize,
    /// Triangles stored across all meshes.
    pub triangles: usize,
    /// Embedded material images stored in the document.
    pub textures: usize,
    /// Complete GLB size in bytes.
    pub output_bytes: usize,
}

/// A supported conversion that cannot preserve all source data in glTF.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExportWarning {
    /// Packed atlas mip levels were requested, but PNG stores only the base level.
    BaseMipOnly {
        /// Highest mip level requested by the atlas settings.
        requested_max_lod: u8,
    },
    /// Animated transform tracks were reduced to their first transform.
    TransformAnimationFirstFrame {
        /// Number of transform nodes containing more than one frame.
        node_count: usize,
    },
}

impl std::fmt::Display for ExportWarning {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BaseMipOnly { requested_max_lod } => write!(
                formatter,
                "requested atlas mip levels through LOD {requested_max_lod}, but GLB PNG images contain only the base level; generate mipmaps when loading"
            ),
            Self::TransformAnimationFirstFrame { node_count } => write!(
                formatter,
                "{node_count} animated transform track(s) were reduced to their first transform; voxel model frames remain available as sibling nodes"
            ),
        }
    }
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
    /// The voxel model could not be baked with the requested settings.
    #[error("could not bake voxel model: {0}")]
    Bake(#[from] BakeError),
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

/// Convert MagicaVoxel bytes into one or more complete GLB files.
///
/// Every input returns one item. Animated shapes become sibling mesh nodes under
/// their shared transform, named with `-001`, `-002`, … suffixes. Frame one is
/// initially visible and later frames use `KHR_node_visibility` with `visible: false`.
pub fn export_vox_to_glb(
    bytes: &[u8],
    settings: &VoxBakeSettings,
) -> Result<ExportedGlb, ExportError> {
    let file = crate::source::parse_vox(bytes).map_err(ExportError::InvalidVox)?;
    let scene = bake_scene(&file, settings).map_err(|error| match error {
        SceneBakeError::MissingScene => ExportError::MissingScene,
        SceneBakeError::Model { source, .. } => ExportError::Bake(source),
    })?;
    if let Some(model) = scene
        .models
        .iter()
        .find(|model| model.baked.volume.is_some())
    {
        return Err(ExportError::UnsupportedCloud(model.name.clone()));
    }

    let warnings = export_warnings(&scene, settings);
    let bytes = write_scene(&scene, settings.voxel_size)?;
    let statistics = export_statistics(&scene, bytes.len());
    Ok(ExportedGlb {
        bytes,
        statistics,
        warnings,
    })
}

fn export_warnings(scene: &BakedScene, settings: &VoxBakeSettings) -> Vec<ExportWarning> {
    let mut warnings = match settings.texture_mode {
        crate::VoxelTextureMode::Packed(crate::TexturePackSettings {
            mip_policy: crate::TextureMipPolicy::Limited { max_lod },
            ..
        }) if max_lod > 0 => vec![ExportWarning::BaseMipOnly {
            requested_max_lod: max_lod,
        }],
        _ => Vec::new(),
    };
    let animated_transforms = scene
        .nodes
        .iter()
        .filter(|node| {
            matches!(
                node,
                BakedSceneGraphNode::Transform { frames, .. } if frames.len() > 1
            )
        })
        .count();
    if animated_transforms > 0 {
        warnings.push(ExportWarning::TransformAnimationFirstFrame {
            node_count: animated_transforms,
        });
    }
    warnings
}

fn export_statistics(scene: &BakedScene, output_bytes: usize) -> ExportStatistics {
    let surfaces = scene
        .models
        .iter()
        .filter_map(|model| model.baked.surface.as_ref())
        .collect::<Vec<_>>();
    ExportStatistics {
        models: scene.models.len(),
        meshes: surfaces.len(),
        triangles: surfaces
            .iter()
            .map(|surface| surface.mesh.indices.len() / 3)
            .sum(),
        textures: surfaces
            .iter()
            .map(|surface| {
                let used = surface
                    .used_palette_indices
                    .iter()
                    .map(|index| &scene.palette.materials[*index as usize])
                    .collect::<Vec<_>>();
                2 + usize::from(used.iter().any(|material| material.emission > 0.0))
                    + usize::from(used.iter().any(|material| material.translucency > 0.0))
                    + usize::from(surface.ambient_occlusion.is_some())
            })
            .sum(),
        output_bytes,
    }
}

fn write_scene(scene: &BakedScene, voxel_size: f32) -> Result<Vec<u8>, ExportError> {
    let mut writer = GltfWriter::new();
    let mut gltf_meshes = Vec::with_capacity(scene.models.len());
    for model in &scene.models {
        gltf_meshes.push(match &model.baked.surface {
            Some(atlas) => Some(writer.add_model(model, atlas, &scene.palette, voxel_size)?),
            None => None,
        });
    }

    let snapshot = export_scene_nodes(scene);
    let roots = snapshot
        .iter()
        .filter_map(|node| writer.add_scene_node(node, &gltf_meshes))
        .collect();
    let scene = writer.root.push(json::Scene {
        name: Some("Voxel scene".into()),
        nodes: roots,
        extensions: None,
        extras: None,
    });
    writer.root.scene = Some(scene);
    writer.finish()
}

#[derive(Clone, Debug)]
struct ExportSceneNode {
    name: Option<String>,
    transform: [f32; 16],
    model_index: Option<usize>,
    children: Vec<ExportSceneNode>,
    animation_frame: Option<usize>,
    animation_frame_count: Option<usize>,
    visible: bool,
}

fn export_scene_nodes(scene: &BakedScene) -> Vec<ExportSceneNode> {
    export_scene_node(scene, scene.root, true, None)
}

fn export_scene_node(
    scene: &BakedScene,
    node_index: usize,
    is_root: bool,
    animation_name: Option<&str>,
) -> Vec<ExportSceneNode> {
    let Some(node) = scene.nodes.get(node_index) else {
        return Vec::new();
    };
    match node {
        BakedSceneGraphNode::Group { children } => children
            .iter()
            .flat_map(|child| export_scene_node(scene, *child, false, animation_name))
            .collect(),
        BakedSceneGraphNode::Shape { models } => models
            .iter()
            .enumerate()
            .filter_map(|(frame_index, model_index)| {
                let model = scene.models.get(*model_index)?;
                let animated = models.len() > 1;
                let base_name = animation_name.unwrap_or(&model.name);
                Some(ExportSceneNode {
                    name: Some(if animated {
                        format!("{base_name}-{:03}", frame_index + 1)
                    } else {
                        model.name.clone()
                    }),
                    transform: glam_identity(),
                    model_index: Some(*model_index),
                    children: Vec::new(),
                    animation_frame: animated.then_some(frame_index),
                    animation_frame_count: None,
                    visible: frame_index == 0,
                })
            })
            .collect(),
        BakedSceneGraphNode::Transform {
            name,
            hidden,
            layer_index,
            child,
            frames,
        } => {
            if *hidden
                || scene
                    .layers
                    .get(*layer_index)
                    .is_some_and(|layer| layer.hidden)
            {
                return Vec::new();
            }
            let children = export_scene_node(scene, *child, false, name.as_deref());
            let animation_frame_count = children
                .iter()
                .filter(|child| child.animation_frame.is_some())
                .count();
            vec![ExportSceneNode {
                name: name.clone(),
                transform: if is_root {
                    glam_identity()
                } else {
                    frames.first().copied().unwrap_or_else(glam_identity)
                },
                model_index: None,
                children,
                animation_frame: None,
                animation_frame_count: (animation_frame_count > 1).then_some(animation_frame_count),
                visible: true,
            }]
        }
    }
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
        root.asset.generator = Some(format!("vox_bake {}", env!("CARGO_PKG_VERSION")));
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
        model: &BakedSceneModel,
        atlas: &MeshAtlas,
        palette: &BakedPalette,
        voxel_size: f32,
    ) -> Result<Index<json::Mesh>, ExportError> {
        let positions = &atlas.mesh.positions;
        let normals = &atlas.mesh.normals;
        let uvs = &atlas.mesh.uv0;
        let ao_uvs = atlas.mesh.uv1.as_ref();
        let indices = &atlas.mesh.indices;

        let (min, max) = bounds(positions);
        let position_accessor =
            self.add_f32_accessor(&positions, json::accessor::Type::Vec3, Some((min, max)));
        let normal_accessor = self.add_f32_accessor(&normals, json::accessor::Type::Vec3, None);
        let uv_accessor = self.add_f32_accessor(&uvs, json::accessor::Type::Vec2, None);
        let ao_accessor = ao_uvs
            .as_ref()
            .map(|values| self.add_f32_accessor(values, json::accessor::Type::Vec2, None));
        let index_accessor = self.add_indices(indices);
        let material = self.add_material(model, atlas, palette, voxel_size)?;
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
        model: &BakedSceneModel,
        atlas: &MeshAtlas,
        palette: &BakedPalette,
        voxel_size: f32,
    ) -> Result<Index<json::Material>, ExportError> {
        let rgba = atlas
            .palette_indices
            .iter()
            .flat_map(|index| palette.materials[*index as usize].srgba_u8(palette.uses_srgb))
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
                let element = &palette.materials[*index as usize];
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
            .map(|index| &palette.materials[*index as usize])
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
                    palette.materials[*index as usize]
                        .emissive_srgba_u8(palette.uses_srgb, emissive_strength)
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
                    (palette.materials[*index as usize]
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
                &ao.pixels,
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
            if let Some(ior) = model.baked.average_ior {
                extensions.ior = Some(json::extensions::material::Ior {
                    ior: json::extensions::material::IndexOfRefraction(ior),
                    extras: None,
                });
                extensions.volume = Some(json::extensions::material::Volume {
                    thickness_factor: json::extensions::material::ThicknessFactor(
                        model.baked.dimensions.into_iter().min().unwrap_or(0) as f32 * voxel_size,
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

    fn add_scene_node(
        &mut self,
        node: &ExportSceneNode,
        meshes: &[Option<Index<json::Mesh>>],
    ) -> Option<Index<json::Node>> {
        let mesh = match node.model_index {
            Some(model_index) => Some(meshes.get(model_index)?.as_ref()?.clone()),
            None => None,
        };
        let children = node
            .children
            .iter()
            .filter_map(|child| self.add_scene_node(child, meshes))
            .collect::<Vec<_>>();
        let (extensions, extras) = if let Some(frame_index) = node.animation_frame {
            let mut extensions = json::extensions::scene::Node::default();
            extensions.others.insert(
                "KHR_node_visibility".into(),
                serde_json::json!({ "visible": node.visible }),
            );
            if !self
                .root
                .extensions_used
                .iter()
                .any(|name| name == "KHR_node_visibility")
            {
                self.root.extensions_used.push("KHR_node_visibility".into());
            }
            let extras = serde_json::value::to_raw_value(&serde_json::json!({
                "vox_bake": {
                    "version": 1,
                    "animation_frame": {
                        "index": frame_index,
                        "initially_visible": node.visible
                    }
                }
            }))
            .expect("static voxel animation extras should serialize");
            (Some(extensions), Some(extras))
        } else if let Some(frame_count) = node.animation_frame_count {
            let extras = serde_json::value::to_raw_value(&serde_json::json!({
                "vox_bake": {
                    "version": 1,
                    "animation": {
                        "frame_count": frame_count
                    }
                }
            }))
            .expect("static voxel animation extras should serialize");
            (None, Some(extras))
        } else {
            (None, None)
        };
        Some(self.root.push(json::Node {
            children: (!children.is_empty()).then_some(children),
            extensions,
            extras,
            matrix: (node.transform != glam_identity()).then_some(node.transform),
            mesh,
            name: node.name.clone(),
            camera: None,
            rotation: None,
            scale: None,
            skin: None,
            translation: None,
            weights: None,
        }))
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

fn glam_identity() -> [f32; 16] {
    [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ]
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
    use std::{collections::HashMap, io::Cursor, path::Path};

    use super::{ExportError, export_scene_nodes, export_vox_to_glb, glam_identity};
    use crate::{
        AmbientOcclusionSettings, BakedLayer, BakedMaterial, BakedScene, BakedSceneGraphNode,
        BakedSceneModel, VoxBakeSettings, VoxelGrid, VoxelTextureMode, bake_model,
    };

    fn fixture(name: &str) -> Vec<u8> {
        std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../assets")
                .join(name),
        )
        .expect("workspace test fixture should exist")
    }

    fn empty_model(name: &str) -> BakedSceneModel {
        let grid = VoxelGrid::empty([1; 3], true);
        let palette = crate::BakedPalette::new(vec![BakedMaterial::default()], true);
        let baked = bake_model(&grid, &palette, &VoxBakeSettings::default()).unwrap();
        BakedSceneModel {
            name: name.into(),
            grid,
            baked,
        }
    }

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

    fn accessor_bytes<'a>(gltf: &'a gltf::Gltf, accessor: &gltf::Accessor<'_>) -> &'a [u8] {
        let view = accessor.view().expect("exported accessor view");
        let start = view.offset() + accessor.offset();
        &gltf.blob.as_deref().unwrap()[start..start + accessor.count() * accessor.size()]
    }

    fn accessor_f32<const N: usize>(
        gltf: &gltf::Gltf,
        accessor: gltf::Accessor<'_>,
    ) -> Vec<[f32; N]> {
        accessor_bytes(gltf, &accessor)
            .chunks_exact(N * std::mem::size_of::<f32>())
            .map(|item| {
                std::array::from_fn(|component| {
                    let start = component * std::mem::size_of::<f32>();
                    f32::from_le_bytes(item[start..start + 4].try_into().unwrap())
                })
            })
            .collect()
    }

    fn decode_png(gltf: &gltf::Gltf, image_name: &str) -> Vec<u8> {
        let image = gltf
            .document
            .images()
            .find(|image| image.name() == Some(image_name))
            .expect("named exported image");
        let gltf::image::Source::View { view, .. } = image.source() else {
            panic!("expected embedded image")
        };
        let blob = gltf.blob.as_deref().unwrap();
        let encoded = &blob[view.offset()..view.offset() + view.length()];
        let mut reader = png::Decoder::new(Cursor::new(encoded)).read_info().unwrap();
        let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut pixels).unwrap();
        pixels.truncate(info.buffer_size());
        pixels
    }

    #[test]
    fn exports_valid_glb() {
        let bytes = fixture("deer.vox");
        let export = export_vox_to_glb(&bytes, &VoxBakeSettings::default()).unwrap();
        let gltf = validate_glb(&export.bytes);
        assert!(gltf.document.meshes().next().is_some());
    }

    #[test]
    fn export_is_byte_deterministic() {
        let bytes = fixture("deer.vox");
        let first = export_vox_to_glb(&bytes, &VoxBakeSettings::default()).unwrap();
        let second = export_vox_to_glb(&bytes, &VoxBakeSettings::default()).unwrap();
        assert_eq!(first.bytes, second.bytes);
    }

    #[test]
    fn animation_frames_are_sibling_nodes_with_initial_visibility() {
        let bytes = fixture("deer.vox");
        let source = crate::source::parse_vox(&bytes).unwrap();
        let expected_frames = source
            .scenes
            .iter()
            .filter_map(|node| match node {
                dot_vox::SceneNode::Shape { models, .. } if models.len() > 1 => Some(models.len()),
                _ => None,
            })
            .sum::<usize>();
        let export = export_vox_to_glb(&bytes, &VoxBakeSettings::default()).unwrap();
        let gltf = validate_glb(&export.bytes);
        assert_eq!(gltf.document.animations().count(), 0);
        assert_eq!(
            gltf.document.default_scene().unwrap().name(),
            Some("Voxel scene")
        );
        assert!(
            gltf.document
                .extensions_used()
                .any(|name| name == "KHR_node_visibility")
        );

        let frame_nodes = gltf
            .document
            .nodes()
            .filter(|node| node.extension_value("KHR_node_visibility").is_some())
            .collect::<Vec<_>>();
        assert_eq!(frame_nodes.len(), expected_frames);
        assert!(frame_nodes.iter().all(|node| {
            let name = node.name().expect("animation frame name");
            name.rsplit_once('-').is_some_and(|(_, suffix)| {
                suffix.len() >= 3 && suffix.chars().all(|character| character.is_ascii_digit())
            })
        }));

        for parent in gltf.document.nodes() {
            let frames = parent
                .children()
                .filter(|node| node.extension_value("KHR_node_visibility").is_some())
                .collect::<Vec<_>>();
            if frames.is_empty() {
                continue;
            }
            let parent_extras: serde_json::Value = serde_json::from_str(
                parent
                    .extras()
                    .as_ref()
                    .expect("animated parent extras")
                    .get(),
            )
            .unwrap();
            assert_eq!(parent_extras["vox_bake"]["version"], 1);
            assert_eq!(
                parent_extras["vox_bake"]["animation"]["frame_count"],
                frames.len()
            );
            assert_eq!(
                frames
                    .iter()
                    .filter(|node| {
                        node.extension_value("KHR_node_visibility")
                            .and_then(|value| value.get("visible"))
                            .and_then(serde_json::Value::as_bool)
                            == Some(true)
                    })
                    .count(),
                1
            );
            assert!(frames.iter().skip(1).all(|node| {
                node.extension_value("KHR_node_visibility")
                    .and_then(|value| value.get("visible"))
                    .and_then(serde_json::Value::as_bool)
                    == Some(false)
            }));
            for (index, frame) in frames.iter().enumerate() {
                let extras: serde_json::Value = serde_json::from_str(
                    frame
                        .extras()
                        .as_ref()
                        .expect("animation frame extras")
                        .get(),
                )
                .unwrap();
                assert_eq!(extras["vox_bake"]["version"], 1);
                assert_eq!(extras["vox_bake"]["animation_frame"]["index"], index);
                assert_eq!(
                    extras["vox_bake"]["animation_frame"]["initially_visible"],
                    index == 0
                );
            }
            assert!(frames.iter().all(|node| node.transform().matrix()
                == [
                    [1.0, 0.0, 0.0, 0.0],
                    [0.0, 1.0, 0.0, 0.0],
                    [0.0, 0.0, 1.0, 0.0],
                    [0.0, 0.0, 0.0, 1.0],
                ]));
        }
    }

    #[test]
    fn animated_shapes_keep_independent_frame_counts() {
        let scene = BakedScene {
            palette: crate::BakedPalette::new(vec![BakedMaterial::default()], true),
            models: (0..5)
                .map(|index| empty_model(&format!("model-{index}")))
                .collect(),
            nodes: vec![
                BakedSceneGraphNode::Transform {
                    name: Some("root".into()),
                    hidden: false,
                    layer_index: 0,
                    child: 1,
                    frames: Vec::new(),
                },
                BakedSceneGraphNode::Group {
                    children: vec![2, 4],
                },
                BakedSceneGraphNode::Transform {
                    name: Some("short".into()),
                    hidden: false,
                    layer_index: 0,
                    child: 3,
                    frames: vec![glam_identity()],
                },
                BakedSceneGraphNode::Shape { models: vec![0, 1] },
                BakedSceneGraphNode::Transform {
                    name: Some("long".into()),
                    hidden: false,
                    layer_index: 0,
                    child: 5,
                    frames: vec![glam_identity()],
                },
                BakedSceneGraphNode::Shape {
                    models: vec![2, 3, 4],
                },
            ],
            root: 0,
            layers: vec![BakedLayer {
                name: None,
                hidden: false,
            }],
        };

        let root = export_scene_nodes(&scene).remove(0);
        assert_eq!(root.children[0].children.len(), 2);
        assert_eq!(root.children[1].children.len(), 3);
        assert_eq!(
            root.children[0].children[0].name.as_deref(),
            Some("short-001")
        );
        assert_eq!(
            root.children[0].children[1].name.as_deref(),
            Some("short-002")
        );
        assert_eq!(
            root.children[1].children[2].name.as_deref(),
            Some("long-003")
        );
        assert!(root.children[0].children[0].visible);
        assert!(!root.children[0].children[1].visible);
        assert!(root.children[1].children[0].visible);
        assert!(
            root.children[1].children[1..]
                .iter()
                .all(|frame| !frame.visible)
        );
    }

    #[test]
    fn palette_mode_embeds_16_by_16_material_textures() {
        let settings = VoxBakeSettings {
            texture_mode: VoxelTextureMode::Palette,
            ..Default::default()
        };
        let export = export_vox_to_glb(&fixture("deer.vox"), &settings).unwrap();
        let gltf = validate_glb(&export.bytes);
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
        let export = export_vox_to_glb(&fixture("study.vox"), &VoxBakeSettings::default()).unwrap();
        let gltf = validate_glb(&export.bytes);
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
        let export = export_vox_to_glb(&fixture("study.vox"), &VoxBakeSettings::default()).unwrap();
        let gltf = validate_glb(&export.bytes);
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
        let error =
            export_vox_to_glb(&fixture("cloud.vox"), &VoxBakeSettings::default()).unwrap_err();
        assert!(matches!(error, ExportError::UnsupportedCloud(_)));
        assert!(error.to_string().contains("cloud"));
    }

    #[test]
    fn exports_ambient_occlusion_texture_on_uv1() {
        let bytes = fixture("deer.vox");
        let settings = VoxBakeSettings {
            ambient_occlusion: Some(AmbientOcclusionSettings {
                depth: 1,
                intensity: 0.75,
            }),
            ..Default::default()
        };
        let export = export_vox_to_glb(&bytes, &settings).unwrap();
        let gltf = validate_glb(&export.bytes);
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

    #[test]
    fn glb_meshes_and_material_pixels_match_the_shared_bake() {
        let bytes = fixture("deer.vox");
        let settings = VoxBakeSettings {
            ambient_occlusion: Some(AmbientOcclusionSettings {
                depth: 1,
                intensity: 0.75,
            }),
            ..Default::default()
        };
        let source = crate::source::parse_vox(&bytes).unwrap();
        let scene = crate::bake_scene(&source, &settings).unwrap();
        let export = export_vox_to_glb(&bytes, &settings).unwrap();
        let gltf = validate_glb(&export.bytes);

        for model in &scene.models {
            let Some(atlas) = &model.baked.surface else {
                continue;
            };
            let mesh = gltf
                .document
                .meshes()
                .find(|mesh| mesh.name() == Some(model.name.as_str()))
                .expect("mesh named after baked model");
            let primitive = mesh.primitives().next().unwrap();
            assert_eq!(
                accessor_f32::<3>(&gltf, primitive.get(&gltf::Semantic::Positions).unwrap()),
                atlas.mesh.positions
            );
            assert_eq!(
                accessor_f32::<3>(&gltf, primitive.get(&gltf::Semantic::Normals).unwrap()),
                atlas.mesh.normals
            );
            assert_eq!(
                accessor_f32::<2>(&gltf, primitive.get(&gltf::Semantic::TexCoords(0)).unwrap()),
                atlas.mesh.uv0
            );
            assert_eq!(
                accessor_f32::<2>(&gltf, primitive.get(&gltf::Semantic::TexCoords(1)).unwrap()),
                atlas.mesh.uv1.clone().unwrap()
            );
            let indices = accessor_bytes(&gltf, &primitive.indices().unwrap())
                .chunks_exact(4)
                .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()))
                .collect::<Vec<_>>();
            assert_eq!(indices, atlas.mesh.indices);

            let expected_base = atlas
                .palette_indices
                .iter()
                .flat_map(|index| {
                    scene.palette.materials[*index as usize].srgba_u8(scene.palette.uses_srgb)
                })
                .collect::<Vec<_>>();
            assert_eq!(
                decode_png(&gltf, &format!("{} base color", model.name)),
                expected_base
            );
            assert_eq!(
                decode_png(&gltf, &format!("{} base color", model.name))
                    .chunks_exact(4)
                    .map(|pixel| pixel[3])
                    .collect::<Vec<_>>(),
                expected_base
                    .chunks_exact(4)
                    .map(|pixel| pixel[3])
                    .collect::<Vec<_>>()
            );

            let expected_mr = atlas
                .palette_indices
                .iter()
                .flat_map(|index| {
                    let material = &scene.palette.materials[*index as usize];
                    [
                        0,
                        (material.roughness.clamp(0.0, 1.0) * 255.0).round() as u8,
                        (material.metalness.clamp(0.0, 1.0) * 255.0).round() as u8,
                        255,
                    ]
                })
                .collect::<Vec<_>>();
            assert_eq!(
                decode_png(&gltf, &format!("{} metallic roughness", model.name)),
                expected_mr
            );
            assert_eq!(
                decode_png(&gltf, &format!("{} occlusion", model.name)),
                atlas.ambient_occlusion.as_ref().unwrap().pixels
            );
        }
    }

    #[test]
    fn emissive_and_transmission_pixels_match_the_shared_bake() {
        let bytes = fixture("study.vox");
        let settings = VoxBakeSettings::default();
        let source = crate::source::parse_vox(&bytes).unwrap();
        let scene = crate::bake_scene(&source, &settings).unwrap();
        let export = export_vox_to_glb(&bytes, &settings).unwrap();
        let gltf = validate_glb(&export.bytes);

        for model in &scene.models {
            let Some(atlas) = &model.baked.surface else {
                continue;
            };
            let used = atlas
                .used_palette_indices
                .iter()
                .map(|index| &scene.palette.materials[*index as usize])
                .collect::<Vec<_>>();
            let emission_max = used
                .iter()
                .map(|material| material.emission)
                .fold(0.0_f32, f32::max);
            if emission_max > 0.0 {
                let expected = atlas
                    .palette_indices
                    .iter()
                    .flat_map(|index| {
                        scene.palette.materials[*index as usize]
                            .emissive_srgba_u8(scene.palette.uses_srgb, emission_max)
                    })
                    .collect::<Vec<_>>();
                assert_eq!(
                    decode_png(&gltf, &format!("{} emissive", model.name)),
                    expected
                );
            }

            if used.iter().any(|material| material.translucency > 0.0) {
                let expected = atlas
                    .palette_indices
                    .iter()
                    .map(|index| {
                        (scene.palette.materials[*index as usize]
                            .translucency
                            .clamp(0.0, 1.0)
                            * 255.0)
                            .round() as u8
                    })
                    .collect::<Vec<_>>();
                assert_eq!(
                    decode_png(&gltf, &format!("{} transmission", model.name)),
                    expected
                );
            }
        }
    }

    #[test]
    fn exported_named_nodes_match_evaluated_scene_transforms_and_filtering() {
        fn collect<'a>(
            nodes: &'a [crate::BakedSceneNode],
            output: &mut HashMap<&'a str, Vec<[f32; 16]>>,
        ) {
            for node in nodes {
                if let Some(name) = node.name.as_deref() {
                    output.entry(name).or_default().push(node.transform);
                }
                collect(&node.children, output);
            }
        }

        let bytes = fixture("study.vox");
        let settings = VoxBakeSettings::default();
        let source = crate::source::parse_vox(&bytes).unwrap();
        let scene = crate::bake_scene(&source, &settings).unwrap();
        let snapshot = crate::evaluate_baked_scene_frame(&scene, 0);
        let mut expected = HashMap::new();
        collect(&snapshot, &mut expected);

        let export = export_vox_to_glb(&bytes, &settings).unwrap();
        let gltf = validate_glb(&export.bytes);
        let mut actual = HashMap::<_, Vec<[f32; 16]>>::new();
        for node in gltf.document.nodes() {
            if let Some(name) = node.name() {
                actual
                    .entry(name)
                    .or_default()
                    .push(node.transform().matrix().concat().try_into().unwrap());
            }
        }

        assert_eq!(actual.len(), expected.len());
        for (name, mut transforms) in expected {
            let actual_transforms = actual.get_mut(name).expect("exported named node");
            assert_eq!(actual_transforms.len(), transforms.len(), "node {name}");
            for transform in transforms.drain(..) {
                let index = actual_transforms
                    .iter()
                    .position(|actual| *actual == transform)
                    .unwrap_or_else(|| panic!("missing transform for node {name}"));
                actual_transforms.swap_remove(index);
            }
        }
    }

    #[test]
    fn export_reports_statistics_and_base_mip_loss() {
        let settings = VoxBakeSettings {
            texture_mode: VoxelTextureMode::Packed(crate::TexturePackSettings {
                mip_policy: crate::TextureMipPolicy::Limited { max_lod: 3 },
                ..Default::default()
            }),
            ..Default::default()
        };
        let export = export_vox_to_glb(&fixture("deer.vox"), &settings).unwrap();
        assert!(export.statistics.models > 0);
        assert!(export.statistics.meshes > 0);
        assert!(export.statistics.triangles > 0);
        assert!(export.statistics.textures >= export.statistics.meshes * 2);
        assert_eq!(export.statistics.output_bytes, export.bytes.len());
        assert_eq!(
            export.warnings,
            [super::ExportWarning::BaseMipOnly {
                requested_max_lod: 3
            }]
        );
    }
}
