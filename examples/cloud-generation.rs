use bevy::{
    core_pipeline::bloom::Bloom,
    pbr::{VolumetricFog, VolumetricLight},
    prelude::*,
};
use bevy_vox_scene::{
    VoxLoaderSettings, VoxScenePlugin, Voxel, VoxelContext, VoxelElement, VoxelModel,
    VoxelModelInstance, VoxelPalette, SDF,
};
use utilities::{PanOrbitCamera, PanOrbitCameraPlugin};

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            PanOrbitCameraPlugin,
            VoxScenePlugin::default(),
        ))
        .add_systems(Startup, (setup_light_camera, spawn_cloud))
        .run();
}

/// Spawn light and camera wih the required `VolumetricLight` and `VolumetricFog` components
fn setup_light_camera(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn((
        Camera3d::default(),
        Camera {
            hdr: true,
            ..Default::default()
        },
        Transform::from_xyz(-10.0, -4.0, 31.0)
            .looking_to(Vec3::new(0.3, 0.1, -0.9).normalize(), Vec3::Y),
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
        VolumetricFog {
            ambient_intensity: 0.0,
            jitter: 0.5,
            ..default()
        },
    ));

    commands.spawn((
        DirectionalLight {
            illuminance: 8000.0,
            shadows_enabled: true,
            ..Default::default()
        },
        Transform::IDENTITY.looking_to(Vec3::new(-2.5, -1., 0.85), Vec3::Y),
        VolumetricLight,
    ));
}

/// Spawn a strange-shaped cloud
fn spawn_cloud(world: &mut World) {
    // create a palette of varying densities
    let densities: Vec<f32> = vec![0.5, 0.4, 0.3];
    let palette = VoxelPalette::new(
        densities
            .iter()
            .map(|density| VoxelElement {
                density: *density,
                ..Default::default()
            })
            .collect(),
    );

    // an SDF sphere with a long thin box cutting through onr axis
    let data = SDF::sphere(13.0)
        .subtract(SDF::cuboid(Vec3::new(4., 2., 13.)))
        .map_to_voxels(
            UVec3::splat(32),
            VoxLoaderSettings::default(),
            |d, _| match d {
                x if x < -4.0 => Voxel(3),
                x if x < -2.0 => Voxel(2),
                x if x < 0.0 => Voxel(1),
                x if x >= 0.0 => Voxel::EMPTY,
                _ => Voxel::EMPTY,
            },
        );

    let context = VoxelContext::new(world, palette);
    let model_name = "my sdf model";
    let (model_handle, _model) =
        VoxelModel::new(world, data, model_name.to_string(), context.clone())
            .expect("Model has been generated");

    // When spawning an instance that only contains fog, we need to supply Transform and Visibility,
    // because the FogVolume needs to spawn in a child entity
    world.spawn((
        Transform::IDENTITY,
        Visibility::Visible,
        VoxelModelInstance {
            model: model_handle,
            context,
        },
    ));
}
