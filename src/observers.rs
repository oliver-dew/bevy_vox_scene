use crate::{VoxelLayer, VoxelModelInstance};
use bevy::{
    ecs::{
        event::EntityEvent,
        hierarchy::{ChildOf, Children},
        name::Name,
    },
    platform::collections::HashSet,
    prelude::{Commands, Component, Entity, On, Query, With},
    world_serialization::WorldInstanceReady,
};
use flipbook::FlipbookPlayer;

/// An event triggered once for each logical voxel model after the scene is spawned and ready,
/// targeted at the entity containing the [`bevy::prelude::WorldAssetRoot`]. Static models report
/// their [`VoxelModelInstance`] entity. Flipbook models report the parent entity containing their
/// [`FlipbookPlayer`], rather than each individual frame entity.
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
    /// The static model entity, or the parent entity of an animated flipbook model.
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
    animation: Query<(), With<FlipbookPlayer>>,
    parents: Query<&ChildOf>,
    name_layer: Query<(Option<&Name>, Option<&VoxelLayer>)>,
    mut commands: Commands,
) {
    let mut reported_animations = HashSet::<Entity>::new();
    for child in children.iter_descendants(vox_scene.entity) {
        if instance.contains(child) {
            let logical_instance =
                animation_ancestor(child, vox_scene.entity, &animation, &parents).unwrap_or(child);
            if logical_instance != child && !reported_animations.insert(logical_instance) {
                continue;
            }
            let (maybe_name, maybe_layer) =
                name_layer.get(logical_instance).unwrap_or((None, None));
            let event = VoxelInstanceReady {
                scene_root: vox_scene.entity,
                instance: logical_instance,
                model_name: maybe_name.map(|name| name.to_string()),
                layer_name: maybe_layer.map(|layer| layer.name.clone()).flatten(),
            };
            commands.trigger(event);
        }
    }
}

fn animation_ancestor(
    mut entity: Entity,
    scene_root: Entity,
    animation: &Query<(), With<FlipbookPlayer>>,
    parents: &Query<&ChildOf>,
) -> Option<Entity> {
    while entity != scene_root {
        if animation.contains(entity) {
            return Some(entity);
        }
        entity = parents.get(entity).ok()?.parent();
    }
    None
}
