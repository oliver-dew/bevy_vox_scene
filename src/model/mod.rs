use bevy::{
    asset::{Asset, Assets, Handle}, ecs::{
        system::{In, ResMut},
        world::World,
    }, pbr::StandardMaterial, prelude::Res, reflect::TypePath, render::{mesh::Mesh, texture::Image}, utils::HashMap
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
    /// Handle to the model's mesh
    pub mesh: Handle<Mesh>,
    /// Handle to the model's material
    pub material: Handle<StandardMaterial>,
    /// True if the model contains translucent voxels.
    pub(crate) has_translucency: bool,
    /// Handle to the model's palette.
    pub(crate) palette: Handle<VoxelPalette>,
}

/// A collection of [`VoxelModel`]s with a shared [`VoxelPalette`]
#[derive(Clone, Debug)]
pub struct VoxelModelCollection {
    /// The palette used by the models
    pub palette: Handle<VoxelPalette>,
    /// The models in the collection
    pub models: Vec<Handle<VoxelModel>>,
    pub(crate) index_for_model_name: HashMap<String, usize>,
    pub(crate) opaque_material: Handle<StandardMaterial>,
    pub(crate) transmissive_material: Handle<StandardMaterial>,
}

#[cfg(feature = "generate_voxels")]
impl VoxelModelCollection {
    /// Create a new collection with the supplied palette
    pub fn new(world: &mut World, palette: VoxelPalette) -> Option<VoxelModelCollection> {
        let system_id = world.register_system(Self::new_collection);
        world.run_system_with_input(system_id, palette).ok()
    }

    fn new_collection(
        In(palette): In<VoxelPalette>,
        mut images: ResMut<Assets<Image>>,
        mut materials: ResMut<Assets<StandardMaterial>>,
        mut palettes: ResMut<Assets<VoxelPalette>>,
    ) -> VoxelModelCollection {
        let material = palette.create_material(&mut images);
        let mut opaque_material = material.clone();
        opaque_material.specular_transmission_texture = None;
        opaque_material.specular_transmission = 0.0;
        VoxelModelCollection {
            palette: palettes.add(palette),
            models: vec![],
            index_for_model_name: HashMap::new(),
            opaque_material: materials.add(opaque_material),
            transmissive_material: materials.add(material),
        }
    }

    /// Generates a [`VoxelModel`] from the supplied [`VoxelData`] and add it to the [`VoxelModelCollection`]
    pub fn add(
        world: &mut World,
        data: VoxelData,
        name: String,
        collection: VoxelModelCollection,
    ) -> Option<(Handle<VoxelModel>, VoxelModel)> {
        let system_id = world.register_system(Self::add_model);
        world
            .run_system_with_input(system_id, (data, name, collection))
            .ok()?
    }

    fn add_model(
        In((data, name, mut collection)): In<(VoxelData, String, VoxelModelCollection)>,
        mut meshes: ResMut<Assets<Mesh>>,
        mut materials: ResMut<Assets<StandardMaterial>>,
        mut models: ResMut<Assets<VoxelModel>>,
        palettes: Res<Assets<VoxelPalette>>,
    ) -> Option<(Handle<VoxelModel>, VoxelModel)> {
        let palette = palettes.get(&collection.palette)?;
        let (mesh, average_ior) = data.remesh(&palette.indices_of_refraction);
        let material = if let Some(ior) = average_ior {
            let mut transmissive_material = materials
                .get(collection.transmissive_material.id())?
                .clone();
            transmissive_material.ior = ior;
            transmissive_material.thickness = data.size().min_element() as f32;
            materials.add(transmissive_material)
        } else {
            collection.opaque_material.clone()
        };
        let model = VoxelModel {
            name: name.clone(),
            data,
            mesh: meshes.add(mesh),
            material,
            has_translucency: average_ior.is_some(),
            palette: collection.palette.clone(),
        };
        let index = collection.models.len();
        collection.index_for_model_name.insert(name, index);
        let model_handle = models.add(model.clone());
        collection.models.push(model_handle.clone());
        Some((model_handle, model))
    }
}
