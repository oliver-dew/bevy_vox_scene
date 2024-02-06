use bevy::{
    asset::{Asset, Assets, Handle},
    ecs::world::World,
    pbr::StandardMaterial,
    reflect::TypePath,
    render::{mesh::Mesh, texture::Image},
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
pub(super) mod sdf;
#[cfg(feature = "modify_voxels")]
pub use self::queryable::VoxelQueryable;
mod palette;
pub use palette::{VoxelElement, VoxelPalette};
mod voxel;

/// Contains the voxel data for a model, as well as handles to the mesh derived from that data and the material
#[derive(Default, Clone)]
pub struct VoxelModel {
    /// The voxel data used to generate the mesh
    pub(crate) data: VoxelData,
    /// Handle to the model's mesh
    pub mesh: Handle<Mesh>,
    /// Handle to the model's material
    pub material: Handle<StandardMaterial>,
}

/// A collection of [`VoxelModel`]s with a shared [`VoxelPalette`]
#[derive(Asset, TypePath)]
pub struct ModelCollection {
    /// The palette used by the models
    pub palette: VoxelPalette,
    /// The models in the collection
    pub models: Vec<VoxelModel>,
    pub(crate) opaque_material: Handle<StandardMaterial>,
    pub(crate) transmissive_material: Handle<StandardMaterial>,
}

impl ModelCollection {
    /// Create a new collection with the supplied palette
    pub fn new(world: &mut World, palette: VoxelPalette) -> Option<Self> {
        let cell = world.cell();
        let mut images = cell.get_resource_mut::<Assets<Image>>()?;
        let mut materials = cell.get_resource_mut::<Assets<StandardMaterial>>()?;
        let material = palette.create_material(&mut images);
        let mut opaque_material = material.clone();
        opaque_material.specular_transmission_texture = None;
        opaque_material.specular_transmission = 0.0;
        let model = ModelCollection {
            palette,
            models: vec![],
            opaque_material: materials.add(opaque_material),
            transmissive_material: materials.add(material),
        };
        Some(model)
    }

    /// Adds a [`VoxelModel`] to the collection generated with the supplied [`VoxelData`]
    pub fn add(&mut self, data: VoxelData, world: &mut World) -> Option<VoxelModel> {
        let cell = world.cell();
        let (mesh, average_ior) = data.remesh(&self.palette.indices_of_refraction);
        let mut meshes = cell.get_resource_mut::<Assets<Mesh>>()?;
        let mut materials = cell.get_resource_mut::<Assets<StandardMaterial>>()?;
        let material = if let Some(ior) = average_ior {
            let mut transmissive_material = materials.get(self.transmissive_material.id())?.clone();
            transmissive_material.ior = ior;
            transmissive_material.thickness = data.size().min_element() as f32;
            materials.add(transmissive_material)
        } else {
            self.opaque_material.clone()
        };
        let model = VoxelModel {
            data,
            mesh: meshes.add(mesh),
            material,
        };
        self.models.push(model.clone());
        Some(model)
    }
}
