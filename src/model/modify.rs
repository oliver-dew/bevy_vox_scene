use bevy::{
    ecs::entity::Entity,
    math::{IVec3, Vec3},
};
use thiserror::Error;

use crate::VoxelModelAssets;

use super::{RawVoxel, Voxel, VoxelModel};

/// An error encountered while modifying a voxel model.
#[derive(Error, Debug, PartialEq, Eq)]
pub enum ModifyVoxelError {
    /// The entity is not a retained voxel model instance.
    #[error("entity is not a retained voxel model instance")]
    InvalidInstance,
    /// The retained model asset is no longer available.
    #[error("retained voxel model asset is unavailable")]
    ModelUnavailable,
    /// The palette needed to remesh the model is no longer available.
    #[error("voxel palette asset is unavailable")]
    PaletteUnavailable,
    /// Baking the modified voxel data failed.
    #[error(transparent)]
    Bake(#[from] vox_bake::BakeError),
    /// The modification added or removed an entire surface or cloud representation.
    #[error("modification cannot add or remove a model's surface or cloud representation")]
    RenderTopologyChanged,
    /// A surface instance is missing its material component.
    #[error("surface voxel instance has no material component")]
    MaterialHandleRequired,
    /// The model's material handle is no longer available.
    #[error("voxel material asset is unavailable")]
    MaterialUnavailable,
    /// The model's cloud-density image handle is no longer available.
    #[error("voxel cloud image asset is unavailable")]
    CloudImageUnavailable,
}

impl VoxelModelAssets<'_, '_> {
    /// Modify voxels in a retained model and regenerate its mesh and material.
    ///
    /// ### Arguments
    /// * `entity` - the retained voxel model instance to modify.
    /// * `region` - a [`VoxelRegion`] defining the area of the voxel model that the modifier will operate on.
    /// * `modify` - a closure that will run against every voxel within the `region`.
    ///
    /// ### Arguments passed to the `modify` closure
    /// * `position` - the position of the current voxel, in voxel space
    /// * `voxel` - the index of the current voxel
    /// * `model` - a reference to the model, allowing, for instance, querying neighbouring voxels via [`crate::VoxelModel`] methods
    ///
    /// ### Notes
    /// Every closure invocation observes the original model. Changes made earlier in the region
    /// are not visible to later invocations.
    ///
    /// Models and meshes are assets and may be shared by several scene instances. Modifying a
    /// shared source changes every instance that references those same asset handles.
    ///
    /// The smaller the `region` is, the more performant the operation will be.
    pub fn modify<F>(
        &mut self,
        entity: Entity,
        region: VoxelRegionMode,
        modify: F,
    ) -> Result<(), ModifyVoxelError>
    where
        F: Fn(IVec3, &Voxel, &VoxelModel) -> Voxel,
    {
        let instance = self
            .instances
            .get(entity)
            .map_err(|_| ModifyVoxelError::InvalidInstance)?;
        let model_handle = instance.model.0.clone();
        let palette_handle = instance.palette.0.clone();
        let mesh_handle = instance.mesh.map(|mesh| mesh.0.clone());
        let material_handle = instance.material.map(|material| material.0.clone());
        let cloud_handle = instance.cloud.map(|cloud| cloud.0.clone());
        let mesh_asset_usage = instance
            .mesh_asset_usage
            .map(|usage| usage.0)
            .unwrap_or(bevy::asset::RenderAssetUsages::RENDER_WORLD);

        let palette = self
            .palettes
            .get(&palette_handle)
            .ok_or(ModifyVoxelError::PaletteUnavailable)?;
        let mut model = self
            .models
            .get_mut(&model_handle)
            .ok_or(ModifyVoxelError::ModelUnavailable)?;
        if mesh_handle.is_some() && material_handle.is_none() {
            return Err(ModifyVoxelError::MaterialHandleRequired);
        }
        if let Some(material) = &material_handle
            && !self.materials.contains(material)
        {
            return Err(ModifyVoxelError::MaterialUnavailable);
        }
        if let Some(cloud_image) = &cloud_handle
            && !self.images.contains(cloud_image)
        {
            return Err(ModifyVoxelError::CloudImageUnavailable);
        }
        let leading_padding = IVec3::splat(model.padding() as i32 / 2);
        let model_size = model.size();
        let region = region.clamped(model_size);
        let start = leading_padding + region.origin;
        let end = start + region.size;
        let mut updated: Vec<RawVoxel> = model.grid.voxels.clone();
        for x in start.x..end.x {
            for y in start.y..end.y {
                for z in start.z..end.z {
                    let index =
                        vox_bake::linearize([x as u32, y as u32, z as u32], model.grid.dimensions);
                    let source: Voxel = model.grid.voxels[index].clone().into();
                    updated[index] = RawVoxel::from(modify(
                        IVec3::new(x, y, z) - leading_padding,
                        &source,
                        &*model,
                    ));
                }
            }
        }
        std::mem::swap(&mut model.grid.voxels, &mut updated);
        let remeshed = model.remesh(&palette.baked);
        let (maybe_atlas, average_ior, maybe_cloud) = match remeshed {
            Ok(remeshed) => remeshed,
            Err(error) => {
                std::mem::swap(&mut model.grid.voxels, &mut updated);
                return Err(error.into());
            }
        };
        if maybe_atlas.is_some() != mesh_handle.is_some()
            || maybe_cloud.is_some() != cloud_handle.is_some()
        {
            std::mem::swap(&mut model.grid.voxels, &mut updated);
            return Err(ModifyVoxelError::RenderTopologyChanged);
        }

        if let Some(atlas) = maybe_atlas {
            let mesh = mesh_handle.expect("surface mesh handle checked before modification");
            let mut material =
                palette.create_atlas_material(&atlas, |_, image| self.images.add(image));
            super::configure_material(&mut material, average_ior, &model);
            let _ = self.meshes.insert(
                &mesh,
                super::mesh::to_bevy_mesh(atlas.mesh, mesh_asset_usage),
            );

            if let Some(material_handle) = &material_handle {
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
            }
        }
        if let (Some(cloud), Some(cloud_handle)) = (maybe_cloud, &cloud_handle) {
            let _ = self.images.insert(cloud_handle, cloud);
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
    pub fn from_center_and_half_extent(center: IVec3, half_extent: IVec3) -> Self {
        Self {
            origin: center - half_extent,
            size: half_extent * 2 + IVec3::ONE,
        }
    }
    /// Computes the center of the region
    pub fn center(&self) -> Vec3 {
        let origin = Vec3::new(
            self.origin.x as f32,
            self.origin.y as f32,
            self.origin.z as f32,
        );
        origin + ((self.size - IVec3::ONE).as_vec3() * 0.5)
    }
}

#[cfg(test)]
mod tests {
    use bevy::{
        app::App,
        asset::Assets,
        ecs::{hierarchy::Children, system::SystemState},
        math::IVec3,
        world_serialization::WorldAssetRoot,
    };

    use crate::{
        Voxel, VoxelModel, VoxelModelAssets, VoxelModelSource, VoxelRegion, VoxelRegionMode,
        test_support::setup_and_load_voxel_scene,
    };

    #[test]
    fn test_modify_voxels() {
        let mut app = App::new();
        let handle = setup_and_load_voxel_scene(&mut app, "test.vox#outer-group/inner-group/dice");
        app.update();
        let scene_root = app.world_mut().spawn(WorldAssetRoot(handle)).id();
        app.update();
        let entity = *app
            .world()
            .get::<Children>(scene_root)
            .expect("children")
            .first()
            .expect("scene root");
        let model_source = app
            .world()
            .get::<VoxelModelSource>(entity)
            .expect("voxel model source")
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
                entity,
                VoxelRegionMode::Box(region),
                |_pos, _voxel, _model| Voxel(7),
            )
            .expect("model modified");
        state.apply(world);
        app.update();
        let model = app
            .world()
            .resource::<Assets<VoxelModel>>()
            .get(&model_source.0)
            .expect("retrieve model from Res<Assets>");

        assert!(
            model.get_voxel_at_point(IVec3::splat(4)).is_err(),
            "Max coordinate should be 3,3,3"
        );
        assert!(
            model.get_voxel_at_point(IVec3::splat(-1)).is_err(),
            "Min coordinate should be 0,0,0"
        );
        let voxel = model
            .get_voxel_at_point(IVec3::splat(2))
            .expect("Retrieve voxel");
        assert_eq!(voxel.0, 7, "Voxel material should've been changed to 7");
    }

    #[test]
    fn test_modify_cloud_updates_without_mesh() {
        let mut app = App::new();
        let handle = setup_and_load_voxel_scene(&mut app, "test.vox#outer-group/inner-group/cloud");
        app.update();
        let scene_root = app.world_mut().spawn(WorldAssetRoot(handle)).id();
        app.update();
        let entity = *app
            .world()
            .get::<Children>(scene_root)
            .expect("children")
            .first()
            .expect("scene root");
        let world = app.world_mut();
        let mut state = SystemState::<VoxelModelAssets>::new(world);
        state
            .get_mut(world)
            .expect("voxel asset resources exist")
            .modify(entity, VoxelRegionMode::All, |_pos, voxel, _model| {
                voxel.clone()
            })
            .expect("cloud texture updated");
        state.apply(world);
    }

    #[test]
    fn region_from_center_and_half_extent_is_inclusive() {
        let region =
            VoxelRegion::from_center_and_half_extent(IVec3::new(4, 5, 6), IVec3::new(1, 2, 3));
        assert_eq!(region.origin, IVec3::new(3, 3, 3));
        assert_eq!(region.size, IVec3::new(3, 5, 7));
        assert_eq!(region.center(), IVec3::new(4, 5, 6).as_vec3());
    }
}
