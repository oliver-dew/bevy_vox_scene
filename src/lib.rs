//! A plugin for the Bevy engine which allows loading Magica Voxel .vox files as scene graphs.
//!
//!```
//!use bevy::prelude::*;
//!use bevy_vox_scene::{VoxScenePlugin, VoxelSceneBundle};
//!
//!fn main() {
//!    App::new()
//!    .add_plugins((
//!        DefaultPlugins,
//!        VoxScenePlugin,
//!    ))
//!    .add_systems(Startup, setup)
//!    .run();
//!}
//!
//!fn setup(
//!    mut commands: Commands,
//!    assets: Res<AssetServer>,
//!) {
//!    
//!    // Load an entire scene graph
//!    commands.spawn(VoxelSceneBundle {
//!        scene: assets.load("study.vox"),
//!        ..default()
//!    });
//! 
//!    // Load a single model using the name assigned to it in MagicaVoxel
//!    commands.spawn(VoxelSceneBundle {
//!        scene: assets.load("study.vox#desk"),
//!        ..default()
//!    });
//!}
//!``` 
use bevy::{
    app::{App, Plugin, SpawnScene},
    asset::AssetApp, ecs::schedule::IntoSystemConfigs,
};

mod loader;
mod voxel_scene;
pub use voxel_scene::{VoxelSceneBundle, VoxelSceneHookBundle, VoxelScene, VoxelLayer, VoxelSceneHook};
pub use loader::VoxLoaderSettings;
#[doc(inline)]
use loader::VoxSceneLoader;
mod mesh;
mod voxel;

/// The core plugin adding functionality for loading `.vox` files.
///
/// Registers an [`bevy::asset::AssetLoader`] capable of loading modes in `.vox` files as usable [`bevy::render::mesh::Mesh`].
pub struct VoxScenePlugin;

impl Plugin for VoxScenePlugin {
    fn build(&self, app: &mut App) {
        app
        .init_asset::<voxel_scene::VoxelScene>()
        .register_asset_loader(VoxSceneLoader)
        .add_systems(SpawnScene, (
            voxel_scene::spawn_vox_scenes,
            voxel_scene::run_hooks
        ).chain());
    }
}
