use bevy::{
    core_pipeline::{bloom::BloomSettings, tonemapping::Tonemapping},
    prelude::*,
};
use bevy_vox_scene::{UnitOffset, VoxLoaderSettings, VoxScenePlugin};
use utilities::{PanOrbitCamera, PanOrbitCameraPlugin};

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            VoxScenePlugin {
                // Using global settings because Bevy's `load_with_settings` is broken:
                // https://github.com/bevyengine/bevy/issues/12320
                // https://github.com/bevyengine/bevy/issues/11111
                global_settings: Some(VoxLoaderSettings {
                    voxel_size: 0.1,
                    mesh_offset: UnitOffset::CENTER_BASE, // centre the model at its base
                    ..default()
                }),
            },
            PanOrbitCameraPlugin,
        ))
        .add_systems(Startup, setup)
        .run();
}

fn setup(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Camera3dBundle {
            camera: Camera {
                hdr: true,
                ..Default::default()
            },
            transform: Transform::from_xyz(8.0, 1.5, 8.0).looking_at(Vec3::ZERO, Vec3::Y),
            tonemapping: Tonemapping::SomewhatBoringDisplayTransform,
            ..Default::default()
        },
        PanOrbitCamera::default(),
        BloomSettings {
            intensity: 0.3,
            ..default()
        },
        EnvironmentMapLight {
            diffuse_map: assets.load("pisa_diffuse.ktx2"),
            specular_map: assets.load("pisa_specular.ktx2"),
            intensity: 500.0,
        },
        Name::new("camera"),
    ));

    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight {
            illuminance: 5000.0,
            shadows_enabled: true,
            ..Default::default()
        },
        transform: Transform::IDENTITY.looking_to(Vec3::new(2.5, -1., 0.85), Vec3::Y),
        ..default()
    });

    commands.spawn(SceneBundle {
        // Only load a single model when using `UnitOffset::CENTER_BASE`
        // If you attempt to load a scene containing several models using a setting other than the default of `UnitOffset::CENTER`,
        // their transforms will be messed up
        scene: assets.load("study.vox#workstation/desk"),
        ..default()
    });

    // Add a ground plane for the voxel desk to stand on
    commands.spawn(PbrBundle {
        mesh: meshes.add(Plane3d::new(Vec3::Y, Vec2::new(30., 30.))),
        material: materials.add(StandardMaterial {
            base_color: Color::LinearRgba(LinearRgba::GREEN),
            ..default()
        }),
        ..default()
    });
}
