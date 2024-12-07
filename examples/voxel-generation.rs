use bevy::{core_pipeline::bloom::Bloom, prelude::*};
use bevy_vox_scene::{
    VoxLoaderSettings, VoxScenePlugin, Voxel, VoxelContext, VoxelModel, VoxelModelInstance,
    VoxelPalette, SDF,
};
use utilities::{PanOrbitCamera, PanOrbitCameraPlugin};

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            PanOrbitCameraPlugin,
            VoxScenePlugin::default(),
        ))
        .add_systems(Startup, (setup_camera, setup))
        .run();
}

fn setup_camera(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn((
        Camera3d::default(),
        Camera {
            hdr: true,
            ..Default::default()
        },
        Transform::from_xyz(-20.0, 10.0, 60.0).looking_at(Vec3::ZERO, Vec3::Y),
        PanOrbitCamera::default(),
        Bloom {
            intensity: 0.3,
            ..default()
        },
        EnvironmentMapLight {
            diffuse_map: assets.load("pisa_diffuse.ktx2"),
            specular_map: assets.load("pisa_specular.ktx2"),
            intensity: 500.0,
            ..default()
        },
    ));
}

fn setup(world: &mut World) {
    let palette = VoxelPalette::from_colors(vec![
        bevy::color::palettes::css::BLUE.into(),
        bevy::color::palettes::css::ALICE_BLUE.into(),
        bevy::color::palettes::css::BISQUE.into(),
    ]);
    let data = SDF::cuboid(Vec3::splat(13.0))
        .subtract(SDF::sphere(16.0))
        .map_to_voxels(
            UVec3::splat(32),
            VoxLoaderSettings::default(),
            |d, _| match d {
                x if x < -1.0 => Voxel(2),
                x if x < 0.0 => Voxel(1),
                x if x >= 0.0 => Voxel::EMPTY,
                _ => Voxel::EMPTY,
            },
        );
    let context = VoxelContext::new(world, palette);
    let model_name = "my sdf model";
    let (model_handle, model) =
        VoxelModel::new(world, data, model_name.to_string(), context.clone())
            .expect("Model has been generated");
    let mesh_handle = model.mesh.expect("Model should have a mesh");
    let material_handle = model.material.expect("Model should have a material");
    world.spawn((
        Mesh3d(mesh_handle),
        MeshMaterial3d(material_handle),
        // The [`VoxelModelInstance`] component is only needed if you want to be able to modify the model at a later time:
        VoxelModelInstance {
            model: model_handle,
            context,
        },
    ));
}
