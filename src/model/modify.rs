use bevy::{
    asset::{Assets, Handle},
    ecs::system::{In, ResMut},
    math::{IVec3, Vec3},
    prelude::Res,
    render::mesh::Mesh,
};
use ndshape::Shape;

use crate::VoxelModelInstance;

use super::{RawVoxel, Voxel, VoxelContext, VoxelModel, VoxelQueryable};

/// Data object passed into [`modify_voxel_model`] system
pub struct VoxelModifier {
    instance: VoxelModelInstance,
    mesh: Handle<Mesh>,
    region: VoxelRegionMode,
    modify: Box<dyn Fn(IVec3, &Voxel, &dyn VoxelQueryable) -> Voxel + Send + Sync + 'static>,
}

impl VoxelModifier {
    /// Run the `modify` closure against every voxel within the `region` of the `model`.
    ///
    /// ### Arguments
    /// * `model` - the id of the [`VoxelModel`] to be modified (you can obtain this by from the [`bevy::asset::Handle::id()`] method).
    /// * `region` - a [`VoxelRegion`] defining the area of the voxel model that the modifier will operate on.
    /// * `modify` - a closure that will run against every voxel within the `region`.
    ///
    /// ### Arguments passed to the `modify` closure
    /// * `position` - the position of the current voxel, in voxel space
    /// * `voxel` - the index of the current voxel
    /// * `model` - a reference to the model, allowing, for instance, querying neighbouring voxels via the methods in [`crate::VoxelQueryable`]
    ///
    /// ### Notes
    /// The smaller the `region` is, the more performant the operation will be.
    pub fn new<F: Fn(IVec3, &Voxel, &dyn VoxelQueryable) -> Voxel + Send + Sync + 'static>(
        instance: VoxelModelInstance,
        mesh: Handle<Mesh>,
        region: VoxelRegionMode,
        modify: F,
    ) -> Self {
        VoxelModifier {
            instance,
            mesh,
            region,
            modify: Box::new(modify),
        }
    }
}

/// System that programmatically modifies the voxels in a model.
///
/// Takes a [`VoxelModifier`] as its input
///
/// ### Example
/// ```no_run
/// # use bevy::prelude::*;
/// # use bevy_vox_scene::{VoxelModelInstance, VoxelRegionMode, VoxelRegion, Voxel, VoxelModifier, modify_voxel_model};
/// # let mut commands: Commands = panic!();
/// # let model_instance: VoxelModelInstance = panic!();
/// # let mesh_handle: Handle<Mesh> = panic!();
/// // cut a sphere-shaped hole out of the loaded model
/// let sphere_center = IVec3::new(10, 10, 10);
/// let radius = 10;
/// let radius_squared = radius * radius;
/// let region = VoxelRegion {
///     origin: sphere_center - IVec3::splat(radius),
///     size: IVec3::splat(1 + (radius * 2)),
/// };
/// let modifier = VoxelModifier::new(
///     model_instance.clone(),
/// 	mesh_handle.clone(),
///     VoxelRegionMode::Box(region),
///     move | position, voxel, model | {
///         // a signed-distance function for a sphere:
///         if position.distance_squared(sphere_center) <= radius_squared {
///             // inside of the sphere, return an empty cell
///             Voxel::EMPTY
///         } else {
///             // outside the sphere, return the underlying voxel value from the model
///             voxel.clone()
///         }
///     },
/// );
/// commands.run_system_cached_with(modify_voxel_model, Some(modifier));
/// ```
pub fn modify_voxel_model(
    In(maybe_modifier): In<Option<VoxelModifier>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut models: ResMut<Assets<VoxelModel>>,
    contexts: Res<Assets<VoxelContext>>,
) {
    let Some(modifier) = maybe_modifier else {
        return;
    };
    let Some(context) = contexts.get(modifier.instance.context.id()) else {
        return;
    };
    let Some(model) = models.get_mut(modifier.instance.model.id()) else {
        return;
    };
    let refraction_indices = &context.palette.indices_of_refraction;
    let density_for_voxel = &context.palette.density_for_voxel;
    let leading_padding = IVec3::splat(model.data.padding() as i32 / 2);
    let model_size = model.size();
    let region = modifier.region.clamped(model_size);
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
    let (maybe_mesh, _average_ior, _maybe_cloud) =
        model.data.remesh(refraction_indices, &density_for_voxel);

    if let Some(mesh) = maybe_mesh {
        meshes.insert(&modifier.mesh, mesh);
    }
}

/// The region of the model to modify
pub enum VoxelRegionMode {
    /// The entire area of the model
    All,
    /// A box region within the model, expressed in voxel space
    Box(VoxelRegion),
}

impl VoxelRegionMode {
    fn clamped(&self, model_size: IVec3) -> VoxelRegion {
        match self {
            VoxelRegionMode::All => VoxelRegion {
                origin: IVec3::ZERO,
                size: model_size,
            },
            VoxelRegionMode::Box(region) => {
                let origin = region.origin.clamp(IVec3::ZERO, model_size - IVec3::ONE);
                let max_size = model_size - origin;
                let size = region.size.clamp(IVec3::ONE, max_size);
                VoxelRegion { origin, size }
            }
        }
    }
}

/// A box region within a model
pub struct VoxelRegion {
    /// The lower-back-left corner of the region
    pub origin: IVec3,
    /// The size of the region
    pub size: IVec3,
}

impl VoxelRegion {
    /// Create a new region from a center and a half size
    pub fn from_center(center: IVec3, half_size: IVec3) -> Self {
        Self {
            origin: center - half_size,
            size: half_size * 2,
        }
    }
    /// Computes the center of the region
    pub fn center(&self) -> Vec3 {
        let origin = Vec3::new(
            self.origin.x as f32,
            self.origin.y as f32,
            self.origin.z as f32,
        );
        let size = Vec3::new(self.size.x as f32, self.size.y as f32, self.size.z as f32);
        origin + (size * 0.5)
    }
}
