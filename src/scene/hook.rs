use bevy::{
    ecs::{component::Component, system::EntityCommands, world::EntityRef},
    log::warn,
};

/// A component containing a closure that will be run against every entity spawned from a Voxel Scene
///
/// Assign this component to an entity that also has a [`Handle<VoxelScene>`](crate::VoxelScene) to execute a closure
/// against every entity that gets spawned in the graph of the Voxel Scene.
/// This allows you to specify, before the scene graph has been spawned, how entities at a deeper level
/// than the root should be modified. A common use-case would adding custom components to entities
/// depending on their name or [`crate::VoxelLayer`].
/// ```rust
/// # use bevy::{prelude::*, app::AppExit, utils::HashSet};
/// # use bevy_vox_scene::{VoxScenePlugin, VoxelSceneHook, VoxelSceneHookBundle};
/// #
/// # fn main() {
/// #     App::new()
/// #     .add_plugins((
/// #         DefaultPlugins,
/// #         VoxScenePlugin,
/// #     ))
/// #     .add_systems(Startup, setup)
/// #     .add_systems(Update, assert_scene_loaded)
/// #     .run();
/// # }
/// #
/// #[derive(Component)]
/// struct Fish;
///
/// fn setup(
///     mut commands: Commands,
///     assets: Res<AssetServer>,
/// ) {
///     commands.spawn((
///         VoxelSceneHookBundle {
///             scene: assets.load("study.vox#tank"),
///
///             // This closure will be run against every child Entity that gets spawned in the scene
///             hook: VoxelSceneHook::new(move |entity, commands| {
///                 let Some(name) = entity.get::<Name>() else { return };
///                 match name.as_str() {
///                     // Node names give the path to the asset, with components separated by /. Here, "goldfish" and "tetra" are two fish types in the "tank"
///                     "tank/goldfish" | "tank/tetra" => {
///                         // add a marker Component.
///                         commands.insert(Fish);
///                     }
///                     _ => {},
///                 }
///             }),
///             ..default()
///         },
///     ));
/// }
/// #
/// # fn assert_scene_loaded(
/// #     query: Query<&Name, With<Fish>>,
/// #     mut exit: EventWriter<AppExit>,
/// # ) {
/// #     let all_fish: Vec<&str> = query.iter().map(|n| { n.as_str() }).collect();
/// #     if all_fish.is_empty() { return };
/// #     assert_eq!(all_fish.len(), 5);
/// #     let expected_names: HashSet<&str> = ["tank/tetra", "tank/goldfish"].into();
/// #     let all_names: HashSet<&str> = HashSet::from_iter(all_fish);
/// #     assert_eq!(expected_names, all_names);
/// #     exit.send(AppExit);
/// # }
/// ```
#[derive(Component)]
pub struct VoxelSceneHook {
    pub(crate) hook: Box<dyn Fn(&EntityRef, &mut EntityCommands) + Send + Sync + 'static>,
}

impl VoxelSceneHook {
    /// Create a new hook with the closure `hook`. This will be run against every entity that gets spawned in the scene graph.
    pub fn new<F: Fn(&EntityRef, &mut EntityCommands) + Send + Sync + 'static>(hook: F) -> Self {
        Self {
            hook: Box::new(hook),
        }
    }
}

impl Default for VoxelSceneHook {
    fn default() -> Self {
        Self::new(|_, _| warn!("Default VoxelSceneHook does nothing"))
    }
}
