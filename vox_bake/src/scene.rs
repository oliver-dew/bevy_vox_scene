use std::{error::Error, fmt};

use dot_vox::{DotVoxData, Frame, SceneNode};
use glam::{Mat3, Mat4, Quat, Vec3};

use crate::{BakeError, BakedModel, BakedPalette, VoxBakeSettings, VoxelGrid, bake_model};

/// Layer metadata retained for output adapters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BakedLayer {
    /// Optional layer name.
    pub name: Option<String>,
    /// Whether the layer is hidden in the source file.
    pub hidden: bool,
}

/// One transform keyframe retained from a MagicaVoxel scene node.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BakedTransformFrame {
    /// Source timeline frame. Missing `_f` attributes are assigned in source order.
    pub frame_index: u32,
    /// Column-major local transform in right-handed Y-up coordinates.
    pub transform: [f32; 16],
}

/// One node in an owned scene graph, including unevaluated animation tracks.
#[derive(Clone, Debug, PartialEq)]
pub enum BakedSceneGraphNode {
    /// A transform with one child and zero or more transform frames.
    Transform {
        /// Optional local node name.
        name: Option<String>,
        /// Whether the node is explicitly hidden.
        hidden: bool,
        /// Layer index from the source scene.
        layer_index: usize,
        /// Child node index.
        child: usize,
        /// Local transform animation keyframes.
        frames: Vec<BakedTransformFrame>,
    },
    /// A group containing child node indices.
    Group {
        /// Child node indices.
        children: Vec<usize>,
    },
    /// A shape containing one or more model-frame references.
    Shape {
        /// Model indices in source frame order.
        models: Vec<usize>,
    },
}

/// One named model and its shared bake output.
#[derive(Clone, Debug, PartialEq)]
pub struct BakedSceneModel {
    /// Stable model name used for asset labels.
    pub name: String,
    /// Owned voxel grid retained for runtime querying and remeshing adapters.
    pub grid: VoxelGrid,
    /// Surface, volume, and optical bake output.
    pub baked: BakedModel,
}

/// A complete parsed and baked voxel scene independent of any output adapter.
#[derive(Clone, Debug, PartialEq)]
pub struct BakedScene {
    /// Analyzed material palette shared by every model.
    pub palette: BakedPalette,
    /// Baked model resources referenced by scene nodes.
    pub models: Vec<BakedSceneModel>,
    /// Owned scene graph including animation tracks.
    pub nodes: Vec<BakedSceneGraphNode>,
    /// Root node index.
    pub root: usize,
    /// Source layer metadata.
    pub layers: Vec<BakedLayer>,
}

/// Failure encountered while baking a complete scene.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SceneBakeError {
    /// The source file has no scene graph root.
    MissingScene,
    /// One model could not be baked.
    Model {
        /// Model index that failed.
        index: usize,
        /// Underlying model bake failure.
        source: BakeError,
    },
}

impl fmt::Display for SceneBakeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingScene => formatter.write_str("VOX file contains no scene graph root"),
            Self::Model { index, source } => {
                write!(formatter, "could not bake model {index}: {source}")
            }
        }
    }
}

impl Error for SceneBakeError {}

/// Bake a complete parsed MagicaVoxel scene into an owned adapter-neutral representation.
pub fn bake_scene(
    data: &DotVoxData,
    settings: &VoxBakeSettings,
) -> Result<BakedScene, SceneBakeError> {
    if data.scenes.is_empty() {
        return Err(SceneBakeError::MissingScene);
    }
    let palette = crate::analyze_vox_palette(
        data,
        settings.diffuse_roughness,
        settings.emission_strength,
        settings.uses_srgb,
    );
    let names = model_names(data);
    let models = data
        .models
        .iter()
        .enumerate()
        .map(|(index, model)| {
            let grid = VoxelGrid::from_vox_model(model, settings.mesh_outer_faces);
            let baked = bake_model(&grid, &palette, settings)
                .map_err(|source| SceneBakeError::Model { index, source })?;
            Ok(BakedSceneModel {
                name: names[index]
                    .clone()
                    .unwrap_or_else(|| format!("model-{index}")),
                grid,
                baked,
            })
        })
        .collect::<Result<_, _>>()?;
    let nodes = data
        .scenes
        .iter()
        .map(|node| match node {
            SceneNode::Transform {
                attributes,
                frames,
                child,
                layer_id,
            } => BakedSceneGraphNode::Transform {
                name: attributes.get("_name").cloned(),
                hidden: attributes.get("_hidden").is_some_and(|value| value == "1"),
                layer_index: *layer_id as usize,
                child: *child as usize,
                frames: frames
                    .iter()
                    .enumerate()
                    .map(|(index, frame)| BakedTransformFrame {
                        frame_index: frame.frame_index().unwrap_or(index as u32),
                        transform: frame_transform(frame, settings.voxel_size),
                    })
                    .collect(),
            },
            SceneNode::Group { children, .. } => BakedSceneGraphNode::Group {
                children: children.iter().map(|index| *index as usize).collect(),
            },
            SceneNode::Shape { models, .. } => BakedSceneGraphNode::Shape {
                models: models.iter().map(|model| model.model_id as usize).collect(),
            },
        })
        .collect();
    let layers = data
        .layers
        .iter()
        .map(|layer| BakedLayer {
            name: layer.name(),
            hidden: layer.hidden(),
        })
        .collect();
    Ok(BakedScene {
        palette,
        models,
        nodes,
        root: 0,
        layers,
    })
}

/// One node in a frame-evaluated, engine-independent voxel scene snapshot.
#[derive(Clone, Debug, PartialEq)]
pub struct BakedSceneNode {
    /// Optional source node or model name.
    pub name: Option<String>,
    /// Column-major local transform in right-handed Y-up coordinates.
    pub transform: [f32; 16],
    /// Model resource index referenced by a shape node.
    pub model_index: Option<usize>,
    /// Child nodes after group nodes have been flattened.
    pub children: Vec<BakedSceneNode>,
}

/// Return the number of static snapshots needed for an owned baked scene.
pub fn baked_scene_frame_count(scene: &BakedScene) -> usize {
    scene
        .nodes
        .iter()
        .map(|node| match node {
            BakedSceneGraphNode::Transform { frames, .. } => frames.len(),
            BakedSceneGraphNode::Shape { models } => models.len(),
            BakedSceneGraphNode::Group { .. } => 1,
        })
        .max()
        .unwrap_or(1)
}

/// Resolve hierarchical scene names for every referenced model.
pub fn model_names(data: &DotVoxData) -> Vec<Option<String>> {
    let mut names = vec![None; data.models.len()];
    if let Some(root) = data.scenes.first() {
        find_model_names(&mut names, &data.scenes, root, None);
    }
    names
}

/// Evaluate one static snapshot from an owned baked scene using hold-last-frame semantics.
pub fn evaluate_baked_scene_frame(scene: &BakedScene, frame_index: usize) -> Vec<BakedSceneNode> {
    evaluate_baked_node(scene, scene.root, frame_index, true)
}

fn evaluate_baked_node(
    scene: &BakedScene,
    node_index: usize,
    frame_index: usize,
    is_root: bool,
) -> Vec<BakedSceneNode> {
    let Some(node) = scene.nodes.get(node_index) else {
        return Vec::new();
    };
    match node {
        BakedSceneGraphNode::Group { children } => children
            .iter()
            .flat_map(|child| evaluate_baked_node(scene, *child, frame_index, false))
            .collect(),
        BakedSceneGraphNode::Shape { models } => {
            let Some(model_index) = models.get(frame_index.min(models.len().saturating_sub(1)))
            else {
                return Vec::new();
            };
            vec![BakedSceneNode {
                name: scene
                    .models
                    .get(*model_index)
                    .map(|model| model.name.clone()),
                transform: Mat4::IDENTITY.to_cols_array(),
                model_index: Some(*model_index),
                children: Vec::new(),
            }]
        }
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
            let children = evaluate_baked_node(scene, *child, frame_index, false);
            let transform = if is_root {
                Mat4::IDENTITY.to_cols_array()
            } else {
                frames
                    .get(frame_index.min(frames.len().saturating_sub(1)))
                    .map(|frame| frame.transform)
                    .unwrap_or(Mat4::IDENTITY.to_cols_array())
            };
            vec![BakedSceneNode {
                name: name.clone(),
                transform,
                model_index: None,
                children,
            }]
        }
    }
}

fn find_model_names(
    names: &mut [Option<String>],
    graph: &[SceneNode],
    node: &SceneNode,
    parent_name: Option<&str>,
) {
    let SceneNode::Transform {
        attributes, child, ..
    } = node
    else {
        return;
    };
    let local_name = attributes.get("_name").map(String::as_str);
    let accumulated = match (parent_name, local_name) {
        (None, None) => None,
        (None, Some(name)) => Some(name.to_owned()),
        (Some(parent), None) => Some(parent.to_owned()),
        (Some(parent), Some(name)) => Some(format!("{parent}/{name}")),
    };
    let Some(child) = graph.get(*child as usize) else {
        return;
    };
    match child {
        SceneNode::Group { children, .. } => {
            for child in children
                .iter()
                .filter_map(|index| graph.get(*index as usize))
            {
                find_model_names(names, graph, child, accumulated.as_deref());
            }
        }
        SceneNode::Shape { models, .. } => {
            let (Some(model), Some(name)) = (models.first(), accumulated.as_deref()) else {
                return;
            };
            let model_index = model.model_id as usize;
            if model_index >= names.len() {
                return;
            }
            let base = name.to_owned();
            let mut candidate = base.clone();
            let mut suffix = 0;
            while names.iter().enumerate().any(|(index, existing)| {
                index != model_index && existing.as_ref() == Some(&candidate)
            }) {
                candidate = format!("{base}_{suffix}");
                suffix += 1;
            }
            names[model_index] = Some(candidate);
        }
        SceneNode::Transform { .. } => {}
    }
}

/// Convert a MagicaVoxel frame transform to a right-handed Y-up column-major matrix.
pub fn frame_transform(frame: &Frame, scene_scale: f32) -> [f32; 16] {
    let Some(position) = frame.position() else {
        return Mat4::IDENTITY.to_cols_array();
    };
    let position =
        Vec3::new(-position.x as f32, position.z as f32, position.y as f32) * scene_scale;
    let translation = Mat4::from_translation(position);
    let rotation = if let Some(orientation) = frame.orientation() {
        let (rotation, scale) = &orientation.to_quat_scale();
        let scale = Vec3::from_array(*scale);
        let quat = Quat::from_array(*rotation);
        let (axis, angle) = quat.to_axis_angle();
        let matrix = Mat3::from_axis_angle(Vec3::new(-axis.x, axis.z, axis.y), angle)
            * Mat3::from_diagonal(scale);
        Mat4::from_mat3(matrix)
    } else {
        Mat4::IDENTITY
    };
    (translation * rotation).to_cols_array()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BakedMaterial, VoxBakeSettings};

    fn model(name: &str) -> BakedSceneModel {
        let grid = VoxelGrid::empty([1; 3], true);
        let palette = BakedPalette::new(vec![BakedMaterial::default()], true);
        let baked = bake_model(&grid, &palette, &VoxBakeSettings::default()).unwrap();
        BakedSceneModel {
            name: name.into(),
            grid,
            baked,
        }
    }

    fn frame(frame_index: u32, transform: [f32; 16]) -> BakedTransformFrame {
        BakedTransformFrame {
            frame_index,
            transform,
        }
    }

    #[test]
    fn owned_scene_evaluation_holds_transforms_and_filters_hidden_nodes() {
        let first = Mat4::from_translation(Vec3::new(1.0, 2.0, 3.0)).to_cols_array();
        let second = Mat4::from_translation(Vec3::new(4.0, 5.0, 6.0)).to_cols_array();
        let scene = BakedScene {
            palette: BakedPalette::new(vec![BakedMaterial::default()], true),
            models: vec![model("visible")],
            nodes: vec![
                BakedSceneGraphNode::Transform {
                    name: Some("root".into()),
                    hidden: false,
                    layer_index: 0,
                    child: 1,
                    frames: vec![frame(0, second)],
                },
                BakedSceneGraphNode::Group {
                    children: vec![2, 3],
                },
                BakedSceneGraphNode::Transform {
                    name: Some("visible-node".into()),
                    hidden: false,
                    layer_index: 0,
                    child: 4,
                    frames: vec![frame(0, first), frame(1, second)],
                },
                BakedSceneGraphNode::Transform {
                    name: Some("hidden-node".into()),
                    hidden: true,
                    layer_index: 0,
                    child: 4,
                    frames: vec![frame(0, first)],
                },
                BakedSceneGraphNode::Shape { models: vec![0] },
            ],
            root: 0,
            layers: vec![BakedLayer {
                name: Some("default".into()),
                hidden: false,
            }],
        };

        assert_eq!(baked_scene_frame_count(&scene), 2);
        let snapshot = evaluate_baked_scene_frame(&scene, 20);
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot[0].transform, Mat4::IDENTITY.to_cols_array());
        assert_eq!(snapshot[0].children.len(), 1);
        assert_eq!(
            snapshot[0].children[0].name.as_deref(),
            Some("visible-node")
        );
        assert_eq!(snapshot[0].children[0].transform, second);
        assert_eq!(snapshot[0].children[0].children[0].model_index, Some(0));
    }
}
