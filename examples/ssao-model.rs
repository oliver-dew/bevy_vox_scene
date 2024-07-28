use bevy::{
    core_pipeline::{
        bloom::BloomSettings,
        experimental::taa::{TemporalAntiAliasBundle, TemporalAntiAliasPlugin},
    },
    input::keyboard::KeyboardInput,
    pbr::ScreenSpaceAmbientOcclusionBundle,
    prelude::*,
};
use bevy_vox_scene::{VoxScenePlugin, VoxelSceneBundle};
use utilities::{PanOrbitCamera, PanOrbitCameraPlugin};

/// Press any key to toggle Screen Space Ambient Occlusion
fn main() {
    let mut app = App::new();

    app.add_plugins((
        DefaultPlugins,
        PanOrbitCameraPlugin,
        VoxScenePlugin::default(),
    ))
    .insert_resource(AmbientLight {
        color: Color::srgb_u8(128, 126, 124),
        brightness: 0.5,
    })
    .add_systems(Startup, setup)
    .add_systems(Update, toggle_ssao.run_if(on_event::<KeyboardInput>()));

    // *Note:* TAA is not _required_ for SSAO, but
    // it enhances the look of the resulting blur effects.
    // Sadly, it's not available under WebGL.
    #[cfg(not(all(feature = "webgl2", target_arch = "wasm32")))]
    app.insert_resource(Msaa::Off)
        .add_plugins(TemporalAntiAliasPlugin);

    app.run();
}

#[derive(Component)]
struct SSAOVisible(bool);

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands
        .spawn((
            Camera3dBundle {
                camera: Camera {
                    hdr: true,
                    ..Default::default()
                },
                transform: Transform::from_xyz(20.0, 10.0, 40.0).looking_at(Vec3::ZERO, Vec3::Y),
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
                diffuse_map: asset_server.load("pisa_diffuse.ktx2"),
                specular_map: asset_server.load("pisa_specular.ktx2"),
                intensity: 500.0,
            },
            SSAOVisible(true),
        ))
        .insert(ScreenSpaceAmbientOcclusionBundle::default());

    commands.spawn(VoxelSceneBundle {
        // Load a model nested inside a group by using a `/` to separate the path components
        scene: asset_server.load("study.vox#tank/goldfish"),
        ..default()
    });
}

fn toggle_ssao(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut query: Query<(Entity, &mut SSAOVisible)>,
) {
    let Ok((entity, mut ssao_visible)) = query.get_single_mut() else {
        return;
    };
    if keys.get_just_pressed().next().is_some() {
        ssao_visible.0 = !ssao_visible.0;
        match ssao_visible.0 {
            true => {
                commands
                    .entity(entity)
                    .insert(ScreenSpaceAmbientOcclusionBundle::default());
            }
            false => {
                commands
                    .entity(entity)
                    .remove::<ScreenSpaceAmbientOcclusionBundle>();
            }
        }
    }
}
