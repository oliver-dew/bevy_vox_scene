use bevy::{
    asset::LoadContext,
    log::warn,
    math::{Mat3, Mat4, Quat, Vec3},
    utils::HashMap,
};
use dot_vox::{Frame, SceneNode};

use crate::scene::VoxelNode;

pub(super) fn find_subasset_names(
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

pub(super) fn find_model_names(name_for_model: &mut Vec<Option<String>>, node: &VoxelNode) {
    if let Some(model_id) = &node.model_id {
        match (&name_for_model[*model_id], &node.name) {
            (None, Some(name)) | (Some(_), Some(name)) => {
                name_for_model[*model_id] = Some(name.to_string())
            }
            (None, None) | (Some(_), None) => (),
        };
    }
    for child in &node.children {
        find_model_names(name_for_model, child);
    }
}

pub(super) fn parse_xform_node(
    graph: &Vec<SceneNode>,
    scene_node: &SceneNode,
    parent_name: Option<&String>,
    load_context: &mut LoadContext,
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
                load_context,
            );
            vox_node
        }
        SceneNode::Group { .. } | SceneNode::Shape { .. } => {
            warn!("Found Group or Shape Node without a parent Transform");
            let mut vox_node = VoxelNode::default();
            parse_xform_child(graph, scene_node, &mut vox_node, parent_name, load_context);
            vox_node
        }
    }
}

fn parse_xform_child(
    graph: &Vec<SceneNode>,
    scene_node: &SceneNode,
    partial_node: &mut VoxelNode,
    parent_name: Option<&String>,
    load_context: &mut LoadContext,
) {
    match scene_node {
        SceneNode::Transform { .. } => {
            warn!("Found nested Transform nodes");
            partial_node.children = vec![parse_xform_node(
                graph,
                scene_node,
                parent_name,
                load_context,
            )];
        }
        SceneNode::Group {
            attributes: _,
            children,
        } => {
            partial_node.children = children
                .iter()
                .map(|child| {
                    parse_xform_node(graph, &graph[*child as usize], parent_name, load_context)
                })
                .collect();
        }
        SceneNode::Shape {
            attributes: _,
            models,
        } => {
            partial_node.model_id = Some(models[0].model_id as usize);
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
    let Some(position) = frame.position() else {
        return Mat4::IDENTITY;
    };
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
