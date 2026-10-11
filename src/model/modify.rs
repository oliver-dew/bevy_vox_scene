use bevy::{
    math::{IVec3, Vec3},
    mesh::Mesh3d,
};
use thiserror::Error;

use crate::{VoxelModelAssets, VoxelModelSource, VoxelPaletteSource};

use super::{RawVoxel, Voxel, VoxelQueryable};

/// An error encountered while modifying a voxel model.
#[derive(Error, Debug, PartialEq, Eq)]
pub enum ModifyVoxelError {
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
    /// A surface model was modified without supplying its mesh handle.
    #[error("surface model modification requires a mesh handle")]
    MeshHandleRequired,
    /// The model's material handle is no longer available.
    #[error("voxel material asset is unavailable")]
    MaterialUnavailable,
    /// The model's cloud-density image handle is no longer available.
    #[error("voxel cloud image asset is unavailable")]
    CloudImageUnavailable,
}

impl VoxelModelAssets<'_> {
    /// Modify voxels in a retained model and regenerate its mesh and material.
    ///
    /// ### Arguments
    /// * `source` - the retained model source component attached to the model entity.
    /// * `palette_source` - the palette source component attached to the model entity.
    /// * `mesh` - the same model entity's mesh component, or `None` for a cloud-only model.
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
    /// Models and meshes are assets and may be shared by several scene instances. Modifying a
    /// shared source changes every instance that references those same asset handles.
    ///
    /// The smaller the `region` is, the more performant the operation will be.
    pub fn modify<F>(
        &mut self,
        source: &VoxelModelSource,
        palette_source: &VoxelPaletteSource,
        mesh: Option<&Mesh3d>,
        region: VoxelRegionMode,
        modify: F,
    ) -> Result<(), ModifyVoxelError>
    where
        F: Fn(IVec3, &Voxel, &dyn VoxelQueryable) -> Voxel,
    {
        let palette = self
            .palettes
            .get(&palette_source.0)
            .ok_or(ModifyVoxelError::PaletteUnavailable)?;
        let mut model = self
            .models
            .get_mut(&source.0)
            .ok_or(ModifyVoxelError::ModelUnavailable)?;
        if model.has_mesh && mesh.is_none() {
            return Err(ModifyVoxelError::MeshHandleRequired);
        }
        if let Some(material) = &model.material
            && !self.materials.contains(material.id())
        {
            return Err(ModifyVoxelError::MaterialUnavailable);
        }
        if let Some(cloud_image) = &model.cloud_image
            && !self.images.contains(cloud_image.id())
        {
            return Err(ModifyVoxelError::CloudImageUnavailable);
        }
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
        std::mem::swap(&mut model.data.grid.voxels, &mut updated);
        let remeshed = model.data.remesh(&palette.baked);
        let (maybe_atlas, average_ior, maybe_cloud) = match remeshed {
            Ok(remeshed) => remeshed,
            Err(error) => {
                std::mem::swap(&mut model.data.grid.voxels, &mut updated);
                return Err(error.into());
            }
        };
        if maybe_atlas.is_some() != model.has_mesh || maybe_cloud.is_some() != model.has_cloud {
            std::mem::swap(&mut model.data.grid.voxels, &mut updated);
            return Err(ModifyVoxelError::RenderTopologyChanged);
        }

        if let Some(atlas) = maybe_atlas {
            let mesh = mesh.expect("surface mesh handle checked before modification");
            let mut material =
                palette.create_atlas_material(&atlas, |_, image| self.images.add(image));
            super::configure_material(&mut material, average_ior, &model.data);
            let _ = self.meshes.insert(
                &mesh.0,
                super::mesh::to_bevy_mesh(atlas.mesh, model.data.mesh_asset_usage),
            );

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
        if let (Some(cloud), Some(cloud_handle)) = (maybe_cloud, &model.cloud_image) {
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
        prelude::Mesh3d,
        world_serialization::WorldAssetRoot,
    };

    use crate::{
        Voxel, VoxelModel, VoxelModelAssets, VoxelModelSource, VoxelPaletteSource, VoxelQueryable,
        VoxelRegion, VoxelRegionMode, test_support::setup_and_load_voxel_scene,
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
        let palette_source = app
            .world()
            .get::<VoxelPaletteSource>(*entity)
            .expect("voxel palette source")
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
                &palette_source,
                Some(&mesh),
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

    #[async_std::test]
    async fn test_modify_cloud_updates_without_mesh() {
        let mut app = App::new();
        let handle =
            setup_and_load_voxel_scene(&mut app, "test.vox#outer-group/inner-group/cloud").await;
        app.update();
        let scene_root = app.world_mut().spawn(WorldAssetRoot(handle)).id();
        app.update();
        let entity = *app
            .world()
            .get::<Children>(scene_root)
            .expect("children")
            .first()
            .expect("scene root");
        let source = app
            .world()
            .get::<VoxelModelSource>(entity)
            .expect("voxel model source")
            .clone();
        let palette_source = app
            .world()
            .get::<VoxelPaletteSource>(entity)
            .expect("voxel palette source")
            .clone();

        let world = app.world_mut();
        let mut state = SystemState::<VoxelModelAssets>::new(world);
        state
            .get_mut(world)
            .expect("voxel asset resources exist")
            .modify(
                &source,
                &palette_source,
                None,
                VoxelRegionMode::All,
                |_pos, voxel, _model| voxel.clone(),
            )
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
