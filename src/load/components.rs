use bevy::image::Image;
use bevy::{
    asset::Handle,
    ecs::component::Component,
    prelude::{ReflectComponent, Transform, Visibility},
    reflect::Reflect,
};

use crate::{VoxelModel, VoxelPalette};

/// Marks an entity as an instance of a voxel model.
#[derive(Component, Clone, Reflect, Default)]
#[reflect(Component)]
#[require(Transform, Visibility)]
pub struct VoxelModelInstance;

/// The retained voxel model from which an instance was baked.
///
/// This component is absent when source voxel data was discarded after baking.
#[derive(Component, Clone, Reflect)]
#[reflect(Component)]
pub struct VoxelModelSource(pub Handle<VoxelModel>);

/// The voxel palette from which an instance was baked.
#[derive(Component, Clone, Reflect)]
#[reflect(Component)]
pub struct VoxelPaletteSource(pub Handle<VoxelPalette>);

/// The density texture used by a voxel cloud instance.
#[derive(Component, Clone, Reflect)]
#[reflect(Component)]
pub struct VoxelCloudSource(pub Handle<Image>);

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
