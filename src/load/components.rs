use bevy::{
    asset::Handle,
    ecs::component::Component,
    prelude::{ReflectComponent, Transform, Visibility},
    reflect::Reflect,
};

use crate::{VoxelContext, VoxelModel};

/// An instance of a [`VoxelModel`].
#[derive(Component, Clone, Reflect)]
#[reflect(Component)]
#[require(Transform, Visibility)]
pub struct VoxelModelInstance {
    /// Handles to the model.
    pub model: Handle<VoxelModel>,
    /// Handle to the context
    pub context: Handle<VoxelContext>,
}

impl VoxelModelInstance {
    /// Create a new instance
    pub fn new(model: Handle<VoxelModel>, context: Handle<VoxelContext>) -> Self {
        Self { model, context }
    }
}

/// A component specifying which layer the Entity belongs to, with an optional name.
///
/// This can be configured in the Magica Voxel world editor.
#[derive(Component, Clone, Reflect, Debug)]
#[reflect(Component)]
pub struct VoxelLayer {
    /// The identifier for the layer. Magic Voxel 0.99.6 allows you to assign nodes to one of 8 layers,
    /// so this value will be an index in the range 0 through 7.
    pub id: u32,
    /// An optional name for the Layer, assignable in Magica Voxel layer editor.
    pub name: Option<String>,
}
