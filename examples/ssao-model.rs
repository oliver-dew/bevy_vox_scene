use bevy::{
    core_pipeline::{
        bloom::Bloom,
        experimental::taa::{TemporalAntiAliasPlugin, TemporalAntiAliasing},
    },
    input::keyboard::KeyboardInput,
    pbr::ScreenSpaceAmbientOcclusion,
    prelude::*,
};
use bevy_vox_scene::VoxScenePlugin;
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
    .add_systems(Update, toggle_ssao.run_if(on_event::<KeyboardInput>));

    // *Note:* TAA is not _required_ for SSAO, but
    // it enhances the look of the resulting blur effects.
    // Sadly, it's not available under WebGL.
    #[cfg(not(all(feature = "webgl2", target_arch = "wasm32")))]
    app.add_plugins(TemporalAntiAliasPlugin);

    app.run();
}

#[derive(Component)]
#[require(ScreenSpaceAmbientOcclusion)]
struct SSAOVisible(bool);

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn((
        Camera3d::default(),
        Camera {
            hdr: true,
            ..default()
        },
        Transform::from_xyz(20.0, 10.0, 40.0).looking_at(Vec3::ZERO, Vec3::Y),
        PanOrbitCamera::default(),
        Bloom {
            intensity: 0.3,
            ..default()
        },
        #[cfg(not(all(feature = "webgl2", target_arch = "wasm32")))]
        Msaa::Off,
        #[cfg(not(all(feature = "webgl2", target_arch = "wasm32")))]
        TemporalAntiAliasing::default(),
        EnvironmentMapLight {
            diffuse_map: asset_server.load("pisa_diffuse.ktx2"),
            specular_map: asset_server.load("pisa_specular.ktx2"),
            intensity: 2000.0,
            ..default()
        },
        ScreenSpaceAmbientOcclusion::default(),
        SSAOVisible(true),
    ));

    commands.spawn(
        // Load a model nested inside a group by using a `/` to separate the path components
        SceneRoot(asset_server.load("study.vox#tank/goldfish")),
    );
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
                    .insert(ScreenSpaceAmbientOcclusion::default());
            }
            false => {
                commands
                    .entity(entity)
                    .remove::<ScreenSpaceAmbientOcclusion>();
            }
        }
    }
}
