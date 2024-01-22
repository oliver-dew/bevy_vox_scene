use crate::VoxelSceneHook;
use bevy::{
    asset::{Assets, Handle},
    core::Name,
    ecs::{
        entity::Entity,
        query::Without,
        system::{Commands, Query, Res},
        world::World,
    },
    hierarchy::{BuildChildren, Children},
    log::warn,
    pbr::PbrBundle,
    render::{prelude::SpatialBundle, view::Visibility},
    transform::components::Transform,
};

use crate::scene::{VoxelLayer, VoxelModel, VoxelModelInstance, VoxelNode, VoxelScene};

pub(crate) fn spawn_vox_scenes(
    mut commands: Commands,
    query: Query<(
        Entity,
        &Handle<VoxelScene>,
        Option<&Transform>,
        Option<&Visibility>,
    )>,
    vox_scenes: Res<Assets<VoxelScene>>,
    vox_models: Res<Assets<VoxelModel>>,
) {
    for (root, scene_handle, transform, visibility) in query.iter() {
        if let Some(scene) = vox_scenes.get(scene_handle) {
            spawn_voxel_node_recursive(&mut commands, &scene.root, root, scene, &vox_models);
            let mut entity = commands.entity(root);
            entity.remove::<Handle<VoxelScene>>();
            if let Some(transform) = transform {
                entity.insert(*transform);
            }
            if let Some(visibility) = visibility {
                entity.insert(*visibility);
            }
        }
    }
}

fn spawn_voxel_node_recursive(
    commands: &mut Commands,
    voxel_node: &VoxelNode,
    entity: Entity,
    scene: &VoxelScene,
    vox_models: &Res<Assets<VoxelModel>>,
) {
    let mut entity_commands = commands.entity(entity);
    if let Some(name) = &voxel_node.name {
        entity_commands.insert(Name::new(name.clone()));
    }
    if let Some(model_handle) = &voxel_node.model_id.and_then(|id| scene.models.get(id)) {
        if let Some(model) = vox_models.get(*model_handle) {
            entity_commands.insert(VoxelModelInstance((**model_handle).clone()));
            #[cfg(not(test))]
            entity_commands.insert(PbrBundle {
                mesh: model.mesh.clone(),
                material: model.material.clone(),
                ..Default::default()
            });
        } else {
            warn!("Model not found, omitting: {:?}", model_handle);
            entity_commands.insert(SpatialBundle::default());
        }
    } else {
        entity_commands.insert(SpatialBundle::default());
    }

    if let Some(layer_info) = scene.layers.get(voxel_node.layer_id as usize) {
        entity_commands.insert((
            VoxelLayer {
                id: voxel_node.layer_id,
                name: layer_info.name.clone(),
            },
            if voxel_node.is_hidden || layer_info.is_hidden {
                Visibility::Hidden
            } else {
                Visibility::Inherited
            },
        ));
    }
    entity_commands
        .insert(Transform::from_matrix(voxel_node.transform))
        .with_children(|builder| {
            for child in &voxel_node.children {
                let mut child_entity = builder.spawn_empty();
                let id = child_entity.id();
                spawn_voxel_node_recursive(child_entity.commands(), child, id, scene, vox_models);
            }
        });
}

pub(crate) fn run_hooks(
    mut commands: Commands,
    world: &World,
    query: Query<(Entity, &VoxelSceneHook), Without<Handle<VoxelScene>>>,
) {
    for (entity, scene_hook) in query.iter() {
        run_hook_recursive(&mut commands, world, entity, scene_hook);
        commands.entity(entity).remove::<VoxelSceneHook>();
    }
}

fn run_hook_recursive(
    commands: &mut Commands,
    world: &World,
    entity: Entity,
    scene_hook: &VoxelSceneHook,
) {
    let entity_ref = world.entity(entity);
    let mut entity_commands = commands.entity(entity);
    (scene_hook.hook)(&entity_ref, &mut entity_commands);
    let Some(children) = entity_ref.get::<Children>() else { return };
    for child in children.as_ref() {
        run_hook_recursive(commands, world, *child, scene_hook);
    }
}
