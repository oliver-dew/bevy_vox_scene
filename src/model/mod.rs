use crate::VoxelModelInstance;
#[cfg(feature = "generate_voxels")]
use bevy::mesh::Mesh;
use bevy::{
    asset::{Asset, Assets, Handle},
    camera::visibility::Visibility,
    ecs::{
        system::{In, ResMut},
        world::World,
    },
    image::Image,
    light::FogVolume,
    mesh::Mesh3d,
    pbr::{MeshMaterial3d, StandardMaterial},
    prelude::Res,
    reflect::TypePath,
    transform::components::Transform,
    world_serialization::WorldAsset,
};

pub use self::{data::VoxelData, voxel::Voxel};
use flipbook::{FlipbookFrame, FlipbookPlayer};
pub(crate) use voxel::RawVoxel;
pub(super) mod data;
pub(super) mod mesh;
#[cfg(feature = "modify_voxels")]
pub(super) mod modify;
pub(crate) mod queryable;
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
    pub(crate) material: Option<Handle<StandardMaterial>>,
}

/// Create a voxel scene from some supplied voxel data
#[cfg(feature = "generate_voxels")]
pub fn create_voxel_scene(
    In((data, name, context_handle)): In<(VoxelData, String, Handle<VoxelContext>)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut models: ResMut<Assets<VoxelModel>>,
    mut scenes: ResMut<Assets<WorldAsset>>,
    contexts: Res<Assets<VoxelContext>>,
) -> Handle<WorldAsset> {
    let context = contexts.get(&context_handle).expect("Voxel Context exists");
    let (maybe_atlas, average_ior, maybe_cloud) = data.remesh(&context.palette.baked);
    let (maybe_mesh_handle, maybe_material_handle) = maybe_atlas.map_or((None, None), |atlas| {
        let mesh_handle = meshes.add(mesh::to_bevy_mesh(
            &atlas.mesh,
            data.settings.supports_remeshing,
        ));
        let mut material = context
            .palette
            .create_atlas_material(&atlas, |_, image| images.add(image));
        configure_material(&mut material, average_ior, &data);
        (Some(mesh_handle), Some(materials.add(material)))
    });
    let cloud_image = maybe_cloud.map(|image| images.add(image));

    let model = VoxelModel {
        name: name.clone(),
        data: data.clone(),
        has_mesh: maybe_mesh_handle.is_some(),
        has_cloud: cloud_image.is_some(),
        material: maybe_material_handle.clone(),
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
        if let Some(material_handle) = maybe_material_handle {
            root.insert(MeshMaterial3d(material_handle));
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
    let scene = WorldAsset::new(world);
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
    mut scenes: ResMut<Assets<WorldAsset>>,
    contexts: Res<Assets<VoxelContext>>,
) -> Handle<WorldAsset> {
    let context = contexts.get(&context_handle).expect("Voxel Context exists");
    let mut world = World::new();
    let mut root = world.spawn((Transform::IDENTITY, Visibility::Visible));
    root.with_children(|spawner| {
        for (index, data) in frames.iter().enumerate() {
            let (maybe_atlas, average_ior, maybe_cloud) = data.remesh(&context.palette.baked);
            let (maybe_mesh_handle, maybe_material_handle) =
                maybe_atlas.map_or((None, None), |atlas| {
                    let mesh_handle = meshes.add(mesh::to_bevy_mesh(
                        &atlas.mesh,
                        data.settings.supports_remeshing,
                    ));
                    let mut material = context
                        .palette
                        .create_atlas_material(&atlas, |_, image| images.add(image));
                    configure_material(&mut material, average_ior, data);
                    (Some(mesh_handle), Some(materials.add(material)))
                });
            let cloud_image = maybe_cloud.map(|image| images.add(image));

            let model = VoxelModel {
                name: format!("{}-{}", name, index),
                data: data.clone(),
                has_mesh: maybe_mesh_handle.is_some(),
                has_cloud: cloud_image.is_some(),
                material: maybe_material_handle.clone(),
            };
            let model_handle = models.add(model.clone());
            let mut frame = spawner.spawn((
                VoxelModelInstance {
                    model: model_handle,
                    context: context_handle.clone(),
                },
                FlipbookFrame(index),
                Transform::IDENTITY,
                if index == 0 {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                },
            ));
            if let Some(mesh_handle) = maybe_mesh_handle {
                frame.insert(Mesh3d(mesh_handle));
                if let Some(material_handle) = maybe_material_handle {
                    frame.insert(MeshMaterial3d(material_handle));
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
    root.insert((FlipbookPlayer {
        frames: (0..frames.len()).collect(),
        ..Default::default()
    },));
    let scene = WorldAsset::new(world);
    scenes.add(scene)
}

/// A [`VoxelPalette`] and other data shared by voxel models in the same scene.
#[derive(Asset, TypePath, Clone, Debug)]
pub struct VoxelContext {
    /// The palette used by the models
    pub palette: VoxelPalette,
}

/// Create a new context with the supplied palette
#[cfg(feature = "generate_voxels")]
pub fn create_voxel_context(
    In(palette): In<VoxelPalette>,
    mut contexts: ResMut<Assets<VoxelContext>>,
) -> Handle<VoxelContext> {
    contexts.add(VoxelContext { palette })
}

pub(crate) fn configure_material(
    material: &mut StandardMaterial,
    average_ior: Option<f32>,
    data: &VoxelData,
) {
    if let Some(ior) = average_ior {
        material.ior = ior;
        material.thickness = data._size().min_element() as f32 * data.settings.voxel_size;
    } else {
        #[cfg(feature = "pbr_transmission_textures")]
        {
            material.specular_transmission_texture = None;
        }
        material.specular_transmission = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use bevy::{
        app::App,
        asset::Assets,
        camera::primitives::MeshAabb,
        ecs::hierarchy::Children,
        math::{UVec3, Vec3, Vec3A},
        mesh::Mesh,
        prelude::Mesh3d,
        world_serialization::WorldAssetRoot,
    };

    use crate::{
        SDF, VoxLoaderSettings, Voxel, VoxelPalette, create_voxel_context, create_voxel_scene,
        test_support::setup_app,
    };

    #[cfg(feature = "generate_voxels")]
    #[test]
    fn test_generate_voxels() {
        let mut app = App::new();
        setup_app(&mut app);
        let palette =
            VoxelPalette::from_colors(vec![bevy::color::palettes::css::GREEN.into()], true);
        let tall_box = SDF::cuboid(Vec3::new(0.5, 2.5, 0.5)).voxelize(
            UVec3::splat(6),
            VoxLoaderSettings::default(),
            Voxel(1),
        );
        let world = app.world_mut();
        let context = world
            .run_system_cached_with(create_voxel_context, palette)
            .expect("Context has been created");
        let scene_handle = world
            .run_system_cached_with(
                create_voxel_scene,
                (tall_box, "tall box".to_string(), context),
            )
            .expect("Add box model");
        let scene_root = world.spawn(WorldAssetRoot(scene_handle)).id();
        app.update();
        let entity = app
            .world()
            .get::<Children>(scene_root)
            .expect("children")
            .first()
            .expect("model entity");

        let mesh_handle = &app.world().get::<Mesh3d>(*entity).expect("voxel mesh").0;
        let mesh = app
            .world()
            .resource::<Assets<Mesh>>()
            .get(mesh_handle)
            .expect("mesh generated");
        assert_eq!(
            mesh.get_aabb().expect("aabb").half_extents,
            Vec3A::new(0.5, 2.5, 0.5)
        );
        assert_eq!(
            mesh.count_vertices(),
            6 * 4,
            "resulting mesh should have 6 quads"
        );
    }
}
