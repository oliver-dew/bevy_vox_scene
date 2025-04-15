use crate::VoxelModelInstance;
use bevy::{
    asset::{Asset, Assets, Handle},
    ecs::{
        system::{In, ResMut},
        world::World,
    },
    image::Image,
    pbr::{FogVolume, MeshMaterial3d, StandardMaterial},
    prelude::Res,
    reflect::TypePath,
    render::{
        mesh::{Mesh, Mesh3d},
        view::Visibility,
    },
    scene::Scene,
    transform::components::Transform,
};

pub use self::{data::VoxelData, voxel::Voxel};
use crate::{load::VoxelAnimationFrame, VoxelAnimationPlayer};
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
pub(super) mod cloud;
mod voxel;

/// Contains the voxel data for a model
#[derive(Asset, TypePath, Default, Clone, Debug)]
pub struct VoxelModel {
    /// Unique name of the model
    pub name: String,
    /// The voxel data used to generate the mesh
    pub(crate) data: VoxelData,
    /// True if the model contains solid or transmissive voxels
    pub has_mesh: bool,
    /// True if the model contains cloud voxels
    pub has_cloud: bool,
}

/// Create a voxel scene from some supplied voxel data
#[cfg(feature = "generate_voxels")]
pub fn create_voxel_scene(
    In((data, name, context_handle)): In<(VoxelData, String, Handle<VoxelContext>)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut models: ResMut<Assets<VoxelModel>>,
    mut scenes: ResMut<Assets<Scene>>,
    contexts: Res<Assets<VoxelContext>>,
) -> Handle<Scene> {
    let context = contexts.get(&context_handle).expect("Voxel Context exists");
    let (maybe_mesh, average_ior, maybe_cloud) = data.remesh(
        &context.palette.indices_of_refraction,
        &context.palette.density_for_voxel,
    );
    let maybe_mesh_handle = maybe_mesh.map(|mesh| meshes.add(mesh));
    let cloud_image = maybe_cloud.map(|image| images.add(image));

    let model = VoxelModel {
        name: name.clone(),
        data: data.clone(),
        has_mesh: maybe_mesh_handle.is_some(),
        has_cloud: cloud_image.is_some(),
    };
    let model_handle = models.add(model.clone());

    let mut world = World::new();
    let mut root = world.spawn((
        Transform::IDENTITY,
        Visibility::Visible,
        VoxelModelInstance {
            model: model_handle,
            context: context_handle,
        },
    ));
    // TODO boolean for "retain model data"
    if let Some(mesh_handle) = maybe_mesh_handle {
        root.insert(Mesh3d(mesh_handle));
        if let Some(ior) = average_ior {
            let mut transmissive_material = materials
                .get(context.transmissive_material.id())
                .expect("Transmissive materialo exists")
                .clone();
            transmissive_material.ior = ior;
            transmissive_material.thickness = data.size().min_element() as f32;
            let mat_handle = materials.add(transmissive_material);
            root.insert(MeshMaterial3d(mat_handle));
        } else {
            root.insert(MeshMaterial3d(context.opaque_material.clone()));
        }
    }
    if cloud_image.is_some() {
        root.with_child((
            FogVolume {
                density_texture: cloud_image,
                absorption: 0.1,
                ..Default::default()
            },
            Transform::from_scale(model.model_size()),
        ));
    }
    let scene = Scene::new(world);
    scenes.add(scene)
}

/// Create a voxel animation from some supplied voxel data
#[cfg(feature = "generate_voxels")]
pub fn create_voxel_animation(
    In((frames, name, context_handle)): In<(Vec<VoxelData>, String, Handle<VoxelContext>)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut models: ResMut<Assets<VoxelModel>>,
    mut scenes: ResMut<Assets<Scene>>,
    contexts: Res<Assets<VoxelContext>>,
) -> Handle<Scene> {
    let context = contexts.get(&context_handle).expect("Voxel Context exists");
    let mut world = World::new();
    let mut root = world.spawn((Transform::IDENTITY, Visibility::Visible));
    root.with_children(|spawner| {
        for (index, data) in frames.iter().enumerate() {
            let (maybe_mesh, average_ior, maybe_cloud) = data.remesh(
                &context.palette.indices_of_refraction,
                &context.palette.density_for_voxel,
            );
            let maybe_mesh_handle = maybe_mesh.map(|mesh| meshes.add(mesh));
            let cloud_image = maybe_cloud.map(|image| images.add(image));

            let model = VoxelModel {
                name: format!("{}-{}", name, index),
                data: data.clone(),
                has_mesh: maybe_mesh_handle.is_some(),
                has_cloud: cloud_image.is_some(),
            };
            let model_handle = models.add(model.clone());
            let mut frame = spawner.spawn((
                VoxelModelInstance {
                    model: model_handle,
                    context: context_handle.clone(),
                },
                VoxelAnimationFrame(index),
                Transform::IDENTITY,
                if index == 0 {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                },
            ));
            if let Some(mesh_handle) = maybe_mesh_handle {
                frame.insert(Mesh3d(mesh_handle));
                if let Some(ior) = average_ior {
                    let mut transmissive_material = materials
                        .get(context.transmissive_material.id())
                        .expect("Transmissive materialo exists")
                        .clone();
                    transmissive_material.ior = ior;
                    transmissive_material.thickness = data.size().min_element() as f32;
                    let mat_handle = materials.add(transmissive_material);
                    frame.insert(MeshMaterial3d(mat_handle));
                } else {
                    frame.insert(MeshMaterial3d(context.opaque_material.clone()));
                }
            }
            if cloud_image.is_some() {
                frame.with_child((
                    FogVolume {
                        density_texture: cloud_image,
                        absorption: 0.1,
                        ..Default::default()
                    },
                    Transform::from_scale(model.model_size()),
                ));
            }
        }
    });
    root.insert((VoxelAnimationPlayer {
        frames: (0..frames.len()).collect(),
        ..Default::default()
    },));
    let scene = Scene::new(world);
    scenes.add(scene)
}

/// A [`VoxelPalette`] that can be shared by multiple models, and handles to the [`StandardMaterial`]s derived from the palette.
#[derive(Asset, TypePath, Clone, Debug)]
pub struct VoxelContext {
    /// The palette used by the models
    pub palette: VoxelPalette,

    pub(crate) opaque_material: Handle<StandardMaterial>,
    pub(crate) transmissive_material: Handle<StandardMaterial>,
}

/// Create a new context with the supplied palette
#[cfg(feature = "generate_voxels")]
pub fn create_voxel_context(
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
