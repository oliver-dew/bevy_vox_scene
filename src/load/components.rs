use bevy::{asset::Handle, ecs::component::Component, prelude::ReflectComponent, reflect::Reflect};

use crate::{VoxelContext, VoxelModel};

#[derive(Debug, Clone)]
pub(crate) struct LayerInfo {
    pub name: Option<String>,
    pub is_hidden: bool,
}

/// Component wrapping the handle to the [`VoxelModel`]
///
/// When the scene is spawned this component gets added to entities with a voxel mesh.
#[derive(Component, Clone, Reflect)]
#[reflect(Component)]
pub struct VoxelModelInstance {
    /// Handle to the model
    pub model: Handle<VoxelModel>,
    /// Handle to the context
    pub context: Handle<VoxelContext>,
}

/// A component specifying which layer the Entity belongs to, with an optional name.
///
/// This can be configured in the Magica Voxel world editor.
#[derive(Component, Clone, Reflect)]
#[reflect(Component)]
pub struct VoxelLayer {
    /// The identifier for the layer. Magic Voxel 0.99.6 allows you to assign nodes to one of 8 layers,
    /// so this value will be an index in the range 0 through 7.
    pub id: u32,
    /// An optional name for the Layer, assignable in Magica Voxel layer editor.
    pub name: Option<String>,
}
