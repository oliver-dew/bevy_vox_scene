use bevy::{
    asset::Assets,
    ecs::{
        component::Component,
        entity::Entity,
        system::{Commands, Query, ResMut},
    },
    math::IVec3,
    render::mesh::Mesh,
};
use ndshape::Shape;

use crate::{RawVoxel, Voxel, VoxelModel, VoxelModelInstance, VoxelQueryable};

/// Programatically modify the voxels in the instanced voxel model
///
/// Attaching this component to an entity that also has a [`VoxelModelInstance`] will
/// run the closure against every voxel in the region.
///
/// ### Example
/// ```no_run
/// # use bevy::prelude::*;
/// # use bevy_vox_scene::{VoxelSceneBundle, ModifyVoxelModel, VoxelRegion, Voxel};
/// # fn setup(mut commands: Commands,
/// # assets: Res<AssetServer>)
/// # {
/// // overlay a voxel sphere over the loaded model
/// let sphere_center = IVec3::new(10, 10, 10);
/// let radius_squared = 10 * 10;
/// commands.spawn((
///     VoxelSceneBundle {
///         scene: assets.load("study.vox#workstation/desk"),
///         ..default ()
///     },
///     ModifyVoxelModel::new(VoxelRegion::All, move | position, voxel, model | {
///         // a signed-distance function for a sphere:
///         if position.distance_squared(sphere_center) < radius_squared {
///             // inside of the sphere, coloured with voxels of index 7 in the palette
///             Voxel(7)
///         } else {
///             // outside the sphere, return the underlying voxel value from the model
///             voxel.clone()
///         }
///     }),
/// ));
/// # }
/// ```
#[derive(Component)]
pub struct ModifyVoxelModel {
    pub(crate) region: VoxelRegion,
    pub(crate) modify: Box<dyn Fn(IVec3, &Voxel, &VoxelModel) -> Voxel + Send + Sync + 'static>,
}

impl ModifyVoxelModel {
    /// Returns a new [`ModifyVoxelModel`] component
    ///
    /// Attaching this component to an entity that also has a [`VoxelModelInstance`] will
    /// run the `modify` closure against every voxel within the `region`.
    ///
    /// ### Arguments
    /// * `region` - a [`VoxelRegion`] defining the area of the voxel model that the modifier will operate on.
    /// * `modify` - a closure that will run against every voxel within the `region`.
    ///
    /// ### Arguments passed to the `modify` closure
    /// * the position of the current voxel, in voxel space
    /// * the index of the current voxel
    /// * a reference to the model, allowing, for instance, querying neighbouring voxels via the methods in [`crate::VoxelQueryable`]
    ///
    /// ### Notes
    /// The smaller the `region` is, the more performant the operation will be.
    pub fn new<F: Fn(IVec3, &Voxel, &VoxelModel) -> Voxel + Send + Sync + 'static>(
        region: VoxelRegion,
        modify: F,
    ) -> Self {
        Self {
            region: region,
            modify: Box::new(modify),
        }
    }
}

/// The region of the model to modify
pub enum VoxelRegion {
    /// The entire area of the model
    All,
    /// A [`BoxRegion`] within the model, expressed in voxel space
    Box(BoxRegion),
}

impl VoxelRegion {
    fn clamped(&self, size: IVec3) -> BoxRegion {
        match self {
            VoxelRegion::All => BoxRegion {
                origin: IVec3::ZERO,
                size,
            },
            VoxelRegion::Box(box_area) => box_area.clamped(size),
        }
    }
}

/// A box area of a voxel model expressed in voxel coordinates
pub struct BoxRegion {
    /// The lower-back-left corner of the region
    pub origin: IVec3,
    /// The size of the region
    pub size: IVec3,
}

impl BoxRegion {
    fn clamped(&self, model_size: IVec3) -> BoxRegion {
        let origin = self.origin.clamp(IVec3::ZERO, model_size - IVec3::ONE);
        let max_size = model_size - origin;
        let size = self.size.clamp(IVec3::ONE, max_size);
        BoxRegion { origin, size }
    }
}

pub(crate) fn modify_voxels(
    mut commands: Commands,
    query: Query<(Entity, &ModifyVoxelModel, &VoxelModelInstance)>,
    mut models: ResMut<Assets<VoxelModel>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    for (entity, modifier, instance) in query.iter() {
        let Some(model) = models.get_mut(instance.0.id()) else { continue; };

        let leading_padding = IVec3::splat(model.data.padding() as i32 / 2);
        let size = model.size();
        let region = modifier.region.clamped(size);
        let start = leading_padding + region.origin;
        let end = start + region.size;
        let mut updated: Vec<RawVoxel> = model.data.voxels.clone();
        for x in start.x..end.x {
            for y in start.y..end.y {
                for z in start.z..end.z {
                    let index = model.data.shape.linearize([x as u32, y as u32, z as u32]) as usize;
                    let source: Voxel = model.data.voxels[index].clone().into();
                    updated[index] = RawVoxel::from((modifier.modify)(
                        IVec3::new(x, y, z) - leading_padding,
                        &source,
                        model,
                    ));
                }
            }
        }
        model.data.voxels = updated;
        meshes.insert(&model.mesh, model.data.remesh());
        // TODO: also update material if transparency has changed
        commands.entity(entity).remove::<ModifyVoxelModel>();
    }
}
