use bevy::{
    asset::{Asset, Assets, Handle},
    ecs::{
        system::{In, ResMut, RunSystemOnce},
        world::World,
    },
    image::Image,
    pbr::StandardMaterial,
    prelude::Res,
    reflect::TypePath,
    render::mesh::Mesh,
};

pub use self::{data::VoxelData, voxel::Voxel};
pub(crate) use palette::MaterialProperty;
pub(crate) use voxel::RawVoxel;
pub(super) mod data;
pub(super) mod mesh;
#[cfg(feature = "modify_voxels")]
pub(super) mod modify;
#[cfg(feature = "modify_voxels")]
pub(super) mod queryable;
#[cfg(feature = "generate_voxels")]
pub(super) mod sdf;
#[cfg(feature = "modify_voxels")]
pub use self::queryable::VoxelQueryable;
mod palette;
pub use palette::{VoxelElement, VoxelPalette};
mod voxel;

/// Contains the voxel data for a model, as well as handles to the mesh derived from that data and the material
#[derive(Asset, TypePath, Default, Clone, Debug)]
pub struct VoxelModel {
    /// Unique name of the model
    pub name: String,
    /// The voxel data used to generate the mesh
    pub(crate) data: VoxelData,
    /// Optional handle to the model's mesh if the VoxelData contains solid or transmissive voxels
    pub mesh: Option<Handle<Mesh>>,
    /// Optional handle to the model's material if the VoxelData contains solid or transmissive voxels
    pub material: Option<Handle<StandardMaterial>>,
    /// Optional handle to the 3D cloud image if the VoxelData contains cloud voxels
    pub cloud_image: Option<Handle<Image>>,
    /// True if the model contains translucent voxels.
    pub(crate) has_translucency: bool,
}

#[cfg(feature = "generate_voxels")]
impl VoxelModel {
    /// Generates a [`VoxelModel`] from the supplied [`VoxelData`]
    pub fn new(
        world: &mut World,
        data: VoxelData,
        name: String,
        context: Handle<VoxelContext>,
    ) -> Option<(Handle<VoxelModel>, VoxelModel)> {
        world
            .run_system_once_with((data, name, context), Self::add_model)
            .ok()?
    }

    fn add_model(
        In((data, name, context_handle)): In<(VoxelData, String, Handle<VoxelContext>)>,
        mut meshes: ResMut<Assets<Mesh>>,
        mut materials: ResMut<Assets<StandardMaterial>>,
        mut models: ResMut<Assets<VoxelModel>>,
        contexts: Res<Assets<VoxelContext>>,
    ) -> Option<(Handle<VoxelModel>, VoxelModel)> {
        let context = contexts.get(&context_handle)?;
        let (mesh, average_ior) = data.remesh(
            &context.palette.indices_of_refraction,
            &context.palette.density_for_voxel,
        );
        let material = if let Some(ior) = average_ior {
            let mut transmissive_material =
                materials.get(context.transmissive_material.id())?.clone();
            transmissive_material.ior = ior;
            transmissive_material.thickness = data.size().min_element() as f32;
            materials.add(transmissive_material)
        } else {
            context.opaque_material.clone()
        };
        let model = VoxelModel {
            name: name.clone(),
            data,
            mesh: Some(meshes.add(mesh)),
            material: Some(material),
            cloud_image: None,
            has_translucency: average_ior.is_some(),
        };
        let model_handle = models.add(model.clone());
        Some((model_handle, model))
    }
}

/// A [`VoxelPalette`] that can be shared by multiple models, and handles to the [`StandardMaterial`]s derived from the palette.
#[derive(Asset, TypePath, Clone, Debug)]
pub struct VoxelContext {
    /// The palette used by the models
    pub palette: VoxelPalette,

    pub(crate) opaque_material: Handle<StandardMaterial>,
    pub(crate) transmissive_material: Handle<StandardMaterial>,
}

#[cfg(feature = "generate_voxels")]
impl VoxelContext {
    /// Create a new context with the supplied palette
    pub fn new(world: &mut World, palette: VoxelPalette) -> Handle<VoxelContext> {
        world
            .run_system_once_with(palette, Self::new_context)
            .expect("ooh")
    }

    fn new_context(
        In(palette): In<VoxelPalette>,
        mut images: ResMut<Assets<Image>>,
        mut materials: ResMut<Assets<StandardMaterial>>,
        mut contexts: ResMut<Assets<VoxelContext>>,
    ) -> Handle<VoxelContext> {
        let material = palette.create_material(&mut images);
        let mut opaque_material = material.clone();
        #[cfg(feature = "pbr_transmission_textures")]
        {
            opaque_material.specular_transmission_texture = None;
        }
        opaque_material.specular_transmission = 0.0;
        let context = VoxelContext {
            palette,
            opaque_material: materials.add(opaque_material),
            transmissive_material: materials.add(material),
        };
        contexts.add(context)
    }
}
