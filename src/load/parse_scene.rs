use bevy::{
    asset::{Handle, LoadContext},
    ecs::{error::BevyError, hierarchy::ChildSpawner, name::Name},
    image::Image,
    light::FogVolume,
    log::warn,
    math::Mat4,
    mesh::{Mesh, Mesh3d},
    pbr::{MeshMaterial3d, StandardMaterial},
    platform::collections::HashSet,
    prelude::{EntityWorldMut, Transform, Visibility, World},
    world_serialization::WorldAsset,
};
use vox_bake::{BakedLayer, BakedSceneGraphNode};

use crate::{VoxelLayer, VoxelModel, VoxelModelInstance, model::queryable::VoxelQueryable};

use super::{VoxelAnimationFrame, components::VoxelAnimationPlayer};

pub(super) fn parse_scene_graph(
    context: &mut LoadContext,
    graph: &[BakedSceneGraphNode],
    root_index: usize,
    vox_models: &[VoxelModel],
    subassets: &mut HashSet<String>,
    layers: &[BakedLayer],
) -> WorldAsset {
    parse_scene_graph_with_parent(
        context, graph, root_index, None, vox_models, subassets, layers,
    )
}

#[allow(clippy::too_many_arguments)]
fn parse_scene_graph_with_parent(
    context: &mut LoadContext,
    graph: &[BakedSceneGraphNode],
    root_index: usize,
    parent_name: Option<&String>,
    vox_models: &[VoxelModel],
    subassets: &mut HashSet<String>,
    layers: &[BakedLayer],
) -> WorldAsset {
    let mut world = World::default();
    let Some(BakedSceneGraphNode::Transform {
        name,
        hidden,
        child,
        layer_index,
        ..
    }) = graph.get(root_index)
    else {
        warn!("Voxel scene root is missing or is not a transform node");
        return WorldAsset::new(world);
    };

    let (accumulated, node_name) = accumulated_name(parent_name, name.as_ref());
    let mut entity = world.spawn(Transform::IDENTITY);
    let layer = layers.get(*layer_index);
    entity.insert(node_visibility(*hidden, layer));
    if let Some(layer) = layer {
        entity.insert(VoxelLayer {
            id: *layer_index as u32,
            name: layer.name.clone(),
        });
    }
    if let Some(node_name) = node_name {
        entity.insert(Name::new(node_name));
    }
    load_child(
        context,
        graph,
        *child,
        &mut entity,
        accumulated.as_ref(),
        vox_models,
        subassets,
        layers,
    );
    WorldAsset::new(world)
}

#[allow(clippy::too_many_arguments)]
fn spawn_node(
    context: &mut LoadContext,
    builder: &mut ChildSpawner,
    graph: &[BakedSceneGraphNode],
    node_index: usize,
    parent_name: Option<&String>,
    vox_models: &[VoxelModel],
    subassets: &mut HashSet<String>,
    layers: &[BakedLayer],
) {
    let Some(node) = graph.get(node_index) else {
        warn!("Scene node index {node_index} is out of bounds");
        return;
    };
    let BakedSceneGraphNode::Transform {
        name,
        hidden,
        layer_index,
        child,
        frames,
    } = node
    else {
        warn!("Found group or shape node without a parent transform");
        let mut entity = builder.spawn_empty();
        load_child(
            context,
            graph,
            node_index,
            &mut entity,
            parent_name,
            vox_models,
            subassets,
            layers,
        );
        return;
    };

    let (accumulated, node_name) = accumulated_name(parent_name, name.as_ref());
    let mut entity = builder.spawn_empty();
    let layer = layers.get(*layer_index);
    entity.insert(node_visibility(*hidden, layer));
    if let Some(layer) = layer {
        entity.insert(VoxelLayer {
            id: *layer_index as u32,
            name: layer.name.clone(),
        });
    }
    if let Some(node_name) = node_name.clone() {
        entity.insert(Name::new(node_name));
    }
    load_child(
        context,
        graph,
        *child,
        &mut entity,
        accumulated.as_ref(),
        vox_models,
        subassets,
        layers,
    );
    entity.insert(
        frames
            .first()
            .map(|matrix| Transform::from_matrix(Mat4::from_cols_array(matrix)))
            .unwrap_or(Transform::IDENTITY),
    );

    if let Some(node_name) = node_name
        && subassets.insert(node_name.clone())
    {
        _ = context.labeled_asset_scope(node_name, |context| {
            Ok::<WorldAsset, BevyError>(parse_scene_graph_with_parent(
                context,
                graph,
                node_index,
                parent_name,
                vox_models,
                subassets,
                layers,
            ))
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn load_child(
    context: &mut LoadContext,
    graph: &[BakedSceneGraphNode],
    node_index: usize,
    entity: &mut EntityWorldMut,
    parent_name: Option<&String>,
    vox_models: &[VoxelModel],
    subassets: &mut HashSet<String>,
    layers: &[BakedLayer],
) {
    let Some(node) = graph.get(node_index) else {
        warn!("Scene child index {node_index} is out of bounds");
        return;
    };
    match node {
        BakedSceneGraphNode::Transform { .. } => {
            warn!("Found nested transform nodes");
            entity.insert(Transform::IDENTITY);
            entity.with_children(|builder| {
                spawn_node(
                    context,
                    builder,
                    graph,
                    node_index,
                    parent_name,
                    vox_models,
                    subassets,
                    layers,
                );
            });
        }
        BakedSceneGraphNode::Group { children } => {
            entity.insert(Transform::IDENTITY);
            entity.with_children(|builder| {
                for child in children {
                    spawn_node(
                        context,
                        builder,
                        graph,
                        *child,
                        parent_name,
                        vox_models,
                        subassets,
                        layers,
                    );
                }
            });
        }
        BakedSceneGraphNode::Shape { models } => {
            if models.len() == 1 {
                attach_model(context, entity, vox_models, models[0]);
            } else if !models.is_empty() {
                entity.insert((
                    VoxelAnimationPlayer {
                        frames: (0..models.len()).collect(),
                        ..Default::default()
                    },
                    Transform::IDENTITY,
                ));
                entity.with_children(|spawner| {
                    for (frame_index, model_index) in models.iter().copied().enumerate() {
                        let mut frame = spawner.spawn((
                            VoxelAnimationFrame(frame_index),
                            if frame_index == 0 {
                                Visibility::Inherited
                            } else {
                                Visibility::Hidden
                            },
                        ));
                        attach_model(context, &mut frame, vox_models, model_index);
                    }
                });
            }
        }
    }
}

fn attach_model(
    context: &mut LoadContext,
    entity: &mut EntityWorldMut,
    models: &[VoxelModel],
    model_index: usize,
) {
    let Some(model) = models.get(model_index) else {
        warn!("Shape model index {model_index} is out of bounds");
        return;
    };
    entity.insert(VoxelModelInstance {
        model: context.get_label_handle(format!("{}@model", model.name)),
        context: context.get_label_handle("voxel-context"),
    });
    if model.has_mesh {
        let mesh: Handle<Mesh> = context.get_label_handle(format!("{}@mesh", model.name));
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
}

fn node_visibility(hidden: bool, layer: Option<&BakedLayer>) -> Visibility {
    if hidden || layer.is_some_and(|layer| layer.hidden) {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    }
}

fn accumulated_name(
    parent_name: Option<&String>,
    node_name: Option<&String>,
) -> (Option<String>, Option<String>) {
    match (parent_name, node_name) {
        (None, None) => (None, None),
        (None, Some(node_name)) => (Some(node_name.clone()), Some(node_name.clone())),
        (Some(parent_name), None) => (Some(parent_name.clone()), None),
        (Some(parent_name), Some(node_name)) => {
            let accumulated = format!("{parent_name}/{node_name}");
            (Some(accumulated.clone()), Some(accumulated))
        }
    }
}
