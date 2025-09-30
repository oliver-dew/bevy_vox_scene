#[cfg(not(all(feature = "webgl2", target_arch = "wasm32")))]
use bevy::anti_alias::taa::TemporalAntiAliasing;
use bevy::{
    camera::ScreenSpaceTransmissionQuality, core_pipeline::tonemapping::Tonemapping,
    post_process::bloom::Bloom, prelude::*,
};
use bevy_vox_scene::VoxScenePlugin;
use utilities::{PanOrbitCamera, PanOrbitCameraPlugin};

/// Asset labels aren't just for loading individual models within a scene, they can load any named group within a scene, a "slice" of the scene
/// Here, just the workstation is loaded from the example scene
fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            PanOrbitCameraPlugin,
            VoxScenePlugin::default(),
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
        Transform::from_xyz(0.0, 1.5, 8.0).looking_at(Vec3::ZERO, Vec3::Y),
        Tonemapping::SomewhatBoringDisplayTransform,
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
    ));

    commands.spawn((
        // "workstation" is the name of the group containing the desk, computer, & keyboard
        SceneRoot(assets.load("study.vox#workstation")),
        Transform::from_scale(Vec3::splat(0.05)),
    ));
}
