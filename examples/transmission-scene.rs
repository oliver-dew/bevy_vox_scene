use bevy::{
    core_pipeline::{
        bloom::BloomSettings,
        core_3d::ScreenSpaceTransmissionQuality,
        experimental::taa::{TemporalAntiAliasBundle, TemporalAntiAliasPlugin},
        tonemapping::Tonemapping,
    },
    pbr::{VolumetricFogSettings, VolumetricLight},
    prelude::*,
};
use bevy_vox_scene::{VoxLoaderSettings, VoxScenePlugin};
use utilities::{PanOrbitCamera, PanOrbitCameraPlugin};

fn main() {
    let mut app = App::new();

    app.add_plugins((
        DefaultPlugins,
        PanOrbitCameraPlugin,
        VoxScenePlugin {
            global_settings: Some(VoxLoaderSettings {
                voxel_size: 0.05,
                ..default()
            }),
        },
    ))
    .add_systems(Startup, setup);

    // *Note:* TAA is not _required_ for specular transmission, but
    // it _greatly enhances_ the look of the resulting blur effects.
    // Sadly, it's not available under WebGL.
    #[cfg(not(all(feature = "webgl2", target_arch = "wasm32")))]
    app.insert_resource(Msaa::Off)
        .add_plugins(TemporalAntiAliasPlugin);

    app.run();
}

fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn((
        Camera3dBundle {
            camera: Camera {
                hdr: true,
                ..Default::default()
            },
            camera_3d: Camera3d {
                screen_space_specular_transmission_quality: ScreenSpaceTransmissionQuality::High,
                screen_space_specular_transmission_steps: 1,
                ..default()
            },
            transform: Transform::from_xyz(8.0, 1.5, 8.0).looking_at(Vec3::ZERO, Vec3::Y),
            tonemapping: Tonemapping::BlenderFilmic,
            ..Default::default()
        },
        PanOrbitCamera::default(),
        BloomSettings {
            intensity: 0.3,
            ..default()
        },
        #[cfg(not(all(feature = "webgl2", target_arch = "wasm32")))]
        TemporalAntiAliasBundle::default(),
        EnvironmentMapLight {
            diffuse_map: assets.load("pisa_diffuse.ktx2"),
            specular_map: assets.load("pisa_specular.ktx2"),
            intensity: 500.0,
        },
        VolumetricFogSettings::default(),
    ));

    commands.spawn((
        DirectionalLightBundle {
            directional_light: DirectionalLight {
                illuminance: 5000.0,
                shadows_enabled: true,
                ..Default::default()
            },
            transform: Transform::IDENTITY.looking_to(Vec3::new(2.5, -1., 0.85), Vec3::Y),
            ..default()
        },
        VolumetricLight,
    ));

    commands.spawn(SceneBundle {
        scene: assets.load("study.vox#scene"),
        ..default()
    });
}
