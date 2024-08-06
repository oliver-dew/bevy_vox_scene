pub(super) mod systems;

use bevy::{
    asset::{Asset, Handle},
    ecs::{bundle::Bundle, component::Component},
    math::Mat4,
    prelude::{Entity, Event},
    reflect::TypePath,
    render::view::Visibility,
    transform::components::Transform,
};

use crate::VoxelModelCollection;

/// A component bundle for spawning Voxel Scenes.
///
/// The root of the spawned scene will be the entity that has this bundle.
/// In addition to the standard components bevy uses to organise and render pbr meshes,
/// spawned entities will also have [`VoxelLayer`] and [`VoxelModelInstance`] components added.
/// ```
/// # use bevy::prelude::*;
/// # use bevy_vox_scene::VoxelSceneBundle;
///
/// fn setup(
///     mut commands: Commands,
///     assets: Res<AssetServer>,
/// ) {
///     commands.spawn(VoxelSceneBundle {
///         scene: assets.load("study.vox"),
///         ..default()
///     });
///
///     commands.spawn(VoxelSceneBundle {
///         // Load a single model using the name assigned to it in MagicaVoxel.
///         // If a model is nested in a named group, than the group will form part of the path
///         // Path components are separated with a slash
///         scene: assets.load("study.vox#workstation/desk"),
///         ..default()
///     });
/// }
/// ```
#[derive(Bundle, Default)]
pub struct VoxelSceneBundle {
    /// A handle to a [`VoxelScene`], typically loaded from a ".vox" file via the [`bevy::asset::AssetServer`].
    /// This Entity will become the root of the spawned Voxel Scene.
    pub scene: Handle<VoxelScene>,
    /// The transform of the scene root. This will override whatever the root transform is in the Magica Voxel scene.
    pub transform: Transform,
    /// The visibility of the scene root. This will override whatever the root visibility is in the Magical Voxel scene.
    pub visibility: Visibility,
}

/// A representation of the Voxel Scene Graph.
///
/// To spawn a voxel scene, add a [`Handle<VoxelScene>`](VoxelScene) or a [`VoxelSceneBundle`] to an Entity.
/// Voxel Scenes can be loaded from Magica Voxel .vox files.
#[derive(Asset, TypePath, Debug)]
pub struct VoxelScene {
    pub(crate) root: VoxelNode,
    pub(crate) layers: Vec<LayerInfo>,
    pub(crate) model_collection: Handle<VoxelModelCollection>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct VoxelNode {
    pub name: Option<String>,
    pub transform: Mat4,
    pub children: Vec<VoxelNode>,
    pub model_id: Option<usize>,
    pub is_hidden: bool,
    pub layer_id: u32,
}

#[derive(Debug, Clone)]
pub(crate) struct LayerInfo {
    pub name: Option<String>,
    pub is_hidden: bool,
}

/// Component wrapping the handle to the [`VoxelModel`]
///
/// When the scene is spawned this component gets added to entities with a voxel mesh.
#[derive(Component, Clone)]
pub struct VoxelModelInstance {
    /// Handle to the collection that this model is instanced from
    pub collection: Handle<VoxelModelCollection>,
    /// The name of the model within the collection
    pub model_name: String,
}

/// A component specifying which layer the Entity belongs to, with an optional name.
///
/// This can be configured in the Magica Voxel world editor.
#[derive(Component, Clone)]
pub struct VoxelLayer {
    /// The identifier for the layer. Magic Voxel 0.99.6 allows you to assign nodes to one of 8 layers,
    /// so this value will be an index in the range 0 through 7.
    pub id: u32,
    /// An optional name for the Layer, assignable in Magica Voxel layer editor.
    pub name: Option<String>,
}

/// Event triggered against the root of a [`VoxelScene`] whenever a [`VoxelModelInstance`] is spawned in one of the descendents of the root
#[derive(Event)]
pub struct DidSpawnVoxelChild {
    /// the child entity that contains the [`VoxelModelInstance`]
    pub child: Entity,
    /// the name of the model in the [`VoxelModelInstance`]
    pub model_name: String,
    /// the [`VoxelLayer`] name of the child entity
    pub layer_name: Option<String>,
}

/// Event triggered on the root entity once a VoxelScene has finished spawning
#[derive(Event)]
pub struct VoxelInstanceReady;
