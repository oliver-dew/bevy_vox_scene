use bevy::{pbr::EarthlikeAtmosphere, prelude::*};
use bevy_vox_scene::VoxScenePlugin;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, VoxScenePlugin::default()))
        .add_systems(Startup, setup)
        .run();
}

fn setup(
    mut commands: Commands,
    assets: Res<AssetServer>,
    earthlike_atmosphere: Res<EarthlikeAtmosphere>,
) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(30.0, 30.0, 60.0).looking_at(Vec3::ZERO, Vec3::Y),
        earthlike_atmosphere.get(),
    ));

    commands.spawn(
        // Load a single model using the name assigned to it in MagicaVoxel
        // If a model is nested in a named group, than the group will form part of the path
        // Path components are separated with a slash
        SceneRoot(assets.load("study.vox#workstation/desk")),
    );

    commands.spawn((
        DirectionalLight::default(),
        Transform::IDENTITY.looking_to(Vec3::new(2.5, -1., 0.85), Vec3::Y),
    ));
}
