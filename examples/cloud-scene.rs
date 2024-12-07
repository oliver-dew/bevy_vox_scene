use bevy::{
    core_pipeline::bloom::Bloom,
    pbr::{FogVolume, VolumetricFog, VolumetricLight},
    prelude::*,
};
use bevy_vox_scene::VoxScenePlugin;
use utilities::{PanOrbitCamera, PanOrbitCameraPlugin};

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            VoxScenePlugin::default(),
            PanOrbitCameraPlugin,
        ))
        .add_systems(Startup, setup)
        .add_systems(Update, scroll_fog)
        .run();
}

fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn((
        Camera {
            hdr: true,
            ..default()
        },
        Camera3d::default(),
        Transform::from_xyz(30.0, 30.0, 60.0).looking_at(Vec3::ZERO, Vec3::Y),
        PanOrbitCamera {
            radius: 60.0,
            ..default()
        },
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
            ..default()
        },
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

    commands.spawn(
        // Load a single model using the name assigned to it in MagicaVoxel
        // If a model is nested in a named group, than the group will form part of the path
        // Path components are separated with a slash
        SceneRoot(assets.load("cloud.vox")),
    );
}

/// Moves fog density texture offset every frame.
fn scroll_fog(time: Res<Time>, mut query: Query<&mut FogVolume>) {
    for mut fog_volume in query.iter_mut() {
        fog_volume.density_texture_offset += Vec3::new(0.0, 0.0, 0.04) * time.delta_secs();
    }
}
