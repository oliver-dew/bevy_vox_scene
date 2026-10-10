#![forbid(missing_docs, unsafe_code)]
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/README.md"))]
//! A plugin for the Bevy engine which allows loading Magica Voxel .vox files as scene graphs.
//!
//!```
//!use bevy::prelude::*;
//!use bevy_vox_scene::VoxScenePlugin;
//!use std::collections::HashSet;
//! # use bevy::{app::AppExit, world_serialization::WorldInstanceReady};
//!
//!fn main() {
//!    App::new()
//!    .add_plugins((
//!        DefaultPlugins,
//!        VoxScenePlugin::default(),
//!    ))
//!    .add_systems(Startup, setup)
//!    .run();
//!}
//!
//!fn setup(
//!    mut commands: Commands,
//!    assets: Res<AssetServer>,
//!) {
//!    // Load an entire scene graph
//!    commands.spawn(WorldAssetRoot(assets.load("study.vox")));
//!
//! #  commands.spawn(WorldAssetRoot(assets.load("study.vox"))).observe(assert_scene_loaded);
//!    // Load a single model using the name assigned to it in MagicaVoxel
//!    commands.spawn(WorldAssetRoot(assets.load("study.vox#workstation/desk")));
//!}
//! # fn assert_scene_loaded(
//! #     ready: On<WorldInstanceReady>,
//! #     children: Query<&Children>,
//! #     query: Query<&Name>,
//! #     mut exit: MessageWriter<AppExit>,
//! # ) {
//! #     let all_names: HashSet<&str> = children
//! #         .iter_descendants(ready.entity)
//! #         .filter_map(|child| query.get(child).map(|n| { n.as_str() } ).ok())
//! #         .collect();
//! #     if all_names.is_empty() { return };
//! #     let expected_names: HashSet<&str> = ["snowflake", "wall-tile", "brick-tile", "floor", "workstation", "workstation/keyboard" , "workstation/desk", "workstation/computer", "stairs", "glass", "tank", "tank/tetra", "tank/black-light", "tank/goldfish", "tank/wall", "tank/water", "tank/scenery"].into();
//! #     assert_eq!(all_names, expected_names, "only in actual: {:?}\nonly in expected: {:?}",
//! #     all_names.difference(&expected_names).collect::<Vec<_>>(),
//! #     expected_names.difference(&all_names).collect::<Vec<_>>());
//! #     exit.write(AppExit::Success);
//! # }
//!```

use bevy::{
    animation::AnimationPlugin,
    app::{App, Plugin},
    asset::AssetApp,
};

#[cfg(feature = "export")]
mod export;
mod flipbook_tests;
mod load;
mod model;
mod observers;

#[cfg(test)]
pub(crate) mod test_support;

#[cfg(feature = "export")]
pub use export::{ExportError, ExportedGlb, export_vox_to_glb};
use flipbook::FlipbookPlugin;
pub use flipbook::{FlipbookFrame, FlipbookPlayer, FlipbookRepeatMode};
#[doc(inline)]
use load::VoxSceneLoader;
pub use load::{
    VoxLoaderSettings, VoxelLayer, VoxelModelInstance, VoxelModelSource, VoxelModelSourcePolicy,
};
#[cfg(any(feature = "generate_voxels", feature = "modify_voxels"))]
pub use model::VoxelModelAssets;
#[cfg(feature = "generate_voxels")]
pub use model::VoxelSceneAssets;
#[cfg(feature = "generate_voxels")]
pub use model::sdf::SDF;
pub use model::{Voxel, VoxelData, VoxelElement, VoxelModel, VoxelPalette};
#[cfg(feature = "modify_voxels")]
pub use model::{
    modify::{ModifyVoxelError, VoxelRegion, VoxelRegionMode},
    queryable::VoxelQueryable,
};
pub use observers::VoxelInstanceReady;
pub use vox_bake::{
    AmbientOcclusionSettings, BakedImage, BakedMesh, BakedMeshAtlas, BakedPixelFormat, BakedVolume,
    TextureMipPolicy, TexturePackSettings, UnitOffset, VoxBakeSettings, VoxelTextureMode,
};
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
        if !app.is_plugin_added::<AnimationPlugin>() {
            app.add_plugins(AnimationPlugin);
        }
        app.add_plugins(FlipbookPlugin)
            .init_asset::<VoxelModel>()
            .init_asset::<VoxelPalette>()
            .register_type::<VoxelLayer>()
            .register_type::<VoxelModelInstance>()
            .register_type::<VoxelModelSource>()
            .register_asset_loader(VoxSceneLoader {
                global_settings: self.global_settings.clone(),
            })
            .add_observer(observers::on_voxel_scene_ready);
    }
}
