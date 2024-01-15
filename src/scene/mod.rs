mod hook;
pub(crate) mod parse;
pub(super) mod systems;
#[cfg(test)]
mod tests;
use bevy::{
    asset::{Asset, Handle},
    ecs::{bundle::Bundle, component::Component},
    math::Mat4,
    pbr::StandardMaterial,
    reflect::TypePath,
    render::{mesh::Mesh, view::Visibility},
    transform::components::Transform,
};
pub use hook::VoxelSceneHook;

use crate::voxel::VoxelData;

/// A component bundle for spawning Voxel Scenes.
///
/// The root of the spawned scene will be the entity that has this bundle.
/// ```no_run
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

/// A component bundle for spawning Voxel Scenes, with a [`VoxelSceneHook`].
///
/// The root of the spawned scene will be the entity that has this bundle.
/// The [`VoxelSceneHook`] allows you to easily modify Entities deep within the scene hierarchy.
#[derive(Bundle, Default)]
pub struct VoxelSceneHookBundle {
    /// A handle to a [`VoxelScene`], typically loaded from a ".vox" file via the [`bevy::asset::AssetServer`].
    /// This Entity will become the root of the spawned Voxel Scene.
    pub scene: Handle<VoxelScene>,
    /// A [`VoxelSceneHook`] allows you to specify a closure that will be run for each Entity spawned in the scene graph.
    pub hook: VoxelSceneHook,
    /// The transform of the scene root. This will override whatever the root transform is in the Magica Voxel scene.
    pub transform: Transform,
    /// The visibility of the scene root. This will override whatever the root visibility is in the Magical Voxel scene.
    pub visibility: Visibility,
}

/// A representation of the Voxel Scene Graph.
///
/// To spawn a voxel scene, add a [Handle<VoxelScene>](VoxelScene), [`VoxelSceneBundle`], or [`VoxelSceneHookBundle`] to an Entity.
/// Voxel Scenes can be loaded from Magica Voxel .vox files.
#[derive(Asset, TypePath, Debug)]
pub struct VoxelScene {
    pub(crate) root: VoxelNode,
    pub(crate) layers: Vec<LayerInfo>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct VoxelNode {
    name: Option<String>,
    transform: Mat4,
    children: Vec<VoxelNode>,
    model: Option<Handle<VoxelModel>>,
    is_hidden: bool,
    layer_id: u32,
}

#[derive(Asset, TypePath)]
pub(crate) struct VoxelModel {
    pub data: VoxelData,
    pub mesh: Handle<Mesh>,
    pub material: Handle<StandardMaterial>,
}

#[derive(Debug, Clone)]
pub(crate) struct LayerInfo {
    pub name: Option<String>,
    pub is_hidden: bool,
}

#[derive(Component)]
pub struct VoxelModelInstance(Handle<VoxelModel>);

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
