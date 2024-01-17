use bevy::{
    asset::Assets,
    ecs::{
        component::Component,
        entity::Entity,
        system::{Commands, Query, ResMut},
    },
    math::{UVec3, Vec3},
    render::mesh::Mesh,
};
use ndshape::Shape;

use crate::{RawVoxel, Voxel, VoxelModel, VoxelModelInstance};

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
/// let sphere_center = Vec3::new(10.0, 10.0, 10.0);
/// let radius_squared = 10.0 * 10.0;
/// commands.spawn((
///     VoxelSceneBundle {
///         scene: assets.load("study.vox#workstation/desk"),
///         ..default ()
///     },
///     ModifyVoxelModel::new(VoxelRegion::All, move | position, voxel | {
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
    pub(crate) modify: Box<dyn Fn(Vec3, &Voxel) -> Voxel + Send + Sync + 'static>,
}

impl ModifyVoxelModel {
    /// Returns a new [`ModifyVoxelModel`] component
    ///
    /// Attaching this component to an entity that also has a [`VoxelModelInstance`] will
    /// run the `modifer` closure against every voxel in the `region``.
    ///
    /// ### Arguments
    /// * `region` - a [`VoxelRegion`] defining the area of the voxel model that the modifier will operate on.
    /// * `modifier` - a closure that will run against every voxel with the `region`.
    ///
    /// ### Notes
    /// The smaller the `region` is, the more peformant the operation will be.
    ///
    pub fn new<F: Fn(Vec3, &Voxel) -> Voxel + Send + Sync + 'static>(
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
    fn clamped(&self, size: UVec3) -> BoxRegion {
        match self {
            VoxelRegion::All => BoxRegion {
                origin: UVec3::ZERO,
                size,
            },
            VoxelRegion::Box(box_area) => box_area.clamped(size),
        }
    }
}

/// A box area of a voxel model expressed in voxel coordinates
pub struct BoxRegion {
    /// The start of the region
    pub origin: UVec3,
    /// The size of the region
    pub size: UVec3,
}

impl BoxRegion {
    fn clamped(&self, model_size: UVec3) -> BoxRegion {
        let origin = self.origin.min(model_size - UVec3::ONE);
        let max_size = model_size - origin;
        let size = self.size.clamp(UVec3::ONE, max_size);
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

        let leading_padding = UVec3::splat(model.data.padding() / 2);
        let size = model.data.size();
        let region = modifier.region.clamped(size);
        let start = leading_padding + region.origin;
        let end = start + region.size;
        let mut updated: Vec<RawVoxel> = model.data.voxels.clone();
        for x in start.x..end.x {
            for y in start.y..end.y {
                for z in start.z..end.z {
                    let index = model.data.shape.linearize([x, y, z]) as usize;
                    let source: Voxel = model.data.voxels[index].clone().into();
                    updated[index] = RawVoxel::from((modifier.modify)(
                        Vec3::new(x as f32, y as f32, z as f32),
                        &source,
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
