use crate::{DidSpawnVoxelChild, VoxelModelCollection};
use bevy::{
    asset::{Assets, Handle},
    core::Name,
    ecs::{
        entity::Entity,
        system::{Commands, Query, Res},
    },
    hierarchy::BuildChildren,
    log::warn,
    pbr::PbrBundle,
    render::{prelude::SpatialBundle, view::Visibility},
    transform::components::Transform,
};

use crate::scene::{VoxelLayer, VoxelModelInstance, VoxelNode, VoxelScene};

use super::VoxelInstanceReady;

// pub(crate) fn spawn_vox_scenes(
//     mut commands: Commands,
//     query: Query<(
//         Entity,
//         &Handle<VoxelScene>,
//         Option<&Transform>,
//         Option<&Visibility>,
//     )>,
//     vox_scenes: Res<Assets<VoxelScene>>,
//     collections: Res<Assets<VoxelModelCollection>>,
// ) {
//     for (root, scene_handle, transform, visibility) in query.iter() {
//         let Some(scene) = vox_scenes.get(scene_handle) else {
//             continue;
//         };
//         let Some(collection) = collections.get(scene.model_collection.id()) else {
//             continue;
//         };
//         spawn_voxel_node_recursive(&mut commands, &scene.root, root, root, scene, collection);
//         let mut entity = commands.entity(root);
//         entity.remove::<Handle<VoxelScene>>();
//         if let Some(transform) = transform {
//             entity.insert(*transform);
//         }
//         if let Some(visibility) = visibility {
//             entity.insert(*visibility);
//         }
//         commands.trigger_targets(VoxelInstanceReady, root);
//     }
// }

// fn spawn_voxel_node_recursive(
//     commands: &mut Commands,
//     voxel_node: &VoxelNode,
//     root: Entity,
//     entity: Entity,
//     scene: &VoxelScene,
//     model_collection: &VoxelModelCollection,
// ) {
//     let mut entity_commands = commands.entity(entity);
//     if let Some(name) = &voxel_node.name {
//         entity_commands.insert(Name::new(name.clone()));
//     }
//     let mut layer_name: Option<String> = None;
//     let mut model_name: Option<String> = None;
//     if let Some(layer_info) = scene.layers.get(voxel_node.layer_id as usize) {
//         layer_name = layer_info.name.clone();
//         entity_commands.insert((
//             VoxelLayer {
//                 id: voxel_node.layer_id,
//                 name: layer_info.name.clone(),
//             },
//             if voxel_node.is_hidden || layer_info.is_hidden {
//                 Visibility::Hidden
//             } else {
//                 Visibility::Inherited
//             },
//         ));
//     }
//     if let Some(model_index) = &voxel_node.model_id {
//         if let Some(model) = model_collection.models.get(*model_index) {
//             model_name = Some(model.name.clone());
//             entity_commands.insert((
//                 VoxelModelInstance {
//                     collection: scene.model_collection.clone(),
//                     model_name: model.name.clone(),
//                 },
//                 PbrBundle {
//                     mesh: model.mesh.clone(),
//                     material: model.material.clone(),
//                     ..Default::default()
//                 },
//             ));
//         } else {
//             warn!("Model not found, omitting: {:?}", model_index);
//             entity_commands.insert(SpatialBundle::default());
//         }
//     } else {
//         entity_commands.insert(SpatialBundle::default());
//     }
//     entity_commands
//         .insert(Transform::from_matrix(voxel_node.transform))
//         .with_children(|builder| {
//             for child in &voxel_node.children {
//                 let mut child_entity = builder.spawn_empty();
//                 let id = child_entity.id();
//                 spawn_voxel_node_recursive(
//                     &mut child_entity.commands(),
//                     child,
//                     root,
//                     id,
//                     scene,
//                     model_collection,
//                 );
//             }
//         });
//     if let Some(model_name) = model_name {
//         commands.trigger_targets(
//             DidSpawnVoxelChild {
//                 child: entity,
//                 model_name,
//                 layer_name,
//             },
//             root,
//         );
//     }
// }
