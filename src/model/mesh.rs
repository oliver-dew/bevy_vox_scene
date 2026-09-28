use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, Mesh, VertexAttributeValues},
    render::render_resource::PrimitiveTopology,
};
use vox_bake::{BakedMesh, BakedMeshAtlas};

pub(crate) type MeshAtlas = BakedMeshAtlas;

pub(crate) fn to_bevy_mesh(mesh: &BakedMesh, supports_remeshing: bool) -> Mesh {
    let mut bevy_mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        if supports_remeshing {
            RenderAssetUsages::default()
        } else {
            RenderAssetUsages::RENDER_WORLD
        },
    );
    bevy_mesh.insert_attribute(
        Mesh::ATTRIBUTE_POSITION,
        VertexAttributeValues::Float32x3(mesh.positions.clone()),
    );
    bevy_mesh.insert_attribute(
        Mesh::ATTRIBUTE_NORMAL,
        VertexAttributeValues::Float32x3(mesh.normals.clone()),
    );
    bevy_mesh.insert_attribute(
        Mesh::ATTRIBUTE_UV_0,
        VertexAttributeValues::Float32x2(mesh.uv0.clone()),
    );
    if let Some(uvs) = &mesh.uv1 {
        bevy_mesh.insert_attribute(
            Mesh::ATTRIBUTE_UV_1,
            VertexAttributeValues::Float32x2(uvs.clone()),
        );
    }
    bevy_mesh.insert_indices(Indices::U32(mesh.indices.clone()));
    bevy_mesh
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bevy_mesh_adapter_preserves_shared_buffers() {
        let baked = BakedMesh {
            positions: vec![[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]],
            normals: vec![[0.0, 1.0, 0.0]; 3],
            uv0: vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
            uv1: Some(vec![[0.25, 0.25], [0.75, 0.25], [0.25, 0.75]]),
            indices: vec![0, 1, 2],
        };
        let mesh = to_bevy_mesh(&baked, true);

        assert_eq!(
            mesh.attribute(Mesh::ATTRIBUTE_POSITION),
            Some(&VertexAttributeValues::Float32x3(baked.positions))
        );
        assert_eq!(
            mesh.attribute(Mesh::ATTRIBUTE_NORMAL),
            Some(&VertexAttributeValues::Float32x3(baked.normals))
        );
        assert_eq!(
            mesh.attribute(Mesh::ATTRIBUTE_UV_0),
            Some(&VertexAttributeValues::Float32x2(baked.uv0))
        );
        assert_eq!(
            mesh.attribute(Mesh::ATTRIBUTE_UV_1),
            Some(&VertexAttributeValues::Float32x2(baked.uv1.unwrap()))
        );
        assert_eq!(mesh.indices(), Some(&Indices::U32(baked.indices)));
    }
}
