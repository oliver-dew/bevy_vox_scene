use bevy::{
    core_pipeline::bloom::Bloom,
    pbr::{FogVolume, VolumetricFog, VolumetricLight},
    prelude::*,
};
use bevy_vox_scene::{
    VoxLoaderSettings, VoxScenePlugin, Voxel, VoxelContext, VoxelElement, VoxelModel,
    VoxelModelInstance, VoxelPalette, SDF,
};
use rand::Rng;
use utilities::{PanOrbitCamera, PanOrbitCameraPlugin};

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            PanOrbitCameraPlugin,
            VoxScenePlugin::default(),
        ))
        .add_systems(Startup, (setup_light_camera, spawn_cloud))
        .add_systems(Update, scroll_fog)
        .run();
}

/// Spawn light and camera with the required `VolumetricLight` and `VolumetricFog` components
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

fn spawn_cloud(world: &mut World) {
    // create a palette of varying densities
    let densities: Vec<f32> = vec![0.3, 1.0, 3.0, 5.0];
    let palette = VoxelPalette::new(
        densities
            .iter()
            .map(|density| VoxelElement {
                density: *density,
                ..Default::default()
            })
            .collect(),
    );

    // Combine a bunch of random SDF::sphere to create the cloud
    let mut rng = rand::thread_rng();
    let mut rng2 = rand::thread_rng();
    let data = (0..30)
        .map(|_| {
            let translation = Vec3::new(
                rng.gen_range(-6.0..=6.0),
                rng.gen_range(-6.0..=6.0),
                rng.gen_range(-6.0..=6.0),
            );
            // spheres are bigger the closer they are to the center
            let inverse_length = (10.4 - translation.length()) * 0.3;
            SDF::sphere(rng.gen_range(0.5..=3.12) + inverse_length).translate(translation)
        })
        .reduce(|acc, new| {
            // 75% of the time we add the new sphere
            if rng2.gen_ratio(3, 4) {
                acc.add(new)
            } else {
                acc.subtract(new)
            }
        })
        .expect("a valid SDF")
        .map_to_voxels(
            UVec3::splat(32),
            VoxLoaderSettings::default(),
            |d, _| match d {
                // higher density the deeper into the cloud we go.
                // nb the `Voxel` values index from 1, with 0 reserved for `Voxel::EMPTY`
                x if x < -3.0 => Voxel(4),
                x if x < -2.0 => Voxel(3),
                x if x < -1.0 => Voxel(2),
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
    // because the FogVolume needs to spawn in a child entity to set the scale of the volume
    world.spawn((
        Transform::IDENTITY,
        Visibility::Visible,
        VoxelModelInstance {
            model: model_handle,
            context,
        },
    ));
}

/// Moves fog density texture offset every frame.
fn scroll_fog(time: Res<Time>, mut query: Query<&mut FogVolume>) {
    for mut fog_volume in query.iter_mut() {
        fog_volume.density_texture_offset += Vec3::new(0.03, -0.005, 0.02) * time.delta_secs();
    }
}
