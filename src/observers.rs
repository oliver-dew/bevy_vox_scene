use bevy::{
    ecs::{event::EntityEvent, hierarchy::Children, name::Name},
    prelude::{Commands, Component, Entity, On, Query},
    world_serialization::WorldInstanceReady,
};

use crate::{VoxelLayer, VoxelModelInstance};

/// An Event triggered once for each [`VoxelModelInstance`] in a scene, triggered after the scene is spawned and ready,
/// targeted at the entity containing the [`bevy::prelude::WorldAssetRoot`].
///
/// The advantage of observing [`VoxelInstanceReady`] over using `Trigger<OnAdd, VoxelModelInstance>`,
/// is that [`VoxelInstanceReady`] is targeted at the [`bevy::prelude::WorldAssetRoot`],
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
///     commands.spawn(WorldAssetRoot(assets.load("study.vox#workstation")))
///         .observe(|
///             mut vox_instance: On<VoxelInstanceReady>,
///             mut commands: Commands,
/// #           mut exit: MessageWriter<AppExit>,
///         | {
///             let Some(name) = &vox_instance.event().model_name else { return };
///             match name.as_str() {
///                 "workstation/computer" => {
///                     commands
///                         .entity(vox_instance.event().instance)
///                         .insert(Computer);
/// #                   exit.write(AppExit::Success);
///                 }
///                 _ => {}
///             }
///     });
/// }
/// ```
#[derive(Component, EntityEvent)]
pub struct VoxelInstanceReady {
    /// The entity on which the [`bevy::scene::WorldAssetRoot`] was spawned
    #[event_target]
    pub scene_root: Entity,
    /// The entity on which the VoxelModelInstance spawned
    pub instance: Entity,
    /// The name of the model that spawned (if it has been named in the MagicaVoxel editor)
    pub model_name: Option<String>,
    /// The name of the model's layer (if it has been named in the MagicaVoxel editor)
    pub layer_name: Option<String>,
}

pub(crate) fn on_voxel_scene_ready(
    vox_scene: On<WorldInstanceReady>,
    children: Query<&Children>,
    instance: Query<&VoxelModelInstance>,
    name_layer: Query<(Option<&Name>, Option<&VoxelLayer>)>,
    mut commands: Commands,
) {
    for child in children.iter_descendants(vox_scene.entity) {
        if instance.contains(child) {
            let (maybe_name, maybe_layer) = name_layer.get(child).unwrap_or((None, None));
            let event = VoxelInstanceReady {
                scene_root: vox_scene.entity,
                instance: child,
                model_name: maybe_name.map(|name| name.to_string()),
                layer_name: maybe_layer.map(|layer| layer.name.clone()).flatten(),
            };
            commands.trigger(event);
        }
    }
}
