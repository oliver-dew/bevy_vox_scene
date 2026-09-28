use block_mesh::{GreedyQuadsBuffer, RIGHT_HANDED_Y_UP_CONFIG, UnorientedQuad, greedy_quads};
use ndshape::{RuntimeShape, Shape};

use crate::VisibleVoxel;

/// One rectangular surface produced by greedy voxel meshing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BakedQuad {
    /// Minimum voxel-grid coordinate of the quad.
    pub minimum: [u32; 3],
    /// Quad extent along its local horizontal axis.
    pub width: u32,
    /// Quad extent along its local vertical axis.
    pub height: u32,
    /// Index into the shared right-handed Y-up face configuration.
    pub face_index: usize,
}

/// Vertex data for one baked quad before it is appended to a mesh buffer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BakedQuadGeometry {
    /// Four vertex positions with the requested origin offset applied.
    pub positions: [[f32; 3]; 4],
    /// Four outward-facing unit normals.
    pub normals: [[f32; 3]; 4],
    /// Six triangle indices relative to `first_vertex`.
    pub indices: [u32; 6],
}

/// Generate engine-independent positions, normals, and triangle indices for a quad.
pub fn bake_quad_geometry(
    quad: &BakedQuad,
    voxel_size: f32,
    position_offset: [f32; 3],
    first_vertex: u32,
) -> BakedQuadGeometry {
    let face = &RIGHT_HANDED_Y_UP_CONFIG.faces[quad.face_index];
    let block_quad = UnorientedQuad {
        minimum: quad.minimum,
        width: quad.width,
        height: quad.height,
    };
    BakedQuadGeometry {
        positions: face
            .quad_mesh_positions(&block_quad, voxel_size)
            .map(|position| std::array::from_fn(|axis| position[axis] - position_offset[axis])),
        normals: face.quad_mesh_normals(),
        indices: face.quad_mesh_indices(first_vertex),
    }
}

/// Greedily merge visible voxel faces into rectangular quads.
pub fn greedy_mesh_quads(voxels: &[VisibleVoxel], dimensions: [u32; 3]) -> Vec<BakedQuad> {
    let shape = RuntimeShape::<u32, 3>::new(dimensions);
    assert_eq!(
        voxels.len(),
        shape.size() as usize,
        "voxel count must match grid dimensions"
    );
    if dimensions.contains(&0) {
        return Vec::new();
    }
    let mut buffer = GreedyQuadsBuffer::new(voxels.len());
    greedy_quads(
        voxels,
        &shape,
        [0; 3],
        dimensions.map(|dimension| dimension - 1),
        &RIGHT_HANDED_Y_UP_CONFIG.faces,
        &mut buffer,
    );
    buffer
        .quads
        .groups
        .iter()
        .enumerate()
        .flat_map(|(face_index, group)| {
            group.iter().map(move |quad| BakedQuad {
                minimum: quad.minimum,
                width: quad.width,
                height: quad.height,
                face_index,
            })
        })
        .collect()
}
