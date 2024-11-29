use bevy::{
    core_pipeline::{bloom::Bloom, tonemapping::Tonemapping},
    prelude::*,
};
use bevy_vox_scene::{UnitOffset, VoxLoaderSettings, VoxScenePlugin};
use utilities::{PanOrbitCamera, PanOrbitCameraPlugin};

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            VoxScenePlugin {
                // Using global settings because Bevy's `load_with_settings` has a couple of issues:
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
        Camera3d::default(),
        Camera {
            hdr: true,
            ..Default::default()
        },
        Transform::from_xyz(8.0, 1.5, 8.0).looking_at(Vec3::ZERO, Vec3::Y),
        Tonemapping::SomewhatBoringDisplayTransform,
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
        Name::new("camera"),
    ));

    commands.spawn((
        DirectionalLight {
            illuminance: 5000.0,
            shadows_enabled: true,
            ..Default::default()
        },
        Transform::IDENTITY.looking_to(Vec3::new(2.5, -1., 0.85), Vec3::Y),
    ));

    commands.spawn(
        // Only load a single model when using `UnitOffset::CENTER_BASE`
        // If you attempt to load a scene containing several models using a setting other than the default of `UnitOffset::CENTER`,
        // their transforms will be messed up
        SceneRoot(assets.load("study.vox#workstation/desk")),
    );

    // Add a ground plane for the voxel desk to stand on
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::new(Vec3::Y, Vec2::new(30., 30.)))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::LinearRgba(LinearRgba::GREEN),
            ..default()
        })),
    ));
}
