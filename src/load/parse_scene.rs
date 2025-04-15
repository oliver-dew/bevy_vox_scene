use bevy::{
    asset::{Handle, LoadContext},
    ecs::{hierarchy::ChildSpawner, name::Name},
    image::Image,
    log::warn,
    math::{Mat3, Mat4, Quat, Vec3},
    pbr::{FogVolume, MeshMaterial3d, StandardMaterial},
    platform::collections::HashSet,
    prelude::{EntityWorldMut, Transform, Visibility, World},
    render::mesh::{Mesh, Mesh3d},
    scene::Scene,
};
use dot_vox::{Frame, SceneNode};

use crate::{VoxelLayer, VoxelModel, VoxelModelInstance, VoxelQueryable};

use super::{
    components::{LayerInfo, VoxelAnimationPlayer},
    VoxelAnimationFrame,
};

pub(super) fn find_model_names(
    name_for_model: &mut Vec<Option<String>>,
    graph: &Vec<SceneNode>,
    scene_node: &SceneNode,
    parent_name: Option<&String>,
) {
    match scene_node {
        SceneNode::Transform {
            attributes,
            frames: _,
            child,
            layer_id: _,
        } => {
            let (accumulated, node_name) =
                get_accumulated_and_node_name(parent_name, attributes.get("_name"));
            match &graph[*child as usize] {
                SceneNode::Group {
                    attributes: _,
                    children,
                } => {
                    for grandchild in children {
                        find_model_names(
                            name_for_model,
                            graph,
                            &graph[*grandchild as usize],
                            accumulated.as_ref(),
                        );
                    }
                }
                SceneNode::Shape {
                    attributes: _,
                    models,
                } => {
                    let model_id = models[0].model_id as usize;
                    match (&name_for_model[model_id], node_name) {
                        (None, Some(name)) | (Some(_), Some(name)) => {
                            let mut node_name = name.clone();
                            // disambiguate model name if we have a scene where different models have the same name
                            let name_root = name;
                            let mut disambiguator = 0;
                            let mut names_to_disambiguate = name_for_model.clone();
                            names_to_disambiguate.remove(model_id);
                            while names_to_disambiguate.contains(&Some(node_name.clone())) {
                                node_name =
                                    format_args!("{}_{}", name_root, disambiguator).to_string();
                                disambiguator += 1;
                            }
                            name_for_model[model_id] = Some(node_name)
                        }
                        (None, None) | (Some(_), None) => (),
                    };
                }
                _ => {}
            }
        }
        _ => {}
    }
}

pub(super) fn parse_scene_graph(
    context: &mut LoadContext,
    graph: &Vec<SceneNode>,
    scene_node: &SceneNode,
    parent_name: Option<&String>,
    vox_models: &Vec<VoxelModel>,
    subassets: &mut HashSet<String>,
    layers: &Vec<LayerInfo>,
    scene_scale: f32,
) -> Scene {
    let mut world = World::default();
    match scene_node {
        SceneNode::Transform {
            attributes,
            frames: _, // nb for the root node we ignore the transform
            child,
            layer_id,
        } => {
            let (accumulated, node_name) =
                get_accumulated_and_node_name(parent_name, attributes.get("_name"));
            let mut entity = world.spawn(Transform::IDENTITY);
            let maybe_layer = layers.get(*layer_id as usize);
            let node_is_hidden = parse_bool(attributes.get("_hidden").cloned());
            let layer_is_hidden = maybe_layer.map_or(false, |v| v.is_hidden);
            let visibility = if node_is_hidden || layer_is_hidden {
                Visibility::Hidden
            } else {
                Visibility::Inherited
            };
            entity.insert(visibility);
            load_xform_child(
                context,
                graph,
                &graph[*child as usize],
                &mut entity,
                accumulated.as_ref(),
                vox_models,
                subassets,
                layers,
                scene_scale,
            );

            if let Some(layer) = maybe_layer {
                entity.insert(VoxelLayer {
                    id: *layer_id,
                    name: layer.name.clone(),
                });
            }

            if let Some(node_name) = node_name.clone() {
                entity.insert(Name::new(node_name.clone()));
            }
        }
        _ => {}
    }
    Scene::new(world)
}

fn load_xform_node(
    context: &mut LoadContext,
    builder: &mut ChildSpawner,
    graph: &Vec<SceneNode>,
    scene_node: &SceneNode,
    parent_name: Option<&String>,
    vox_models: &Vec<VoxelModel>,
    subassets: &mut HashSet<String>,
    layers: &Vec<LayerInfo>,
    scene_scale: f32,
) {
    match scene_node {
        SceneNode::Transform {
            attributes,
            frames,
            child,
            layer_id,
        } => {
            let (accumulated, node_name) =
                get_accumulated_and_node_name(parent_name, attributes.get("_name"));
            let mut entity = builder.spawn_empty();

            let maybe_layer = layers.get(*layer_id as usize);
            if let Some(layer) = maybe_layer {
                entity.insert(VoxelLayer {
                    id: *layer_id,
                    name: layer.name.clone(),
                });
            }
            if let Some(node_name) = node_name.clone() {
                entity.insert(Name::new(node_name));
            }

            let node_is_hidden = parse_bool(attributes.get("_hidden").cloned());
            let layer_is_hidden = maybe_layer.map_or(false, |v| v.is_hidden);
            let visibility = if node_is_hidden || layer_is_hidden {
                Visibility::Hidden
            } else {
                Visibility::Inherited
            };
            entity.insert(visibility);

            load_xform_child(
                context,
                graph,
                &graph[*child as usize],
                &mut entity,
                accumulated.as_ref(),
                vox_models,
                subassets,
                layers,
                scene_scale,
            );

            entity.insert(Transform::from_matrix(transform_from_frame(
                &frames[0],
                scene_scale,
            )));

            if let Some(node_name) = node_name {
                // create sub-asset
                if subassets.insert(node_name.clone()) {
                    context.labeled_asset_scope(node_name, |context| {
                        parse_scene_graph(
                            context,
                            graph,
                            scene_node,
                            parent_name,
                            vox_models,
                            subassets,
                            layers,
                            scene_scale,
                        )
                    });
                }
            }
        }
        SceneNode::Group { .. } | SceneNode::Shape { .. } => {
            warn!("Found Group or Shape Node without a parent Transform");
            let mut node = builder.spawn_empty();
            load_xform_child(
                context,
                graph,
                scene_node,
                &mut node,
                parent_name,
                vox_models,
                subassets,
                layers,
                scene_scale,
            );
        }
    }
}

fn load_xform_child(
    context: &mut LoadContext,
    graph: &Vec<SceneNode>,
    scene_node: &SceneNode,
    entity: &mut EntityWorldMut,
    parent_name: Option<&String>,
    vox_models: &Vec<VoxelModel>,
    subassets: &mut HashSet<String>,
    layers: &Vec<LayerInfo>,
    scene_scale: f32,
) {
    match scene_node {
        SceneNode::Transform { .. } => {
            warn!("Found nested Transform nodes");
            entity.insert(Transform::IDENTITY);

            entity.with_children(|builder| {
                load_xform_node(
                    context,
                    builder,
                    graph,
                    scene_node,
                    parent_name,
                    vox_models,
                    subassets,
                    layers,
                    scene_scale,
                );
            });
        }
        SceneNode::Group {
            attributes: _,
            children,
        } => {
            entity.insert(Transform::IDENTITY);
            entity.with_children(|builder| {
                for child in children {
                    load_xform_node(
                        context,
                        builder,
                        graph,
                        &graph[*child as usize],
                        parent_name,
                        vox_models,
                        subassets,
                        layers,
                        scene_scale,
                    );
                }
            });
        }
        SceneNode::Shape {
            attributes: _,
            models,
        } => {
            let model_count = models.len();
            if model_count == 1 {
                let model = &vox_models[models[0].model_id as usize];
                entity.insert(VoxelModelInstance {
                    model:  context.get_label_handle(format!("{}@model", model.name)),
                    context: context.get_label_handle("voxel-context"),
                });
                if model.has_mesh {
                    let mesh: Handle<Mesh> =
                        context.get_label_handle(format!("{}@mesh", model.name));
                    let material: Handle<StandardMaterial> =
                        context.get_label_handle(format!("{}@material", model.name));
                    entity.insert((Mesh3d(mesh), MeshMaterial3d(material)));
                }
                if model.has_cloud {
                    let cloud_image: Handle<Image> =
                        context.get_label_handle(format!("{}@cloud-image", model.name));
                    entity.with_child((
                        FogVolume {
                            density_texture: Some(cloud_image),
                            absorption: 0.1,
                            ..Default::default()
                        },
                        Transform::from_scale(model.model_size()),
                    ));
                }
            } else if model_count > 1 {
                entity.insert(VoxelAnimationPlayer {
                    frames: (0..model_count).collect(),
                    ..Default::default()
                });
                entity.with_children(|spawner| {
                    for index in 0..model_count {
                        let model = &vox_models[models[index].model_id as usize];
                        let mut frame = spawner.spawn((
                            VoxelModelInstance {
                                model:  context.get_label_handle(format!("{}@model", model.name)),
                                context: context.get_label_handle("voxel-context"),
                            },
                            VoxelAnimationFrame(index),
                            if index == 0 {
                                Visibility::Inherited
                            } else {
                                Visibility::Hidden
                            },
                        ));
                        
                        if model.has_mesh {
                            let mesh: Handle<Mesh> =
                                context.get_label_handle(format!("{}@mesh", model.name));
                            let material: Handle<StandardMaterial> =
                                context.get_label_handle(format!("{}@material", model.name));
                            frame.insert((Mesh3d(mesh), MeshMaterial3d(material)));
                        }

                        if model.has_cloud {
                            let cloud_image: Handle<Image> =
                                context.get_label_handle(format!("{}@cloud-image", model.name));
                            frame.with_child((
                                FogVolume {
                                    density_texture: Some(cloud_image),
                                    absorption: 0.1,
                                    ..Default::default()
                                },
                                Transform::from_scale(model.model_size()),
                            ));
                        }
                    }
                });
            }
        }
    }
}

fn get_accumulated_and_node_name(
    parent_name: Option<&String>,
    node_name: Option<&String>,
) -> (Option<String>, Option<String>) {
    match (parent_name, node_name) {
        (None, None) => (None, None),
        (None, Some(node_name)) => (Some(node_name.to_string()), Some(node_name.to_string())),
        (Some(parent_name), None) => (Some(parent_name.to_string()), None), // allow group name to pass down through unnamed child
        (Some(parent_name), Some(node_name)) => {
            let accumulated = format!("{}/{}", parent_name, node_name);
            (Some(accumulated.clone()), Some(accumulated))
        }
    }
}

fn parse_bool(value: Option<String>) -> bool {
    match value.as_deref() {
        Some("1") => true,
        Some("0") => false,
        Some(_) => {
            warn!("Invalid boolean string");
            false
        }
        None => false,
    }
}

fn transform_from_frame(frame: &Frame, scene_scale: f32) -> Mat4 {
    let Some(position) = frame.position() else {
        return Mat4::IDENTITY;
    };
    let position =
        Vec3::new(-position.x as f32, position.z as f32, position.y as f32) * scene_scale;
    let translation = Mat4::from_translation(position);
    let rotation = if let Some(orientation) = frame.orientation() {
        let (rotation, scale) = &orientation.to_quat_scale();
        let scale: Vec3 = (*scale).into();
        let quat = Quat::from_array(*rotation);
        let (axis, angle) = quat.to_axis_angle();
        let mat3 = Mat3::from_axis_angle(Vec3::new(-axis.x, axis.z, axis.y), angle)
            * Mat3::from_diagonal(scale);
        Mat4::from_mat3(mat3)
    } else {
        Mat4::IDENTITY
    };
    translation * rotation
}
