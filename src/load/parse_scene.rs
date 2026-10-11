use bevy::{
    animation::{
        AnimatedBy, AnimationClip, AnimationPlayer, AnimationTargetId, animated_field,
        animation_curves::{AnimatableCurve, AnimatableKeyframeCurve},
        graph::{AnimationGraph, AnimationGraphHandle},
    }, asset::{Handle, LoadContext}, ecs::{entity::Entity, error::BevyError, hierarchy::ChildSpawner, name::Name}, image::Image, light::FogVolume, log::warn, math::Mat4, mesh::{Mesh, Mesh3d}, pbr::{MeshMaterial3d, StandardMaterial}, platform::collections::HashSet, prelude::{EntityWorldMut, Transform, Visibility, World}, world_serialization::WorldAsset,
};
use vox_bake::{BakedLayer, BakedSceneGraphNode, BakedTransformFrame};

use crate::{
    VoxelCloudSource, VoxelLayer, VoxelModelInstance, VoxelModelSource, VoxelPaletteSource,
    load::LoadedVoxelModel, model::VoxelMeshAssetUsage,
};

use flipbook::{FlipbookFrame, FlipbookPlayer};

pub(super) fn parse_scene_graph(
    context: &mut LoadContext,
    graph: &[BakedSceneGraphNode],
    root_index: usize,
    vox_models: &[LoadedVoxelModel],
    subassets: &mut HashSet<String>,
    layers: &[BakedLayer],
    animation_frames_per_second: f32,
) -> WorldAsset {
    parse_scene_graph_with_parent(
        context,
        graph,
        root_index,
        None,
        vox_models,
        subassets,
        layers,
        animation_frames_per_second,
    )
}

#[allow(clippy::too_many_arguments)]
fn parse_scene_graph_with_parent(
    context: &mut LoadContext,
    graph: &[BakedSceneGraphNode],
    root_index: usize,
    parent_name: Option<&String>,
    vox_models: &[LoadedVoxelModel],
    subassets: &mut HashSet<String>,
    layers: &[BakedLayer],
    animation_frames_per_second: f32,
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
    let mut clip = AnimationClip::default();
    let mut entity = world.spawn(Transform::IDENTITY);
    let player = entity.id();
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
        &mut clip,
        player,
        animation_frames_per_second,
    );
    if !clip.curves().is_empty() {
        let clip = context.add_labeled_asset("Animation0".to_string(), clip);
        let (graph, _) = AnimationGraph::from_clip(clip);
        let graph = context.add_labeled_asset("AnimationGraph".to_string(), graph);
        world
            .entity_mut(player)
            .insert((AnimationPlayer::default(), AnimationGraphHandle(graph)));
    }
    WorldAsset::new(world)
}

#[allow(clippy::too_many_arguments)]
fn spawn_node(
    context: &mut LoadContext,
    builder: &mut ChildSpawner,
    graph: &[BakedSceneGraphNode],
    node_index: usize,
    parent_name: Option<&String>,
    vox_models: &[LoadedVoxelModel],
    subassets: &mut HashSet<String>,
    layers: &[BakedLayer],
    clip: &mut AnimationClip,
    player: Entity,
    animation_frames_per_second: f32,
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
            clip,
            player,
            animation_frames_per_second,
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
    if frames.len() > 1
        && let Some(node_name) = node_name.as_ref()
    {
        let target = AnimationTargetId::from_name(&Name::new(node_name.clone()));
        entity.insert((target, AnimatedBy(player)));
        add_transform_curves(clip, target, frames, animation_frames_per_second);
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
        clip,
        player,
        animation_frames_per_second,
    );
    entity.insert(
        frames
            .first()
            .map(|frame| Transform::from_matrix(Mat4::from_cols_array(&frame.transform)))
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
                animation_frames_per_second,
            ))
        });
    }
}

fn add_transform_curves(
    clip: &mut AnimationClip,
    target: AnimationTargetId,
    frames: &[BakedTransformFrame],
    frames_per_second: f32,
) {
    let fps = if frames_per_second.is_finite() && frames_per_second > 0.0 {
        frames_per_second
    } else {
        vox_bake::default_animation_frames_per_second()
    };
    let transforms = frames
        .iter()
        .map(|frame| {
            let (scale, rotation, translation) =
                Mat4::from_cols_array(&frame.transform).to_scale_rotation_translation();
            (frame.frame_index as f32 / fps, translation, rotation, scale)
        })
        .collect::<Vec<_>>();
    let translation =
        AnimatableKeyframeCurve::new(transforms.iter().map(|(time, value, _, _)| (*time, *value)));
    let rotation =
        AnimatableKeyframeCurve::new(transforms.iter().map(|(time, _, value, _)| (*time, *value)));
    let scale =
        AnimatableKeyframeCurve::new(transforms.iter().map(|(time, _, _, value)| (*time, *value)));
    if let Ok(curve) = translation {
        clip.add_curve_to_target(
            target,
            AnimatableCurve::new(animated_field!(Transform::translation), curve),
        );
    }
    if let Ok(curve) = rotation {
        clip.add_curve_to_target(
            target,
            AnimatableCurve::new(animated_field!(Transform::rotation), curve),
        );
    }
    if let Ok(curve) = scale {
        clip.add_curve_to_target(
            target,
            AnimatableCurve::new(animated_field!(Transform::scale), curve),
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn load_child(
    context: &mut LoadContext,
    graph: &[BakedSceneGraphNode],
    node_index: usize,
    entity: &mut EntityWorldMut,
    parent_name: Option<&String>,
    vox_models: &[LoadedVoxelModel],
    subassets: &mut HashSet<String>,
    layers: &[BakedLayer],
    clip: &mut AnimationClip,
    player: Entity,
    animation_frames_per_second: f32,
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
                    clip,
                    player,
                    animation_frames_per_second,
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
                        clip,
                        player,
                        animation_frames_per_second,
                    );
                }
            });
        }
        BakedSceneGraphNode::Shape { models } => {
            if models.len() == 1 {
                attach_model(context, entity, vox_models, models[0]);
            } else if !models.is_empty() {
                entity.insert((
                    FlipbookPlayer::from_frame_count(models.len()),
                    Transform::IDENTITY,
                ));
                entity.with_children(|spawner| {
                    for (frame_index, model_index) in models.iter().copied().enumerate() {
                        let mut frame = spawner.spawn((
                            FlipbookFrame(frame_index),
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
    models: &[LoadedVoxelModel],
    model_index: usize,
) {
    let Some(model) = models.get(model_index) else {
        warn!("Shape model index {model_index} is out of bounds");
        return;
    };
    entity.insert((
        VoxelModelInstance,
        VoxelPaletteSource(context.get_label_handle("voxel-palette")),
    ));
    if model.retain_voxel_data {
        entity.insert(VoxelModelSource(
            context.get_label_handle(format!("{}@model", model.name())),
        ));
    }
    if model.has_mesh {
        let mesh: Handle<Mesh> = context.get_label_handle(format!("{}@mesh", model.name()));
        let material: Handle<StandardMaterial> =
            context.get_label_handle(format!("{}@material", model.name()));
        entity.insert((
            Mesh3d(mesh),
            MeshMaterial3d(material),
            VoxelMeshAssetUsage(model.mesh_asset_usage),
        ));
    }
    if model.has_cloud {
        let cloud_image: Handle<Image> =
            context.get_label_handle(format!("{}@cloud-image", model.name()));
        entity.insert(VoxelCloudSource(cloud_image.clone()));
        entity.with_child((
            FogVolume {
                density_texture: Some(cloud_image),
                absorption: 0.1,
                ..Default::default()
            },
            Transform::from_scale(model.model.model_size()),
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
