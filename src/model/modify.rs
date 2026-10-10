use bevy::math::{IVec3, Vec3};
use thiserror::Error;

use crate::{VoxelModelAssets, VoxelModelSource};

use super::{RawVoxel, Voxel, VoxelQueryable};

/// An error encountered while modifying a voxel model.
#[derive(Error, Debug, PartialEq, Eq)]
pub enum ModifyVoxelError {
    /// The model was retained for queries but not runtime remeshing.
    #[error("voxel model source is not remeshable")]
    NotRemeshable,
    /// The retained model asset is no longer available.
    #[error("retained voxel model asset is unavailable")]
    ModelUnavailable,
    /// The palette needed to remesh the model is no longer available.
    #[error("voxel palette asset is unavailable")]
    PaletteUnavailable,
}

impl VoxelModelAssets<'_> {
    /// Modify voxels in a retained model and regenerate its mesh and material.
    ///
    /// ### Arguments
    /// * `source` - the remeshable source component attached to the model entity.
    /// * `mesh` - the model entity's mesh handle.
    /// * `region` - a [`VoxelRegion`] defining the area of the voxel model that the modifier will operate on.
    /// * `modify` - a closure that will run against every voxel within the `region`.
    ///
    /// ### Arguments passed to the `modify` closure
    /// * `position` - the position of the current voxel, in voxel space
    /// * `voxel` - the index of the current voxel
    /// * `model` - a reference to the model, allowing, for instance, querying neighbouring voxels via the methods in [`crate::VoxelQueryable`]
    ///
    /// ### Notes
    /// Every closure invocation observes the original model. Changes made earlier in the region
    /// are not visible to later invocations.
    ///
    /// The smaller the `region` is, the more performant the operation will be.
    pub fn modify<F>(
        &mut self,
        source: &VoxelModelSource,
        mesh: &bevy::asset::Handle<bevy::mesh::Mesh>,
        region: VoxelRegionMode,
        modify: F,
    ) -> Result<(), ModifyVoxelError>
    where
        F: Fn(IVec3, &Voxel, &dyn VoxelQueryable) -> Voxel,
    {
        let VoxelModelSource::Remeshable { model, palette } = source else {
            return Err(ModifyVoxelError::NotRemeshable);
        };
        let palette = self
            .palettes
            .get(palette.id())
            .ok_or(ModifyVoxelError::PaletteUnavailable)?;
        let mut model = self
            .models
            .get_mut(model.id())
            .ok_or(ModifyVoxelError::ModelUnavailable)?;
        let leading_padding = IVec3::splat(model.data.padding() as i32 / 2);
        let model_size = model.size();
        let region = region.clamped(model_size);
        let start = leading_padding + region.origin;
        let end = start + region.size;
        let mut updated: Vec<RawVoxel> = model.data.grid.voxels.clone();
        for x in start.x..end.x {
            for y in start.y..end.y {
                for z in start.z..end.z {
                    let index = vox_bake::linearize(
                        [x as u32, y as u32, z as u32],
                        model.data.grid.dimensions,
                    );
                    let source: Voxel = model.data.grid.voxels[index].clone().into();
                    updated[index] = RawVoxel::from(modify(
                        IVec3::new(x, y, z) - leading_padding,
                        &source,
                        &*model,
                    ));
                }
            }
        }
        model.data.grid.voxels = updated;
        let (maybe_atlas, average_ior, _maybe_cloud) = model.data.remesh(&palette.baked);

        if let Some(atlas) = maybe_atlas {
            let mut material =
                palette.create_atlas_material(&atlas, |_, image| self.images.add(image));
            super::configure_material(&mut material, average_ior, &model.data);
            let _ = self
                .meshes
                .insert(mesh, super::mesh::to_bevy_mesh(atlas.mesh, true));

            if let Some(material_handle) = &model.material {
                if let Some(old_material) = self.materials.get(material_handle) {
                    #[allow(unused_mut)]
                    let mut old_images = vec![
                        old_material.base_color_texture.clone(),
                        old_material.emissive_texture.clone(),
                        old_material.metallic_roughness_texture.clone(),
                        old_material.occlusion_texture.clone(),
                    ];
                    #[cfg(feature = "pbr_transmission_textures")]
                    old_images.push(old_material.specular_transmission_texture.clone());
                    for image_handle in old_images.into_iter().flatten() {
                        self.images.remove(image_handle.id());
                    }
                }
                let _ = self.materials.insert(material_handle, material);
            } else {
                model.material = Some(self.materials.add(material));
            }
        }
        Ok(())
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

#[cfg(test)]
mod tests {
    use bevy::{
        app::App,
        asset::Assets,
        ecs::{hierarchy::Children, system::SystemState},
        math::IVec3,
        prelude::Mesh3d,
        world_serialization::WorldAssetRoot,
    };

    use crate::{
        Voxel, VoxelModel, VoxelModelAssets, VoxelModelSource, VoxelQueryable, VoxelRegion,
        VoxelRegionMode, model::queryable::OutOfBoundsError,
        test_support::setup_and_load_voxel_scene,
    };

    #[async_std::test]
    async fn test_modify_voxels() {
        let mut app = App::new();
        let handle =
            setup_and_load_voxel_scene(&mut app, "test.vox#outer-group/inner-group/dice").await;
        app.update();
        let scene_root = app.world_mut().spawn(WorldAssetRoot(handle)).id();
        app.update();
        let entity = app
            .world()
            .get::<Children>(scene_root)
            .expect("children")
            .first()
            .expect("scene root");
        let model_source = app
            .world()
            .get::<VoxelModelSource>(*entity)
            .expect("voxel model source")
            .clone();
        let mesh = app
            .world()
            .get::<Mesh3d>(*entity)
            .expect("voxel mesh")
            .clone();
        let region = VoxelRegion {
            origin: IVec3::splat(2),
            size: IVec3::ONE,
        };
        let world = app.world_mut();
        let mut state = SystemState::<VoxelModelAssets>::new(world);
        state
            .get_mut(world)
            .expect("voxel asset resources exist")
            .modify(
                &model_source,
                &mesh.0,
                VoxelRegionMode::Box(region),
                |_pos, _voxel, _model| Voxel(7),
            )
            .expect("model modified");
        state.apply(world);
        app.update();
        let model = app
            .world()
            .resource::<Assets<VoxelModel>>()
            .get(model_source.model().id())
            .expect("retrieve model from Res<Assets>");

        assert_eq!(
            model.get_voxel_at_point(IVec3::splat(4)),
            Err(OutOfBoundsError),
            "Max coordinate should be 3,3,3"
        );
        assert_eq!(
            model.get_voxel_at_point(IVec3::splat(-1)),
            Err(OutOfBoundsError),
            "Min coordinate should be 0,0,0"
        );
        let voxel = model
            .get_voxel_at_point(IVec3::splat(2))
            .expect("Retrieve voxel");
        assert_eq!(voxel.0, 7, "Voxel material should've been changed to 7");
    }
}
