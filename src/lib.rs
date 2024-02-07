#![forbid(missing_docs, unsafe_code)]
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/README.md"))]
//! A plugin for the Bevy engine which allows loading Magica Voxel .vox files as scene graphs.
//!
//!```
//!use bevy::prelude::*;
//!use bevy_vox_scene::{VoxScenePlugin, VoxelSceneBundle};
//! # use bevy::{app::AppExit, utils::HashSet};
//!
//!fn main() {
//!    App::new()
//!    .add_plugins((
//!        DefaultPlugins,
//!        VoxScenePlugin,
//!    ))
//!    .add_systems(Startup, setup)
//! #   .add_systems(Update, assert_scene_loaded)
//!    .run();
//!}
//!
//!fn setup(
//!    mut commands: Commands,
//!    assets: Res<AssetServer>,
//!) {
//!    // Load an entire scene graph
//!    commands.spawn(VoxelSceneBundle {
//!        scene: assets.load("study.vox"),
//!        ..default()
//!    });
//!
//!    // Load a single model using the name assigned to it in MagicaVoxel
//!    commands.spawn(VoxelSceneBundle {
//!        scene: assets.load("study.vox#workstation/desk"),
//!        ..default()
//!    });
//!}
//! # fn assert_scene_loaded(
//! #     query: Query<&Name>,
//! #     mut exit: EventWriter<AppExit>,
//! # ) {
//! #     let all_names: HashSet<&str> = query.iter().map(|n| { n.as_str()} ).collect();
//! #     if all_names.is_empty() { return };
//! #     let expected_names: HashSet<&str> = ["snowflake", "wall-tile", "brick-tile", "floor", "workstation", "workstation/keyboard" , "workstation/desk", "workstation/computer", "stairs", "glass", "tank", "tank/tetra", "tank/black-light", "tank/goldfish", "tank/wall", "tank/water", "tank/scenery"].into();
//! #     assert_eq!(all_names, expected_names);
//! #     exit.send(AppExit);
//! # }
//!```

use bevy::{
    app::{App, Plugin, SpawnScene},
    asset::AssetApp,
    ecs::schedule::IntoSystemConfigs,
};

mod load;
mod model;
mod scene;

#[cfg(test)]
mod tests;

pub use load::VoxLoaderSettings;
#[doc(inline)]
use load::VoxSceneLoader;
#[cfg(feature = "modify_voxels")]
pub use model::{
    modify::{ModifyVoxelCommandsExt, VoxelRegion, VoxelRegionMode},
    queryable::VoxelQueryable,
};
pub use model::{
    sdf::SDF, VoxelModelCollection, Voxel, VoxelData, VoxelElement, VoxelModel, VoxelPalette,
};
pub use scene::{
    VoxelLayer, VoxelModelInstance, VoxelScene, VoxelSceneBundle, VoxelSceneHook,
    VoxelSceneHookBundle,
};

/// Plugin adding functionality for loading `.vox` files.
///
/// Registers an [`bevy::asset::AssetLoader`] capable of loading `.vox` files as spawnable [`VoxelScene`]s.
pub struct VoxScenePlugin;

impl Plugin for VoxScenePlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<VoxelScene>()
            .init_asset::<VoxelModelCollection>()
            .register_asset_loader(VoxSceneLoader)
            .add_systems(
                SpawnScene,
                (scene::systems::spawn_vox_scenes, scene::systems::run_hooks).chain(),
            );
    }
}
