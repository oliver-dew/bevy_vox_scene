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

/// Retained source data for a voxel model instance.
///
/// This component is absent when the source voxels were discarded after baking.
#[derive(Component, Clone, Reflect)]
#[reflect(Component)]
pub enum VoxelModelSource {
    /// The voxel grid can be queried, but the rendered mesh cannot be regenerated.
    Queryable {
        /// Handle to the retained voxel model.
        model: Handle<VoxelModel>,
        /// Handle to the palette from which the model was baked.
        palette: Handle<VoxelPalette>,
    },
    /// The voxel grid can be queried and used to regenerate the rendered mesh.
    Remeshable {
        /// Handle to the retained voxel model.
        model: Handle<VoxelModel>,
        /// Handle to the palette required when regenerating the model.
        palette: Handle<VoxelPalette>,
    },
}

impl VoxelModelSource {
    /// Return the retained voxel model handle.
    pub fn model(&self) -> &Handle<VoxelModel> {
        match self {
            Self::Queryable { model, .. } | Self::Remeshable { model, .. } => model,
        }
    }

    /// Return the palette used to bake the voxel model.
    pub fn palette(&self) -> &Handle<VoxelPalette> {
        match self {
            Self::Queryable { palette, .. } | Self::Remeshable { palette, .. } => palette,
        }
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
