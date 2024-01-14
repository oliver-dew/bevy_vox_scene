use bevy::{
    asset::{Asset, Assets, Handle},
    core::Name,
    ecs::{
        bundle::Bundle,
        component::Component,
        entity::Entity,
        query::Without,
        system::{Commands, EntityCommands, Query, Res},
        world::{EntityRef, World},
    },
    hierarchy::{BuildChildren, Children},
    log::warn,
    math::{Mat3, Mat4, Quat, Vec3},
    pbr::{PbrBundle, StandardMaterial},
    reflect::TypePath,
    render::{mesh::Mesh, prelude::SpatialBundle, view::Visibility},
    transform::components::Transform,
    utils::HashMap,
};
use dot_vox::{Frame, SceneNode};

/// A component bundle for spawning Voxel Scenes.
///
/// The root of the spawned scene will be the entity that has this bundle.
/// ```no_run
/// # use bevy::prelude::*;
/// # use bevy_vox_scene::VoxelSceneBundle;
///
/// fn setup(
///     mut commands: Commands,
///     assets: Res<AssetServer>,
/// ) {
///     commands.spawn(VoxelSceneBundle {
///         scene: assets.load("study.vox"),
///         ..default()
///     });
///
///     commands.spawn(VoxelSceneBundle {
///         // Load a single model using the name assigned to it in MagicaVoxel.
///         // If a model is nested in a named group, than the group will form part of the path
///         // Path components are separated with a slash
///         scene: assets.load("study.vox#workstation/desk"),
///         ..default()
///     });
/// }
/// ```
#[derive(Bundle, Default)]
pub struct VoxelSceneBundle {
    /// A handle to a [`VoxelScene`], typically loaded from a ".vox" file via the [`bevy::asset::AssetServer`].
    /// This Entity will become the root of the spawned Voxel Scene.
    pub scene: Handle<VoxelScene>,
    /// The transform of the scene root. This will override whatever the root transform is in the Magica Voxel scene.
    pub transform: Transform,
    /// The visibility of the scene root. This will override whatever the root visibility is in the Magical Voxel scene.
    pub visibility: Visibility,
}

/// A component bundle for spawning Voxel Scenes, with a [`VoxelSceneHook`].
///
/// The root of the spawned scene will be the entity that has this bundle.
/// The [`VoxelSceneHook`] allows you to easily modify Entities deep within the scene hierarchy.
#[derive(Bundle, Default)]
pub struct VoxelSceneHookBundle {
    /// A handle to a [`VoxelScene`], typically loaded from a ".vox" file via the [`bevy::asset::AssetServer`].
    /// This Entity will become the root of the spawned Voxel Scene.
    pub scene: Handle<VoxelScene>,
    /// A [`VoxelSceneHook`] allows you to specify a closure that will be run for each Entity spawned in the scene graph.
    pub hook: VoxelSceneHook,
    /// The transform of the scene root. This will override whatever the root transform is in the Magica Voxel scene.
    pub transform: Transform,
    /// The visibility of the scene root. This will override whatever the root visibility is in the Magical Voxel scene.
    pub visibility: Visibility,
}

/// A representation of the Voxel Scene Graph.
///
/// To spawn a voxel scene, add a [Handle<VoxelScene>](VoxelScene), [`VoxelSceneBundle`], or [`VoxelSceneHookBundle`] to an Entity.
/// Voxel Scenes can be loaded from Magica Voxel .vox files.
#[derive(Asset, TypePath, Debug)]
pub struct VoxelScene {
    pub(crate) root: VoxelNode,
    pub(crate) models: Vec<VoxelModel>,
    pub(crate) layers: Vec<LayerInfo>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct VoxelNode {
    name: Option<String>,
    transform: Mat4,
    children: Vec<VoxelNode>,
    model_id: Option<usize>,
    is_hidden: bool,
    layer_id: u32,
}

#[derive(Debug, Clone)]
pub(crate) struct VoxelModel {
    pub mesh: Handle<Mesh>,
    pub material: Handle<StandardMaterial>,
}

#[derive(Debug, Clone)]
pub(crate) struct LayerInfo {
    pub name: Option<String>,
    pub is_hidden: bool,
}

/// A component specifying which layer the Entity belongs to, with an optional name.
///
/// This can be configured in the Magica Voxel world editor.
#[derive(Component, Clone)]
pub struct VoxelLayer {
    /// The identifier for the layer. Magic Voxel 0.99.6 allows you to assign nodes to one of 8 layers,
    /// so this value will be an index in the range 0 through 7.
    pub id: u32,
    /// An optional name for the Layer, assignable in Magica Voxel layer editor.
    pub name: Option<String>,
}

/// A component containing a closure that will be run against every entity spawned from a Voxel Scene
///
/// Assign this component to an entity that also has a [Handle<VoxelScene>](VoxelScene) to execute a closure
/// against every entity that gets spawned in the graph of the Voxel Scene.
/// This allows you to specify, before the scene graph has been spawned, how entities at a deeper level
/// than the root should be modified. A common use-case would adding custom components to entities
/// depending on their name or [`VoxelLayer`].
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
    hook: Box<dyn Fn(&EntityRef, &mut EntityCommands) + Send + Sync + 'static>,
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

pub(super) fn spawn_vox_scenes(
    mut commands: Commands,
    query: Query<(
        Entity,
        &Handle<VoxelScene>,
        Option<&Transform>,
        Option<&Visibility>,
    )>,
    vox_scenes: Res<Assets<VoxelScene>>,
) {
    for (root, scene_handle, transform, visibility) in query.iter() {
        if let Some(scene) = vox_scenes.get(scene_handle) {
            spawn_voxel_node_recursive(&mut commands, &scene.root, root, scene);
            let mut entity = commands.entity(root);
            entity.remove::<Handle<VoxelScene>>();
            if let Some(transform) = transform {
                entity.insert(*transform);
            }
            if let Some(visibility) = visibility {
                entity.insert(*visibility);
            }
        }
    }
}

fn spawn_voxel_node_recursive(
    commands: &mut Commands,
    voxel_node: &VoxelNode,
    entity: Entity,
    scene: &VoxelScene,
) {
    let mut entity_commands = commands.entity(entity);
    if let Some(name) = &voxel_node.name {
        entity_commands.insert(Name::new(name.clone()));
    }
    if let Some(model) = voxel_node.model_id.and_then(|id| {
        if let Some(model) = scene.models.get(id) {
            Some(model)
        } else {
            warn!("Model {} not found, omitting", id);
            None
        }
    }) {
        #[cfg(not(test))]
        entity_commands.insert(PbrBundle {
            mesh: model.mesh.clone(),
            material: model.material.clone(),
            ..Default::default()
        });
    } else {
        entity_commands.insert(SpatialBundle::default());
    }

    if let Some(layer_info) = scene.layers.get(voxel_node.layer_id as usize) {
        entity_commands.insert((
            VoxelLayer {
                id: voxel_node.layer_id,
                name: layer_info.name.clone(),
            },
            if voxel_node.is_hidden || layer_info.is_hidden {
                Visibility::Hidden
            } else {
                Visibility::Inherited
            },
        ));
    }
    entity_commands
        .insert(Transform::from_matrix(voxel_node.transform))
        .with_children(|builder| {
            for child in &voxel_node.children {
                let mut child_entity = builder.spawn_empty();
                let id = child_entity.id();
                spawn_voxel_node_recursive(child_entity.commands(), child, id, scene);
            }
        });
}

pub(super) fn run_hooks(
    mut commands: Commands,
    world: &World,
    query: Query<(Entity, &VoxelSceneHook), Without<Handle<VoxelScene>>>,
) {
    for (entity, scene_hook) in query.iter() {
        run_hook_recursive(&mut commands, world, entity, scene_hook);
        commands.entity(entity).remove::<VoxelSceneHook>();
    }
}

fn run_hook_recursive(
    commands: &mut Commands,
    world: &World,
    entity: Entity,
    scene_hook: &VoxelSceneHook,
) {
    let entity_ref = world.entity(entity);
    let mut entity_commands = commands.entity(entity);
    (scene_hook.hook)(&entity_ref, &mut entity_commands);
    let Some(children) = entity_ref.get::<Children>() else { return };
    for child in children.as_ref() {
        run_hook_recursive(commands, world, *child, scene_hook);
    }
}

pub(crate) fn parse_xform_node(
    graph: &Vec<SceneNode>,
    scene_node: &SceneNode,
    parent_name: Option<&String>,
) -> VoxelNode {
    match scene_node {
        SceneNode::Transform {
            attributes,
            frames,
            child,
            layer_id,
        } => {
            let (accumulated, node_name) =
                get_accumulated_and_node_name(parent_name, attributes.get("_name"));
            let mut vox_node = VoxelNode {
                name: node_name,
                transform: transform_from_frame(&frames[0]),
                is_hidden: parse_bool(attributes.get("_hidden").cloned()),
                layer_id: *layer_id,
                ..Default::default()
            };
            parse_xform_child(
                graph,
                &graph[*child as usize],
                &mut vox_node,
                accumulated.as_ref(),
            );
            vox_node
        }
        SceneNode::Group { .. } | SceneNode::Shape { .. } => {
            warn!("Found Group or Shape Node without a parent Transform");
            let mut vox_node = VoxelNode::default();
            parse_xform_child(graph, scene_node, &mut vox_node, parent_name);
            vox_node
        }
    }
}

pub(crate) fn find_subasset_names(
    subassets_by_name: &mut HashMap<String, VoxelNode>,
    node: &VoxelNode,
) {
    if let Some(name) = &node.name {
        if !subassets_by_name.contains_key(name) {
            subassets_by_name.insert(name.to_string(), node.clone());
        }
    }
    for child in &node.children {
        find_subasset_names(subassets_by_name, child);
    }
}

fn parse_xform_child(
    graph: &Vec<SceneNode>,
    scene_node: &SceneNode,
    partial_node: &mut VoxelNode,
    parent_name: Option<&String>,
) {
    match scene_node {
        SceneNode::Transform { .. } => {
            warn!("Found nested Transform nodes");
            partial_node.children = vec![parse_xform_node(graph, scene_node, parent_name)];
        }
        SceneNode::Group {
            attributes: _,
            children,
        } => {
            partial_node.children = children
                .iter()
                .map(|child| parse_xform_node(graph, &graph[*child as usize], parent_name))
                .collect();
        }
        SceneNode::Shape {
            attributes: _,
            models,
        } => {
            let model_id = models[0].model_id as usize;
            partial_node.model_id = Some(model_id);
        }
    }
}

fn get_accumulated_and_node_name(
    parent_name: Option<&String>,
    node_name: Option<&String>,
) -> (Option<String>, Option<String>) {
    match (parent_name, node_name) {
        (None, None) => (None, None),
        (None, Some(node_name)) => (Some(node_name.to_string()), Some(node_name.to_string())),
        (Some(parent_name), None) => (Some(parent_name.to_string()), None), // allow group name to pass down through unnamed child
        (Some(parent_name), Some(node_name)) => {
            let accumulated = format!("{}/{}", parent_name, node_name);
            (Some(accumulated.clone()), Some(accumulated))
        }
    }
}

fn parse_bool(value: Option<String>) -> bool {
    match value.as_deref() {
        Some("1") => true,
        Some("0") => false,
        Some(_) => {
            warn!("Invalid boolean string");
            false
        }
        None => false,
    }
}

fn transform_from_frame(frame: &Frame) -> Mat4 {
    let Some(position) = frame.position() else { return Mat4::IDENTITY };
    let position = [-position.x as f32, position.z as f32, position.y as f32];
    let translation = Mat4::from_translation(Vec3::from_array(position));
    let rotation = if let Some(orientation) = frame.orientation() {
        let (rotation, scale) = &orientation.to_quat_scale();
        let scale: Vec3 = (*scale).into();
        let quat = Quat::from_array(*rotation);
        let (axis, angle) = quat.to_axis_angle();
        let mat3 = Mat3::from_axis_angle(Vec3::new(-axis.x, axis.z, axis.y), angle)
            * Mat3::from_diagonal(scale);
        Mat4::from_mat3(mat3)
    } else {
        Mat4::IDENTITY
    };
    translation * rotation
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::VoxScenePlugin;
    use bevy::{
        app::App,
        asset::{AssetApp, AssetPlugin, AssetServer, LoadState},
        hierarchy::Children,
        render::texture::ImagePlugin,
        MinimalPlugins,
    };

    #[async_std::test]
    async fn test_load_scene() {
        let mut app = App::new();
        let handle = setup_and_load_voxel_scene(&mut app, "test.vox").await;
        app.update();
        let scene = app
            .world
            .resource::<Assets<VoxelScene>>()
            .get(handle)
            .expect("retrieve test.vox from Res<Assets>");
        assert_eq!(
            scene.models.len(),
            3,
            "Same 3 models are instanced through the scene"
        );
        assert_eq!(scene.layers.len(), 8);
        assert_eq!(
            scene
                .layers
                .first()
                .unwrap()
                .name
                .as_ref()
                .expect("Layer 0 name"),
            "scenery"
        );
        let outer_group = scene.root.children.first().expect("First object in scene");
        assert_eq!(
            outer_group.name.as_ref().expect("Name of first obj"),
            "outer-group"
        );
        assert_eq!(outer_group.children.len(), 3);
        let inner_group = outer_group
            .children
            .first()
            .expect("First child of outer-group");
        assert_eq!(
            inner_group.name.as_ref().expect("name of inner group"),
            "outer-group/inner-group"
        );
    }

    #[async_std::test]
    async fn test_load_scene_slice() {
        let mut app = App::new();
        let handle = setup_and_load_voxel_scene(&mut app, "test.vox#outer-group/inner-group").await;
        app.update();
        let scene = app
            .world
            .resource::<Assets<VoxelScene>>()
            .get(handle)
            .expect("retrieve test.vox from Res<Assets>");
        assert_eq!(
            scene.models.len(),
            3,
            "Same 3 models are instanced through the scene"
        );
        assert_eq!(scene.layers.len(), 8);
        assert_eq!(
            scene
                .layers
                .first()
                .unwrap()
                .name
                .as_ref()
                .expect("Layer 0 name"),
            "scenery"
        );
        let inner_group = &scene.root;
        assert_eq!(
            inner_group.name.as_ref().expect("Name of first obj"),
            "outer-group/inner-group"
        );
        assert_eq!(inner_group.children.len(), 4);
        let dice = inner_group
            .children
            .last()
            .expect("Last child of inner-group");
        assert_eq!(
            dice.name.as_ref().expect("name of dice"),
            "outer-group/inner-group/dice"
        );
    }

    #[async_std::test]
    async fn test_transmissive_mat() {
        let mut app = App::new();
        let handle =
            setup_and_load_voxel_scene(&mut app, "test.vox#outer-group/inner-group/walls").await;
        app.update();
        let scene = app
            .world
            .resource::<Assets<VoxelScene>>()
            .get(handle)
            .expect("retrieve scene from Res<Assets>");
        let walls = &scene.root;
        let mat_handle = &scene.models[walls.model_id.expect("walls model_id")].material;
        let material = app
            .world
            .resource::<Assets<StandardMaterial>>()
            .get(mat_handle)
            .expect("material");
        assert!(material.specular_transmission_texture.is_some());
        assert_eq!(material.specular_transmission, 1.0);
        assert!((material.ior - 1.3).abs() / 1.3 <= 0.00001);
        assert!(material.metallic_roughness_texture.is_some());
    }

    #[async_std::test]
    async fn test_opaque_mat() {
        let mut app = App::new();
        let handle =
            setup_and_load_voxel_scene(&mut app, "test.vox#outer-group/inner-group/dice").await;
        app.update();
        let scene = app
            .world
            .resource::<Assets<VoxelScene>>()
            .get(handle)
            .expect("retrieve scene from Res<Assets>");
        let dice = &scene.root;
        let mat_handle = &scene.models[dice.model_id.expect("dice model_id")].material;
        let material = app
            .world
            .resource::<Assets<StandardMaterial>>()
            .get(mat_handle)
            .expect("material");
        assert!(material.specular_transmission_texture.is_none());
        assert_eq!(material.specular_transmission, 0.0);
        assert!(material.metallic_roughness_texture.is_some());
    }

    #[async_std::test]
    async fn test_spawn_system() {
        let mut app = App::new();
        let handle = setup_and_load_voxel_scene(&mut app, "test.vox#outer-group/inner-group").await;
        app.update();

        assert_eq!(
            app.world
                .resource::<AssetServer>()
                .load_state(handle.clone()),
            LoadState::Loaded
        );
        let entity = app
            .world
            .spawn(VoxelSceneHookBundle {
                scene: handle,
                hook: VoxelSceneHook::new(move |entity, _| {
                    let Some(name) = entity.get::<Name>() else { return };
                    let expected_names: [&'static str; 3] = [
                        "outer-group/inner-group",
                        "outer-group/inner-group/dice",
                        "outer-group/inner-group/walls",
                    ];
                    assert!(expected_names.contains(&name.as_str()));
                }),
                ..Default::default()
            })
            .id();
        app.update();
        assert!(app.world.get::<Handle<VoxelScene>>(entity).is_none());
        assert_eq!(
            app.world.query::<&VoxelLayer>().iter(&app.world).len(),
            5,
            "5 voxel nodes spawned in this scene slice"
        );
        assert_eq!(
            app.world.query::<&Name>().iter(&app.world).len(),
            3,
            "But only 3 of the voxel nodes are named"
        );
        assert_eq!(
            app.world
                .get::<Name>(entity)
                .expect("Name component")
                .as_str(),
            "outer-group/inner-group"
        );
        let children = app
            .world
            .get::<Children>(entity)
            .expect("children of inner-group")
            .as_ref();
        assert_eq!(children.len(), 4, "inner-group has 4 children");
        assert_eq!(
            app.world
                .get::<Name>(*children.last().expect("last child"))
                .expect("Name component")
                .as_str(),
            "outer-group/inner-group/dice"
        );
        app.update(); // fire the hooks
    }

    /// `await` the response from this and then call `app.update()`
    async fn setup_and_load_voxel_scene(
        app: &mut App,
        filename: &'static str,
    ) -> Handle<VoxelScene> {
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            ImagePlugin::default(),
            VoxScenePlugin,
        ))
        .init_asset::<StandardMaterial>()
        .init_asset::<Mesh>();
        let assets = app.world.resource::<AssetServer>();
        assets
            .load_untyped_async(filename)
            .await
            .expect(format!("Loaded {filename}").as_str())
            .typed::<VoxelScene>()
    }
}
