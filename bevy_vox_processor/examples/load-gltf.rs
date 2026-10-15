use bevy::{light::light_consts::lux, post_process::bloom::Bloom, prelude::*};
use bevy_vox_processor::VoxGltfProcessorPlugin;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(AssetPlugin {
                mode: AssetMode::Processed,
                ..default()
            }),
            VoxGltfProcessorPlugin,
        ))
        .add_systems(Startup, spawn_scene)
        .run();
}

fn spawn_scene(mut commands: Commands, assets: Res<AssetServer>) {
    let scene = assets.load(GltfAssetLabel::Scene(0).from_asset("knight.vox"));
    commands.spawn(WorldAssetRoot(scene));
    commands.spawn((
        Camera3d::default(),
        Bloom::ANAMORPHIC,
        Transform::from_xyz(1., 2., -5.).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: lux::CIVIL_TWILIGHT,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(2.0, 8.0, -3.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}
