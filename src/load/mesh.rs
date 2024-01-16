use bevy::render::{
    mesh::{Indices, Mesh, VertexAttributeValues},
    render_resource::PrimitiveTopology,
};
use block_mesh::{greedy_quads, GreedyQuadsBuffer, RIGHT_HANDED_Y_UP_CONFIG};
use ndshape::Shape;

use super::voxel::VoxelData;

pub(crate) fn mesh_model(data: &VoxelData) -> Mesh {
    let mut greedy_quads_buffer = GreedyQuadsBuffer::new(data.shape.size() as usize);
    let quads_config = RIGHT_HANDED_Y_UP_CONFIG;
    greedy_quads(
        &data.voxels,
        &data.shape,
        [0; 3],
        data.shape.as_array().map(|x| x - 1),
        &quads_config.faces,
        &mut greedy_quads_buffer,
    );
    let [x, y, z] = data.shape.as_array().map(|x| x - 2);

    let num_indices = greedy_quads_buffer.quads.num_quads() * 6;
    let num_vertices = greedy_quads_buffer.quads.num_quads() * 4;

    let mut indices = Vec::with_capacity(num_indices);
    let mut positions = Vec::with_capacity(num_vertices);
    let mut normals = Vec::with_capacity(num_vertices);
    let mut uvs = Vec::with_capacity(num_vertices);

    let mut render_mesh = Mesh::new(PrimitiveTopology::TriangleList);

    for (group, face) in greedy_quads_buffer
        .quads
        .groups
        .iter()
        .zip(quads_config.faces.as_ref())
    {
        for quad in group.iter() {
            let palette_index = data.voxels[data.shape.linearize(quad.minimum) as usize].index;
            indices.extend_from_slice(&face.quad_mesh_indices(positions.len() as u32));
            positions.extend_from_slice(
                &face
                    .quad_mesh_positions(quad, 1.0)
                    .map(|position| position.map(|x| x - 1.0)) // corrects the 1 offset introduced by the meshing.
                    .map(|position| {
                        [
                            position[0] - (x as f32) / 2.0,
                            position[1] - (y as f32) / 2.0,
                            position[2] - (z as f32) / 2.0,
                        ]
                    }), // move center of the mesh center
            );
            let u = ((palette_index % 16) as f32 + 0.5) / 16.0;
            let v = ((palette_index / 16) as f32 + 0.5) / 16.0;
            uvs.extend_from_slice(&[[u, v], [u, v], [u, v], [u, v]]);
            normals.extend_from_slice(&face.quad_mesh_normals());
        }
    }

    render_mesh.insert_attribute(
        Mesh::ATTRIBUTE_POSITION,
        VertexAttributeValues::Float32x3(positions),
    );

    render_mesh.insert_attribute(
        Mesh::ATTRIBUTE_NORMAL,
        VertexAttributeValues::Float32x3(normals),
    );
    render_mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, VertexAttributeValues::Float32x2(uvs));

    render_mesh.set_indices(Some(Indices::U32(indices.clone())));

    render_mesh
}
