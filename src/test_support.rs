use bevy::{
    MinimalPlugins,
    app::App,
    asset::{AssetApp, AssetPlugin, AssetServer, Handle},
    camera::visibility::VisibilityClass,
    image::ImagePlugin,
    light::FogVolume,
    mesh::Mesh,
    pbr::{MeshMaterial3d, StandardMaterial},
    prelude::{
        GlobalTransform, InheritedVisibility, Mesh3d, Transform, ViewVisibility, Visibility,
    },
    transform::components::TransformTreeChanged,
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
        ImagePlugin::default(),
        WorldSerializationPlugin::default(),
        VoxScenePlugin::default(),
    ))
    .init_asset::<StandardMaterial>()
    .init_asset::<Mesh>()
    .init_asset::<WorldAsset>()
    .register_type::<Visibility>()
    .register_type::<ViewVisibility>()
    .register_type::<InheritedVisibility>()
    .register_type::<VisibilityClass>()
    .register_type::<Transform>()
    .register_type::<GlobalTransform>()
    .register_type::<TransformTreeChanged>()
    .register_type::<Mesh3d>()
    .register_type::<MeshMaterial3d<StandardMaterial>>()
    .register_type::<FogVolume>();
}
