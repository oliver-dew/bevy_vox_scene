#[cfg(not(all(feature = "webgl2", target_arch = "wasm32")))]
use bevy::anti_alias::taa::TemporalAntiAliasing;
use bevy::{
    camera::ScreenSpaceTransmissionQuality,
    core_pipeline::tonemapping::Tonemapping,
    light::{FogVolume, VolumetricFog, VolumetricLight},
    pbr::ScreenSpaceAmbientOcclusion,
    post_process::bloom::Bloom,
    prelude::*,
};
use bevy_vox_scene::{VoxLoaderSettings, VoxScenePlugin};
use utilities::{PanOrbitCamera, PanOrbitCameraPlugin};

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            PanOrbitCameraPlugin,
            VoxScenePlugin {
                global_settings: Some(VoxLoaderSettings {
                    voxel_size: 0.05,
                    ..default()
                }),
            },
        ))
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn((
        Camera3d {
            screen_space_specular_transmission_quality: ScreenSpaceTransmissionQuality::High,
            screen_space_specular_transmission_steps: 1,
            ..default()
        },
        Transform::from_xyz(8.0, 1.5, 8.0).looking_at(Vec3::ZERO, Vec3::Y),
        Tonemapping::BlenderFilmic,
        PanOrbitCamera::default(),
        Bloom {
            intensity: 0.3,
            ..default()
        },
        #[cfg(not(all(feature = "webgl2", target_arch = "wasm32")))]
        TemporalAntiAliasing::default(),
        #[cfg(not(all(feature = "webgl2", target_arch = "wasm32")))]
        Msaa::Off,
        EnvironmentMapLight {
            diffuse_map: assets.load("pisa_diffuse.ktx2"),
            specular_map: assets.load("pisa_specular.ktx2"),
            intensity: 500.0,
            ..default()
        },
        VolumetricFog {
            ambient_intensity: 0.0,
            ..default()
        },
        ScreenSpaceAmbientOcclusion::default(),
    ));

    commands.spawn((
        DirectionalLight {
            illuminance: 5000.0,
            shadows_enabled: true,
            ..Default::default()
        },
        Transform::IDENTITY.looking_to(Vec3::new(2.5, -1., 0.85), Vec3::Y),
        VolumetricLight,
    ));

    commands.spawn((
        FogVolume::default(),
        Transform::from_scale(Vec3::splat(30.0)),
    ));

    commands.spawn(SceneRoot(assets.load("study.vox")));
}
