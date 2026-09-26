#[cfg(not(all(feature = "webgl2", target_arch = "wasm32")))]
use bevy::anti_alias::taa::TemporalAntiAliasing;
use bevy::{
    input::{common_conditions::input_just_pressed}, pbr::ScreenSpaceAmbientOcclusion, post_process::bloom::Bloom, prelude::*,
};
use bevy_vox_scene::{AmbientOcclusionSettings, VoxLoaderSettings, VoxScenePlugin};
use utilities::{PanOrbitCamera, PanOrbitCameraPlugin};

/// Press S to toggle Screen Space Ambient Occlusion
/// Press A to toggle Baked Ambient Occlusion
fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            PanOrbitCameraPlugin,
            VoxScenePlugin {
                global_settings: Some(VoxLoaderSettings {
                    ambient_occlusion: Some(AmbientOcclusionSettings {
                        depth: 4,
                        intensity: 1.0,
                    }),
                    ..default()
                })
            },
        ))
        .add_systems(Startup, setup)
        .add_systems(Update, (
            toggle_ssao.run_if(input_just_pressed(KeyCode::KeyS)),
            toggle_baked_ao.run_if(input_just_pressed(KeyCode::KeyA)),
        ))
        .run();
}

#[derive(Component)]
#[require(ScreenSpaceAmbientOcclusion)]
struct SSAOVisible(bool);

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn((
        Camera3d::default(),
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
            intensity: 500.0,
            ..default()
        },
        ScreenSpaceAmbientOcclusion::default(),
        SSAOVisible(true),
    ));
    commands.spawn(
        // Load a model nested inside a group by using a `/` to separate the path components
        WorldAssetRoot(asset_server.load("study.vox#workstation/desk")),
    );
}

fn toggle_ssao(
    mut commands: Commands,
    mut query: Query<(Entity, &mut SSAOVisible)>,
) {
    let Ok((entity, mut ssao_visible)) = query.single_mut() else {
        return;
    };
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

fn toggle_baked_ao(
    mut show_baked_ao: Local<bool>,
    mut occlusion_handle: Local<Option<Handle<Image>>>,
    material_handle: Single<&MeshMaterial3d<StandardMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(mut material) = materials.get_mut(&material_handle.0) else { return };
    *show_baked_ao = !(*show_baked_ao);
    if *show_baked_ao && let Some(handle) = &*occlusion_handle {
        material.occlusion_texture = Some(handle.clone());
    } else if let Some(handle) = &material.occlusion_texture {
        *occlusion_handle = Some(handle.clone());
        material.occlusion_texture = None
    }
}
