use bevy::{
    core::Name,
    prelude::{Commands, Component, Entity, Event, OnAdd, Parent, Query, Trigger},
};

use crate::VoxelModelInstance;

/// An Event triggered when a [`VoxelModelInstance`] is spawned.
///
/// The advantage of observing [`VoxelInstanceSpawned`] over using `Trigger<OnAdd, VoxelModelInstance>`,
/// is that [`VoxelInstanceSpawned`] bubbles upwards through its parents, so you can observe it on
/// the [`bevy::prelude::SceneRoot`], and scope your observer to one branch of your scene:
///
/// ### Example
/// ```
/// # use bevy::prelude::*;
/// # use bevy_vox_scene::{VoxScenePlugin, VoxelInstanceSpawned};
/// #
/// # fn main() {
/// #     App::new()
/// #         .add_plugins((
/// #             DefaultPlugins,
/// #             VoxScenePlugin::default()
/// #         ))
/// #         .add_systems(Startup, setup)
/// #     .run();
/// # }
/// #
/// /// A marker component I want to insert into the scene
/// #[derive(Component)]
/// struct Computer;
///
/// fn setup(
///     mut commands: Commands,
///     assets: Res<AssetServer>,
/// ) {
///     // observer is scoped just to this branch
///     commands.spawn(SceneRoot(assets.load("study.vox#workstation")))
///         .observe(|
///             mut trigger: Trigger<VoxelInstanceSpawned>,
///             mut commands: Commands,
/// #           mut exit: EventWriter<AppExit>,
///         | {
///             let name = trigger.event().model_name.as_str();
///             match name {
///                 "workstation/computer" => {
///                     commands
///                         .entity(trigger.event().entity)
///                         .insert(Computer);
///                     trigger.propagate(false); // stop the event bubbling up further, if you like
/// #                   exit.send(AppExit::Success);
///                 }
///                 _ => {}
///             }
///     });
/// }
/// ```
#[derive(Component)]
pub struct VoxelInstanceSpawned {
    /// The entity on which the VoxelModelInstance spawned
    pub entity: Entity,
    /// The name of the model that spawned
    pub model_name: String,
}

impl Event for VoxelInstanceSpawned {
    type Traversal = &'static Parent;
    const AUTO_PROPAGATE: bool = true;
}

pub(crate) fn on_voxel_instance_spawned(
    trigger: Trigger<OnAdd, VoxelModelInstance>,
    mut commands: Commands,
    query: Query<&Name>,
) {
    let Ok(name) = query.get(trigger.entity()) else {
        return;
    };
    let event = VoxelInstanceSpawned {
        entity: trigger.entity(),
        model_name: name.to_string(),
    };
    commands.trigger_targets(event, trigger.entity());
}
