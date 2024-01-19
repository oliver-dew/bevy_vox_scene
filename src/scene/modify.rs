use bevy::{
    asset::{AssetId, Assets},
    ecs::system::{Command, Commands},
    math::IVec3,
    render::mesh::Mesh,
};
use ndshape::Shape;

use crate::{RawVoxel, Voxel, VoxelModel, VoxelQueryable};

/// Command that programatically modifies the voxels in a model.
///
/// This command will run the closure against every voxel within the region of the model.
///
/// ### Example
/// ```no_run
/// # use bevy::prelude::*;
/// # use bevy_vox_scene::{VoxelModel, ModifyVoxelCommandsExt, VoxelRegion, Voxel};
/// # let mut commands: Commands = panic!();
/// # let model_handle: Handle<VoxelModel> = panic!();
/// // cut a sphere-shaped hole out of the loaded model
/// let sphere_center = IVec3::new(10, 10, 10);
/// let radius = 10;
/// let radius_squared = radius * radius;
/// let region = VoxelRegion::Box {
///     origin: sphere_center - IVec3::splat(radius),
///     size: IVec3::splat(1 + (radius * 2)),
/// };
/// commands.modify_voxel_model(
///     model_handle.id(),
///     region,
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
/// ```
pub trait ModifyVoxelCommandsExt {
    /// Run the `modify` closure against every voxel within the `region` of the `model`.
    ///
    /// ### Arguments
    /// * `model` - the id of the [`VoxelModel`] to be modified (you can obtain this by from the [`bevy::asset::Handle::id()`] method).
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
    fn modify_voxel_model<F: Fn(IVec3, &Voxel, &VoxelModel) -> Voxel + Send + Sync + 'static>(
        &mut self,
        model: AssetId<VoxelModel>,
        region: VoxelRegion,
        modify: F,
    ) -> &mut Self;
}

impl ModifyVoxelCommandsExt for Commands<'_, '_> {
    fn modify_voxel_model<F: Fn(IVec3, &Voxel, &VoxelModel) -> Voxel + Send + Sync + 'static>(
        &mut self,
        model: AssetId<VoxelModel>,
        region: VoxelRegion,
        modify: F,
    ) -> &mut Self {
        self.add(ModifyVoxelModel {
            model,
            region,
            modify: Box::new(modify),
        });
        self
    }
}

struct ModifyVoxelModel {
    model: AssetId<VoxelModel>,
    region: VoxelRegion,
    modify: Box<dyn Fn(IVec3, &Voxel, &VoxelModel) -> Voxel + Send + Sync + 'static>,
}

impl Command for ModifyVoxelModel {
    fn apply(self, world: &mut bevy::prelude::World) {
        let cell = world.cell();
        let perform = || -> Option<()> {
            let mut meshes = cell.get_resource_mut::<Assets<Mesh>>()?;
            let mut models = cell.get_resource_mut::<Assets<VoxelModel>>()?;
            let mut model = models.get_mut(self.model)?;
            modify_model(&mut model, &self, &mut meshes);
            Some(())
        };
        perform();
    }
}

/// The region of the model to modify
pub enum VoxelRegion {
    /// The entire area of the model
    All,
    /// A box region within the model, expressed in voxel space
    Box {
        /// The lower-back-left corner of the region
        origin: IVec3,
        /// The size of the region
        size: IVec3,
    },
}

impl VoxelRegion {
    fn clamped(&self, model_size: IVec3) -> (IVec3, IVec3) {
        match self {
            VoxelRegion::All => (IVec3::ZERO, model_size),
            VoxelRegion::Box { origin, size } => {
                let origin = origin.clamp(IVec3::ZERO, model_size - IVec3::ONE);
                let max_size = model_size - origin;
                let size = size.clamp(IVec3::ONE, max_size);
                (origin, size)
            }
        }
    }
}

fn modify_model(model: &mut VoxelModel, modifier: &ModifyVoxelModel, meshes: &mut Assets<Mesh>) {
    let leading_padding = IVec3::splat(model.data.padding() as i32 / 2);
    let model_size = model.size();
    let (origin, size) = modifier.region.clamped(model_size);
    let start = leading_padding + origin;
    let end = start + size;
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
}
