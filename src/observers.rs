use bevy::{
    asset::Assets,
    core::Name,
    pbr::{FogVolume, MeshMaterial3d},
    prelude::{
        BuildChildren, Commands, Component, Entity, Event, Mesh3d, OnAdd, Parent, Query, Res,
        Transform, Trigger,
    },
};

use crate::{VoxelLayer, VoxelModel, VoxelModelInstance, VoxelQueryable};

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
///             let Some(name) = &trigger.event().model_name else { return };
///             match name.as_str() {
///                 "workstation/computer" => {
///                     commands
///                         .entity(trigger.event().entity)
///                         .insert(Computer);
///                     // If you want, you can stop the event bubbling up further
///                     // in this case I only want there to be one `Computer` marker in the scene:
///                     trigger.propagate(false);
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
    pub model_name: Option<String>,
    /// The name of the model's layer (if it has been named in the MagicaVoxel editor)
    pub layer_name: Option<String>,
}

impl Event for VoxelInstanceSpawned {
    type Traversal = &'static Parent;
    const AUTO_PROPAGATE: bool = true;
}

pub(crate) fn on_voxel_instance_spawned(
    trigger: Trigger<OnAdd, VoxelModelInstance>,
    models: Res<Assets<VoxelModel>>,
    mut commands: Commands,
    query: Query<(&VoxelModelInstance, Option<&Name>, Option<&VoxelLayer>)>,
) {
    let Ok((model_instance, maybe_name, maybe_layer)) = query.get(trigger.entity()) else {
        return;
    };
    let Some(model) = models.get(&model_instance.model) else {
        return;
    };
    if model.cloud_image.is_some() {
        commands.entity(trigger.entity()).with_child((
            FogVolume {
                density_texture: model.cloud_image.clone(),
                absorption: 0.1,
                ..Default::default()
            },
            Transform::from_scale(model.model_size()),
        ));
    };
    if let Some(handle) = model.mesh.clone() {
        commands.entity(trigger.entity()).insert(Mesh3d(handle));
    };
    if let Some(handle) = model.material.clone() {
        commands
            .entity(trigger.entity())
            .insert(MeshMaterial3d(handle));
    };
    let event = VoxelInstanceSpawned {
        entity: trigger.entity(),
        model_name: maybe_name.map(|name| name.to_string()),
        layer_name: maybe_layer.map(|layer| layer.name.clone()).flatten(),
    };
    commands.trigger_targets(event, trigger.entity());
}
