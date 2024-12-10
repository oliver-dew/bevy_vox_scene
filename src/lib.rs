#![forbid(missing_docs, unsafe_code)]
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/README.md"))]
//! A plugin for the Bevy engine which allows loading Magica Voxel .vox files as scene graphs.
//!
//!```
//!use bevy::prelude::*;
//!use bevy_vox_scene::VoxScenePlugin;
//! # use bevy::{app::AppExit, utils::HashSet};
//!
//!fn main() {
//!    App::new()
//!    .add_plugins((
//!        DefaultPlugins,
//!        VoxScenePlugin::default(),
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
//!    commands.spawn(SceneRoot(assets.load("study.vox")));
//!
//!    // Load a single model using the name assigned to it in MagicaVoxel
//!    commands.spawn(SceneRoot(assets.load("study.vox#workstation/desk")));
//!}
//! # fn assert_scene_loaded(
//! #     query: Query<&Name>,
//! #     mut exit: EventWriter<AppExit>,
//! # ) {
//! #     let all_names: HashSet<&str> = query.iter().map(|n| { n.as_str()} ).collect();
//! #     if all_names.is_empty() { return };
//! #     let expected_names: HashSet<&str> = ["snowflake", "wall-tile", "brick-tile", "floor", "workstation", "workstation/keyboard" , "workstation/desk", "workstation/computer", "stairs", "glass", "tank", "tank/tetra", "tank/black-light", "tank/goldfish", "tank/wall", "tank/water", "tank/scenery"].into();
//! #     assert_eq!(all_names, expected_names);
//! #     exit.send(AppExit::Success);
//! # }
//!```

use bevy::{
    app::{App, Plugin, Update},
    asset::AssetApp,
};

mod load;
mod model;
mod observers;
mod systems;

#[cfg(test)]
mod tests;

#[doc(inline)]
use load::VoxSceneLoader;
pub use load::{
    UnitOffset, VoxLoaderSettings, VoxelAnimationPlayer, VoxelLayer, VoxelModelInstance,
};
#[cfg(feature = "generate_voxels")]
pub use model::sdf::SDF;
#[cfg(feature = "modify_voxels")]
pub use model::{
    modify::{ModifyVoxelCommandsExt, VoxelRegion, VoxelRegionMode},
    queryable::VoxelQueryable,
};
pub use model::{Voxel, VoxelContext, VoxelData, VoxelElement, VoxelModel, VoxelPalette};
pub use observers::VoxelInstanceSpawned;

/// Plugin adding functionality for loading `.vox` files.
///
/// Registers an [`bevy::asset::AssetLoader`] capable of loading `.vox` files as spawnable [`VoxelScene`]s.
#[derive(Default)]
pub struct VoxScenePlugin {
    /// Inject global settings. This is a workaround for `load_with_settings` currently being broken.
    /// See: https://github.com/bevyengine/bevy/issues/12320
    /// and: https://github.com/bevyengine/bevy/issues/11111
    pub global_settings: Option<VoxLoaderSettings>,
}

impl Plugin for VoxScenePlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<VoxelModel>()
            .init_asset::<VoxelContext>()
            .register_type::<VoxelLayer>()
            .register_type::<VoxelModelInstance>()
            .register_type::<VoxelAnimationPlayer>()
            .register_asset_loader(VoxSceneLoader {
                global_settings: self.global_settings.clone(),
            })
            .add_observer(observers::on_voxel_instance_spawned)
            .add_systems(Update, systems::update_animations);
    }
}
