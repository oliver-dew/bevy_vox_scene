use bevy::{
    ecs::{hierarchy::Children, name::Name},
    prelude::{Commands, Component, Entity, Event, Query, Trigger},
    scene::SceneInstanceReady,
};

use crate::{VoxelLayer, VoxelModelInstance};

/// An Event triggered once for each [`VoxelModelInstance`] in a scene, triggered after the scene is spawned and ready,
/// targeted at the entity containing the [`bevy::prelude::SceneRoot`].
///
/// The advantage of observing [`VoxelInstanceReady`] over using `Trigger<OnAdd, VoxelModelInstance>`,
/// is that [`VoxelInstanceReady`] is targeted at the [`bevy::prelude::SceneRoot`],
/// so you can scope your observer just to that spawn event:
///
/// ### Example
/// ```
/// # use bevy::prelude::*;
/// # use bevy_vox_scene::{VoxScenePlugin, VoxelInstanceReady};
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
///             mut trigger: Trigger<VoxelInstanceReady>,
///             mut commands: Commands,
/// #           mut exit: EventWriter<AppExit>,
///         | {
///             let Some(name) = &trigger.event().model_name else { return };
///             match name.as_str() {
///                 "workstation/computer" => {
///                     commands
///                         .entity(trigger.event().instance)
///                         .insert(Computer);
/// #                   exit.write(AppExit::Success);
///                 }
///                 _ => {}
///             }
///     });
/// }
/// ```
#[derive(Component, Event)]
pub struct VoxelInstanceReady {
    /// The entity on which the VoxelModelInstance spawned
    pub instance: Entity,
    /// The name of the model that spawned (if it has been named in the MagicaVoxel editor)
    pub model_name: Option<String>,
    /// The name of the model's layer (if it has been named in the MagicaVoxel editor)
    pub layer_name: Option<String>,
}

pub(crate) fn on_voxel_scene_ready(
    trigger: Trigger<SceneInstanceReady>,
    query: Query<(
        Option<&VoxelModelInstance>,
        Option<&Name>,
        Option<&VoxelLayer>,
        Option<&Children>,
    )>,
    commands: Commands,
) {
    seek_model_instance_recursive(trigger.target(), trigger.target(), query, commands);
}

fn seek_model_instance_recursive(
    root: Entity,
    entity: Entity,
    query: Query<(
        Option<&VoxelModelInstance>,
        Option<&Name>,
        Option<&VoxelLayer>,
        Option<&Children>,
    )>,
    mut commands: Commands,
) {
    let Ok((maybe_model, maybe_name, maybe_layer, maybe_children)) = query.get(entity) else {
        return;
    };
    if maybe_model.is_some() {
        let event = VoxelInstanceReady {
            instance: entity,
            model_name: maybe_name.map(|name| name.to_string()),
            layer_name: maybe_layer.map(|layer| layer.name.clone()).flatten(),
        };
        commands.trigger_targets(event, root);
    }
    if let Some(children) = maybe_children {
        for child in children {
            seek_model_instance_recursive(root, *child, query, commands.reborrow());
        }
    }
}
