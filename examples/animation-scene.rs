use bevy::{input::keyboard::KeyboardInput, prelude::*};
use bevy_vox_scene::{VoxScenePlugin, VoxelAnimationPlayer, VoxelInstanceReady};
use utilities::{PanOrbitCamera, PanOrbitCameraPlugin};

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            PanOrbitCameraPlugin,
            VoxScenePlugin::default(),
        ))
        .add_systems(Startup, setup)
        .add_systems(Update, toggle_pause.run_if(on_event::<KeyboardInput>))
        .run();
}

fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(30.0, 30.0, 60.0).looking_at(Vec3::ZERO, Vec3::Y),
        PanOrbitCamera::default(),
        EnvironmentMapLight {
            diffuse_map: assets.load("pisa_diffuse.ktx2"),
            specular_map: assets.load("pisa_specular.ktx2"),
            intensity: 500.0,
            ..default()
        },
    ));

    commands.spawn(SceneRoot(assets.load("deer.vox"))).observe(
        |trigger: Trigger<VoxelInstanceReady>, mut commands: Commands| {
            if trigger.event().model_name == Some("deer".to_string()) {
                // add marker component to scope pause action
                commands.entity(trigger.event().instance).insert(Deer);
            }
        },
    );
}

/// Press any key to toggle the pause state of the animation
fn toggle_pause(
    keys: Res<ButtonInput<KeyCode>>,
    mut query: Query<&mut VoxelAnimationPlayer, With<Deer>>,
) {
    let Ok(mut animation_player) = query.single_mut() else {
        return;
    };
    if keys.get_just_pressed().next().is_some() {
        animation_player.is_paused = !animation_player.is_paused;
    }
}

#[derive(Component)]
struct Deer;
