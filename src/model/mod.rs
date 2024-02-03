use bevy::{
    asset::{Asset, Assets, Handle},
    ecs::world::World,
    math::{IVec3, UVec3},
    pbr::StandardMaterial,
    reflect::TypePath,
    render::{mesh::Mesh, texture::Image},
    utils::HashMap,
};
use block_mesh::VoxelVisibility;
use ndshape::{RuntimeShape, Shape};

use self::voxel::VisibleVoxel;
pub use self::voxel::Voxel;
pub(crate) use voxel::RawVoxel;
pub(crate) use palette::MaterialProperty;
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

/// Asset containing the voxel data for a model, as well as handles to the mesh derived from that data and the material
#[derive(Asset, TypePath, Default, Clone)]
pub struct VoxelModel {
    /// The voxel data used to generate the mesh
    pub(crate) data: VoxelData,
    /// Handle to the model's mesh
    pub mesh: Handle<Mesh>,
    /// Handle to the model's material
    pub material: Handle<StandardMaterial>,

    pub(crate) palette: Handle<VoxelPalette>,
}

/// A collection of [`VoxelModel`]s with a shared [`VoxelPalette`]
pub struct ModelCollection {
    palette: Handle<VoxelPalette>,
    models: Vec<Handle<VoxelModel>>,
    opaque_material: Handle<StandardMaterial>,
    transmissive_material: Handle<StandardMaterial>,
}

impl ModelCollection {
    /// Create a new collection with the supplied palette
    pub fn new(world: &mut World, palette: VoxelPalette) -> Option<Self> {
        let cell = world.cell();
        let mut images = cell.get_resource_mut::<Assets<Image>>()?;
        let mut materials = cell.get_resource_mut::<Assets<StandardMaterial>>()?;
        let mut palettes = cell.get_resource_mut::<Assets<VoxelPalette>>()?;
        let material = palette.create_material(&mut images);
        let model = ModelCollection {
            palette: palettes.add(palette),
            models: vec![],
            opaque_material: materials.add(material.clone()),
            transmissive_material: materials.add(material),
        };
        Some(model)
    }

    /// Adds a [`VoxelModel`] to the collection generated with the supplied [`VoxelData``]
    pub fn add(&mut self, data: VoxelData, world: &mut World) -> Option<VoxelModel> {
        let cell = world.cell();
        let palettes = cell.get_resource::<Assets<VoxelPalette>>()?;
        let palette = palettes.get(self.palette.id())?;
        let mesh = data.remesh(&palette.ior_for_voxel());
        let mut meshes = cell.get_resource_mut::<Assets<Mesh>>()?;
        let mut models = cell.get_resource_mut::<Assets<VoxelModel>>()?;
        let model = VoxelModel {
            data,
            mesh: meshes.add(mesh),
            material: self.opaque_material.clone(),
            palette: self.palette.clone(),
        };
        let model_handle = models.add(model.clone());
        self.models.push(model_handle);
        Some(model)
    }
}

/// The voxel data used to create a mesh and a material.
#[derive(Clone)]
pub struct VoxelData {
    pub(crate) shape: RuntimeShape<u32, 3>,
    pub(crate) voxels: Vec<RawVoxel>,
    pub(crate) mesh_outer_faces: bool,
}

impl Default for VoxelData {
    fn default() -> Self {
        Self {
            shape: RuntimeShape::<u32, 3>::new([0, 0, 0]),
            voxels: Default::default(),
            mesh_outer_faces: true,
        }
    }
}

impl VoxelData {
    /// Returns a new, empty VoxelData model
    pub fn new(size: UVec3, mesh_outer_faces: bool) -> Self {
        let padding = if mesh_outer_faces {
            UVec3::splat(2)
        } else {
            UVec3::ZERO
        };
        let shape = RuntimeShape::<u32, 3>::new((size + padding).into());
        let size = shape.size() as usize;
        Self {
            shape,
            voxels: vec![RawVoxel::EMPTY; size],
            mesh_outer_faces,
        }
    }
    /// The size of the voxel model.
    pub(crate) fn _size(&self) -> IVec3 {
        let raw_size: UVec3 = self.shape.as_array().into();
        let padded = raw_size - UVec3::splat(self.padding());
        IVec3::try_from(padded).unwrap_or(IVec3::ZERO)
    }

    /// If the outer faces are to be meshed, the mesher requires 1 voxel of padding around the edge of the model
    pub(crate) fn padding(&self) -> u32 {
        if self.mesh_outer_faces {
            2
        } else {
            0
        }
    }

    pub(crate) fn remesh(&self, ior_for_voxel: &HashMap<u8, f32>) -> Mesh {
        let (visible_voxels, _) = self.visible_voxels(ior_for_voxel);
        mesh::mesh_model(&visible_voxels, self)
    }

    /// Returns the [`VoxelVisibility`] of each Voxel, and, if the model contains
    /// translucent voxels, the average Index of Refraction.
    pub(crate) fn visible_voxels(
        &self,
        ior_for_voxel: &HashMap<u8, f32>,
    ) -> (Vec<VisibleVoxel>, Option<f32>) {
        let mut refraction_indices: Vec<f32> = Vec::new();
        let voxels: Vec<VisibleVoxel> = self
            .voxels
            .iter()
            .map(|v| VisibleVoxel {
                index: v.0,
                visibility: if *v == RawVoxel::EMPTY {
                    VoxelVisibility::Empty
                } else if let Some(ior) = ior_for_voxel.get(&v.0) {
                    refraction_indices.push(*ior);
                    VoxelVisibility::Translucent
                } else {
                    VoxelVisibility::Opaque
                },
            })
            .collect();
        let average_ior: Option<f32> = if refraction_indices.is_empty() {
            None
        } else {
            let ior = refraction_indices
                .iter()
                .cloned()
                .reduce(|acc, e| acc + e)
                .unwrap_or(0.0)
                / refraction_indices.len() as f32;
            Some(ior)
        };
        (voxels, average_ior)
    }
}
