use bevy::{core_pipeline::bloom::Bloom, prelude::*};
use bevy_vox_scene::VoxScenePlugin;
use utilities::{PanOrbitCamera, PanOrbitCameraPlugin};

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
    // An hdr and bloom-enabled camera is needed to create the emissive glowing effect
    commands.spawn((
        Camera3d::default(),
        Camera {
            hdr: true,
            ..default()
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

    commands.spawn(
        // Load a single model using the name assigned to it in MagicaVoxel
        SceneRoot(assets.load("study.vox#workstation/computer")),
    );
}
