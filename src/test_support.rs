use bevy::{
    MinimalPlugins,
    app::App,
    asset::{AssetApp, AssetPlugin, AssetServer, Handle},
    image::Image,
    mesh::MeshPlugin,
    pbr::{MaterialPlugin, StandardMaterial},
    world_serialization::{WorldAsset, WorldSerializationPlugin},
};

use crate::VoxScenePlugin;

pub(crate) async fn setup_and_load_voxel_scene(
    app: &mut App,
    filename: &'static str,
) -> Handle<WorldAsset> {
    setup_app(app);
    let assets = app.world().resource::<AssetServer>();
    assets
        .load_builder()
        .load_untyped_async(filename)
        .await
        .expect(format!("Loaded {filename}").as_str())
        .typed::<WorldAsset>()
}

pub(crate) fn setup_app(app: &mut App) {
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        WorldSerializationPlugin::default(),
        MeshPlugin,
        MaterialPlugin::<StandardMaterial>::default(),
        VoxScenePlugin::default(),
    ))
    .init_asset::<Image>();
}
