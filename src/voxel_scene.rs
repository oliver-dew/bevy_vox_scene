use bevy::{ecs::{bundle::Bundle, component::Component, system::{Commands, Query, Res}, entity::Entity, event::{Event, EventWriter}}, asset::{Handle, Asset, Assets}, transform::components::Transform, reflect::TypePath, math::{Mat4, Vec3, Mat3, Quat}, render::{mesh::Mesh, view::Visibility, prelude::SpatialBundle}, pbr::{StandardMaterial, PbrBundle}, core::Name, hierarchy::BuildChildren, log::warn};
use dot_vox::{SceneNode, Frame};

#[derive(Bundle, Default)]
pub struct VoxelSceneBundle {
    pub scene: Handle<VoxelScene>,
    pub transform: Transform,
    pub visibility: Visibility,
}

#[derive(Asset, TypePath, Debug)]
pub struct VoxelScene {
    pub name: String,
    pub root: VoxelNode,
    pub models: Vec<VoxelModel>,
    pub layers: Vec<LayerInfo>,
}

#[derive(Debug, Clone, Default)]
pub struct VoxelNode {
    pub name: Option<String>,
    pub transform: Mat4,
    pub children: Vec<VoxelNode>,
    pub model_id: Option<usize>,
    pub is_hidden: bool,
    pub layer_id: u32,
}

#[derive(Debug, Clone)]
pub struct VoxelModel {
    pub mesh: Handle<Mesh>,
    pub material: Handle<StandardMaterial>,
}

#[derive(Debug, Clone)]
pub struct LayerInfo {
    pub name: Option<String>,
    pub is_hidden: bool,
}

#[derive(Component, Clone)]
pub struct VoxelLayer {
    pub id: u32,
    pub name: Option<String>,
}

#[derive(Event)]
pub struct VoxelEntityReady {
    pub scene_name: String,
    pub entity: Entity,
    pub name: String,
    pub layer_id: u32,
}

pub(crate) fn spawn_vox_scenes(
    mut commands: Commands,
    query: Query<(Entity, &Transform, &Visibility, &Handle<VoxelScene>)>,
    vox_scenes: Res<Assets<VoxelScene>>,
    mut event_writer: EventWriter<VoxelEntityReady>,
) {
    for (root, transform, visibility, scene_handle) in query.iter() {
        if let Some(scene) = vox_scenes.get(scene_handle) {
            spawn_voxel_node_recursive(&mut commands, &scene.root, root, scene, &mut event_writer);
            commands.entity(root)
            .remove::<Handle<VoxelScene>>()
            .insert((*transform, *visibility));
        }
    }
}

fn spawn_voxel_node_recursive(
    commands: &mut Commands,
    voxel_node: &VoxelNode,
    entity: Entity,
    scene: &VoxelScene,
    event_writer: &mut EventWriter<VoxelEntityReady>,
) {
    let mut entity_commands = commands.entity(entity);
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
            if voxel_node.is_hidden || layer_info.is_hidden { Visibility::Hidden } else { Visibility::Inherited },
        ));
    }
    entity_commands.insert(
        Transform::from_matrix(voxel_node.transform),
    ).with_children(|builder| {
        for child in &voxel_node.children {
            let mut child_entity = builder.spawn_empty();
            let id = child_entity.id();
            spawn_voxel_node_recursive(child_entity.commands(), child, id, scene, event_writer);
        }
    });
    if let Some(name) = &voxel_node.name {
        entity_commands.insert(Name::new(name.clone()));
        event_writer.send(VoxelEntityReady {
            scene_name: scene.name.clone(),
            entity, 
            name: name.to_string(), 
            layer_id: voxel_node.layer_id 
        });
    }
}

pub(crate) fn parse_xform_node(
    graph: &Vec<SceneNode>,
    scene_node: &SceneNode,
    parent_name: Option<&String>,
) -> VoxelNode {
    match scene_node {
        SceneNode::Transform { attributes, frames, child, layer_id } => {
            let (accumulated, node_name) = get_accumulated_and_node_name(parent_name, attributes.get("_name"));
            let mut vox_node = VoxelNode {
                name: node_name,
                transform: transform_from_frame(&frames[0]),
                is_hidden: parse_bool(attributes.get("_hidden").cloned()),
                layer_id: *layer_id,
                ..Default::default()
            };
            parse_xform_child(graph, &graph[*child as usize], &mut vox_node, accumulated.as_ref());
            vox_node                      
        }
        SceneNode::Group { .. } | SceneNode:: Shape { .. } => {
            warn!("Found Group or Shape Node without a parent Transform");
            let mut vox_node = VoxelNode::default();
            parse_xform_child(graph, scene_node, &mut vox_node, parent_name);
            vox_node
        }
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
            partial_node.children = vec![
            parse_xform_node(graph, scene_node, parent_name)
            ];
        }
        SceneNode::Group { attributes: _, children } => {
            partial_node.children = children.iter().map(|child| {
                parse_xform_node(graph, &graph[*child as usize], parent_name)
            }).collect();
        }
        SceneNode::Shape { attributes: _, models } => {
            let model_id = models[0].model_id as usize;
            partial_node.model_id = Some(model_id);
        }
    }
}

fn get_accumulated_and_node_name(
    parent_name: Option<&String>,
    node_name: Option<&String>
) -> (Option<String>, Option<String>) {
    match (parent_name, node_name) {
        (None, None) => (None, None),
        (None, Some(node_name)) => (Some(node_name.to_string()), Some(node_name.to_string())),
        (Some(parent_name), None) => (Some(parent_name.to_string()), None), // allow group name to pass down through unnamed child
        (Some(parent_name), Some(node_name)) => {
            let accumulated = format!("{}/{}", parent_name, node_name);
            (Some(accumulated.clone()), Some(accumulated))
        },
    }
}

fn parse_bool(value: Option<String>) -> bool {
    match value.as_deref() {
        Some("1") => true,
        Some("0") => false,
        Some(_) => {
            warn!("Invalid boolean string");
            false
        },
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
        let mat3 = Mat3::from_axis_angle(Vec3::new(-axis.x, axis.z, axis.y), angle) * Mat3::from_diagonal(scale);
        Mat4::from_mat3(mat3)
    } else {
        Mat4::IDENTITY
    };
    translation * rotation
}

#[cfg(test)]
mod tests {
    use bevy::{app::App, asset::{AssetPlugin, AssetServer, LoadState, AssetApp}, MinimalPlugins, render::texture::ImagePlugin, hierarchy::Children, reflect::Enum};
    use crate::VoxScenePlugin;
    use super::*;
    
    #[async_std::test]
    async fn test_load_scene() {
        let mut app = App::new();
        let handle = setup_and_load_voxel_scene(&mut app, "test.vox").await;
        app.update();
        let scene = app.world.resource::<Assets<VoxelScene>>().get(handle).expect("retrieve test.vox from Res<Assets>");
        assert_eq!(scene.name, "test.vox");
        assert_eq!(scene.models.len(), 3, "Same 3 models are instanced through the scene");
        assert_eq!(scene.layers.len(), 8);
        assert_eq!(scene.layers.first().unwrap().name.as_ref().expect("Layer 0 name"), "scenery");
        let outer_group = scene.root.children.first().expect("First object in scene");
        assert_eq!(outer_group.name.as_ref().expect("Name of first obj"), "outer-group");
        assert_eq!(outer_group.children.len(), 3);
        let inner_group = outer_group.children.first().expect("First child of outer-group");
        assert_eq!(inner_group.name.as_ref().expect("name of inner group"), "outer-group/inner-group");
    }
    
    #[async_std::test]
    async fn test_load_scene_slice() {
        let mut app = App::new();
        let handle = setup_and_load_voxel_scene(&mut app, "test.vox#outer-group/inner-group").await;
        app.update();
        let scene = app.world.resource::<Assets<VoxelScene>>().get(handle).expect("retrieve test.vox from Res<Assets>");
        assert_eq!(scene.name, "test.vox#outer-group/inner-group");
        assert_eq!(scene.models.len(), 3, "Same 3 models are instanced through the scene");
        assert_eq!(scene.layers.len(), 8);
        assert_eq!(scene.layers.first().unwrap().name.as_ref().expect("Layer 0 name"), "scenery");
        let inner_group = &scene.root;
        assert_eq!(inner_group.name.as_ref().expect("Name of first obj"), "outer-group/inner-group");
        assert_eq!(inner_group.children.len(), 4);
        let dice = inner_group.children.last().expect("Last child of inner-group");
        assert_eq!(dice.name.as_ref().expect("name of dice"), "outer-group/inner-group/dice");
    }
    
    #[async_std::test]
    async fn test_transmissive_mat() {
        let mut app = App::new();
        let handle = setup_and_load_voxel_scene(&mut app, "test.vox#outer-group/inner-group/walls").await;
        app.update();
        let scene = app.world.resource::<Assets<VoxelScene>>().get(handle).expect("retrieve scene from Res<Assets>");
        let walls = &scene.root;
        let mat_handle = &scene.models[walls.model_id.expect("walls model_id")].material;
        let material = app.world.resource::<Assets<StandardMaterial>>().get(mat_handle).expect("material");
        assert!(material.specular_transmission_texture.is_some());
        assert_eq!(material.specular_transmission, 1.0);
        assert!((material.ior - 1.3).abs() / 1.3 <= 0.00001);
        assert!(material.metallic_roughness_texture.is_some());
    }

    #[async_std::test]
    async fn test_opaque_mat() {
        let mut app = App::new();
        let handle = setup_and_load_voxel_scene(&mut app, "test.vox#outer-group/inner-group/dice").await;
        app.update();
        let scene = app.world.resource::<Assets<VoxelScene>>().get(handle).expect("retrieve scene from Res<Assets>");
        let dice = &scene.root;
        let mat_handle = &scene.models[dice.model_id.expect("dice model_id")].material;
        let material = app.world.resource::<Assets<StandardMaterial>>().get(mat_handle).expect("material");
        assert!(material.specular_transmission_texture.is_none());
        assert_eq!(material.specular_transmission, 0.0);
        assert!(material.metallic_roughness_texture.is_some());
    }

    #[async_std::test]
    async fn test_spawn_system() {
        let mut app = App::new();
        let handle = setup_and_load_voxel_scene(&mut app, "test.vox#outer-group/inner-group").await;
        app.update();
        
        assert_eq!(app.world.resource::<AssetServer>().load_state(handle.clone()), LoadState::Loaded);
        let entity = app.world.spawn(VoxelSceneBundle {
            scene: handle,
            ..Default::default()
        }).id();
        app.update();
        
        assert!(app.world.get::<Handle<VoxelScene>>(entity).is_none());
        assert_eq!(app.world.query::<&VoxelLayer>().iter(&app.world).len(), 5, "5 voxel nodes spawned in this scene slice");
        assert_eq!(app.world.query::<&Name>().iter(&app.world).len(), 3, "But only 3 of the voxel nodes are named"); 
        assert_eq!(app.world.get::<Name>(entity).expect("Name component").as_str(), "outer-group/inner-group");
        let children = app.world.get::<Children>(entity).expect("children of inner-group").as_ref();
        assert_eq!(children.len(), 4, "inner-group has 4 children");
        assert_eq!(app.world.get::<Name>(*children.last().expect("last child")).expect("Name component").as_str(), "outer-group/inner-group/dice");
    }

    /// `await` the response from this and then call `app.update()` 
    async fn setup_and_load_voxel_scene(app: &mut App, filename: &'static str) -> Handle<VoxelScene> {
        app
        .add_plugins((MinimalPlugins, AssetPlugin::default(), ImagePlugin::default(), VoxScenePlugin))
        .init_asset::<StandardMaterial>()
        .init_asset::<Mesh>();
        let assets = app.world.resource::<AssetServer>();
        assets.load_untyped_async(filename).await.expect(format!("Loaded {filename}").as_str()).typed::<VoxelScene>()
    }
}
