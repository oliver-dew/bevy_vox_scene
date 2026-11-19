use std::collections::{BTreeSet, HashMap};

use glam::{IVec3, Vec2, Vec3};

use crate::{BakedMesh, BakedQuad, VisibleVoxel, VoxelVisibility, bake_quad_geometry, linearize};

const INSET: f32 = 0.5;
const AXES: [IVec3; 3] = [IVec3::X, IVec3::Y, IVec3::Z];

pub(crate) struct QuadMapping {
    pub material: [[f32; 2]; 4],
    pub ao: Option<[[f32; 2]; 4]>,
}

struct Face {
    origin: Vec3,
    normal: Vec3,
    axes: [Vec3; 2],
    extent: [f32; 2],
}

impl Face {
    fn new(quad: &BakedQuad) -> Self {
        let geometry = bake_quad_geometry(quad, 1.0, [0.0; 3], 0);
        let [origin, u, v, _] = geometry.positions.map(Vec3::from_array);
        let extent = [quad.width as f32, quad.height as f32];
        Self {
            origin,
            normal: Vec3::from_array(geometry.normals[0]),
            axes: [(u - origin) / extent[0], (v - origin) / extent[1]],
            extent,
        }
    }

    fn uv(&self, corners: [[f32; 2]; 4], point: Vec3) -> [f32; 2] {
        let local = point - self.origin;
        let [u, v] = std::array::from_fn(|axis| {
            (local.dot(self.axes[axis]) / self.extent[axis]).clamp(0.0, 1.0)
        });
        std::array::from_fn(|axis| {
            corners[0][axis]
                + u * (corners[1][axis] - corners[0][axis])
                + v * (corners[2][axis] - corners[0][axis])
        })
    }
}

#[derive(Clone, Copy)]
struct Source {
    quad: usize,
    voxel: IVec3,
}

struct Grid<'a> {
    voxels: &'a [VisibleVoxel],
    dimensions: [u32; 3],
}

impl Grid<'_> {
    fn clamp(&self, voxel: IVec3) -> IVec3 {
        voxel.clamp(
            IVec3::ZERO,
            IVec3::from_array(self.dimensions.map(|n| n as i32 - 1)),
        )
    }

    fn voxel(&self, voxel: IVec3) -> VisibleVoxel {
        self.voxels[linearize(
            self.clamp(voxel).to_array().map(|n| n as u32),
            self.dimensions,
        )]
    }

    fn occupied(&self, sample: IVec3) -> bool {
        self.voxel(sample.div_euclid(IVec3::splat(2))).visibility != VoxelVisibility::Empty
    }

    fn sample_position(&self, sample: IVec3) -> Vec3 {
        Vec3::from_array(std::array::from_fn(|axis| {
            let n = sample[axis];
            let offset = if n.rem_euclid(2) == 0 {
                INSET
            } else {
                1.0 - INSET
            };
            (n.div_euclid(2) as f32 + offset).clamp(0.0, self.dimensions[axis] as f32)
        }))
    }
}

pub(crate) fn append_beveled(
    mesh: &mut BakedMesh,
    quads: &[BakedQuad],
    mappings: &[QuadMapping],
    voxels: &[VisibleVoxel],
    dimensions: [u32; 3],
    voxel_size: f32,
    offset: [f32; 3],
) {
    let grid = Grid { voxels, dimensions };
    let faces: Vec<_> = quads.iter().map(Face::new).collect();
    let mut output = Output {
        mesh,
        faces: &faces,
        mappings,
        voxel_size,
        offset: Vec3::from_array(offset),
    };
    let mut sources = HashMap::new();
    let mut turns = BTreeSet::new();

    for (index, (quad, face)) in quads.iter().zip(&faces).enumerate() {
        let origin = IVec3::from_array(quad.minimum.map(|n| n as i32));
        let [u, v] = face.axes.map(Vec3::as_ivec3);
        let normal = face.normal.as_ivec3();
        // Split each face into a central tile and two border strips per axis.
        // The strips belong to the same contour cell on either side of a voxel
        // boundary, but are merged only within their original atlas rectangle.
        let width = quad.width as usize * 3;
        let height = quad.height as usize * 3;
        let mut flat = vec![false; width * height];
        let plane = face.origin.as_ivec3();
        let axial_cell = (2 * plane - IVec3::ONE) * normal.abs();
        let inside = normal.min(IVec3::ZERO).abs();
        let corners = [IVec3::ZERO, u, u + v, v];
        for y in 0..height {
            for x in 0..width {
                let owner = origin + (x / 3) as i32 * u + (y / 3) as i32 * v;
                let cell = axial_cell
                    + (2 * origin.dot(u) + tile_cell(x)) * u
                    + (2 * origin.dot(v) + tile_cell(y)) * v;
                let backing = grid.voxel(owner + normal).visibility != VoxelVisibility::Empty;
                let mask = corners.map(|corner| {
                    grid.occupied(cell + corner + inside)
                        && grid.occupied(cell + corner + inside + normal) == backing
                });
                flat[y * width + x] = mask.iter().all(|&occupied| occupied);
                if !flat[y * width + x] {
                    if backing {
                        append_backing_tile(&mut output, index, &grid, cell, mask, [x, y]);
                    } else {
                        turns.insert(cell.to_array());
                        sources.insert((owner, normal), index);
                    }
                }
            }
        }
        append_flat_rectangles(&mut output, index, &mut flat, width, height);
    }

    for cell in turns {
        // At a half-voxel inset the two samples within each voxel
        // coincide. Only cells spanning a boundary on all axes have volume.
        if cell.iter().any(|axis| axis.rem_euclid(2) == 0) {
            continue;
        }
        append_turn(&mut output, &grid, &sources, IVec3::from_array(cell));
    }
}

fn tile_cell(tile: usize) -> i32 {
    (tile / 3) as i32 * 2 + (tile % 3) as i32 - 1
}

fn tile_position(tile: usize) -> f32 {
    (tile / 3) as f32 + [0.0, INSET, 1.0 - INSET][tile % 3]
}

// Keep the opaque backing at a transmission interface inside the chamfer.
// Clip the marching-squares footprint to the original owner's atlas tile.
fn append_backing_tile(
    output: &mut Output<'_>,
    index: usize,
    grid: &Grid<'_>,
    cell: IVec3,
    mask: [bool; 4],
    [x, y]: [usize; 2],
) {
    let face = &output.faces[index];
    let [u, v] = face.axes;
    let reverse = u.cross(v).dot(face.normal) < 0.0;
    let min = grid.sample_position(cell) - face.origin;
    let max = grid.sample_position(cell + IVec3::ONE) - face.origin;
    let corners = [
        Vec2::new(min.dot(u), min.dot(v)),
        Vec2::new(max.dot(u), min.dot(v)),
        Vec2::new(max.dot(u), max.dot(v)),
        Vec2::new(min.dot(u), max.dot(v)),
    ];
    let middle = std::array::from_fn::<_, 4, _>(|i| corners[i].midpoint(corners[(i + 1) % 4]));
    for start in (0..4).filter(|&i| mask[i] && !mask[(i + 3) % 4]) {
        let mut polygon = vec![middle[(start + 3) % 4]];
        let mut i = start;
        while mask[i] {
            polygon.push(corners[i]);
            i = (i + 1) % 4;
        }
        polygon.push(middle[(i + 3) % 4]);
        for (axis, tile) in [x, y].into_iter().enumerate() {
            polygon = clip_tile(&polygon, axis, tile_position(tile), 1.0);
            polygon = clip_tile(&polygon, axis, tile_position(tile + 1), -1.0);
        }
        if polygon.len() >= 3 {
            let points: Vec<_> = polygon
                .iter()
                .map(|p| face.origin + p.x * u + p.y * v)
                .collect();
            output.polygon(&points, reverse, index, None);
        }
    }
}

fn clip_tile(points: &[Vec2], axis: usize, boundary: f32, sign: f32) -> Vec<Vec2> {
    let mut clipped = Vec::with_capacity(points.len());
    for (&a, &b) in points.iter().zip(points.iter().cycle().skip(1)) {
        let da = sign * (a[axis] - boundary);
        let db = sign * (b[axis] - boundary);
        if da >= 0.0 {
            clipped.push(a);
        }
        if da * db < 0.0 {
            clipped.push(a.lerp(b, da / (da - db)));
        }
    }
    clipped
}

fn append_flat_rectangles(
    output: &mut Output<'_>,
    index: usize,
    flat: &mut [bool],
    width: usize,
    height: usize,
) {
    let face = &output.faces[index];
    for y in 0..height {
        for x in 0..width {
            if !flat[y * width + x] {
                continue;
            }
            let right = (x..width)
                .find(|&column| !flat[y * width + column])
                .unwrap_or(width);
            let bottom = (y + 1..height)
                .find(|&row| {
                    !flat[row * width + x..row * width + right]
                        .iter()
                        .all(|&tile| tile)
                })
                .unwrap_or(height);
            for row in y..bottom {
                flat[row * width + x..row * width + right].fill(false);
            }
            let [u, v] = face.axes;
            let corners = [(x, y), (right, y), (right, bottom), (x, bottom)]
                .map(|(s, t)| face.origin + tile_position(s) * u + tile_position(t) * v);
            let reverse = u.cross(v).dot(face.normal) < 0.0;
            output.polygon(&corners, reverse, index, None);
        }
    }
}

#[derive(Clone, Copy)]
struct Crossing {
    position: Vec3,
    normal: IVec3,
    source: Source,
}

// Binary cube contours are built from marching squares on the six cell faces.
// At checkerboards, connect the two crossings around each occupied corner.
// Every adjacent cell makes the same choice on its shared face.
fn append_turn(
    output: &mut Output<'_>,
    grid: &Grid<'_>,
    sources: &HashMap<(IVec3, IVec3), usize>,
    cell: IVec3,
) {
    let corners: [IVec3; 8] = std::array::from_fn(|i| {
        cell + IVec3::new((i & 1) as i32, ((i >> 1) & 1) as i32, ((i >> 2) & 1) as i32)
    });
    let occupied = corners.map(|sample| grid.occupied(sample));
    let mut crossings = [None; 12];
    for (axis, direction) in AXES.into_iter().enumerate() {
        let bit = 1 << axis;
        for start in (0..8).filter(|i| i & bit == 0) {
            let end = start | bit;
            if occupied[start] == occupied[end] {
                continue;
            }
            let (inside, normal) = if occupied[start] {
                (start, direction)
            } else {
                (end, -direction)
            };
            let voxel = grid.clamp(corners[inside].div_euclid(IVec3::splat(2)));
            crossings[edge_index(start, end)] = Some(Crossing {
                position: grid
                    .sample_position(corners[start])
                    .midpoint(grid.sample_position(corners[end])),
                normal,
                source: Source {
                    quad: sources[&(voxel, normal)],
                    voxel,
                },
            });
        }
    }

    // Direct each segment with the occupied side on the inside of the loop.
    // This keeps winding consistent even for saddle-shaped contours.
    let mut next = [usize::MAX; 12];
    for (axis, direction) in AXES.into_iter().enumerate() {
        let u = 1 << ((axis + 1) % 3);
        let v = 1 << ((axis + 2) % 3);
        for (base, face_normal) in [(0, -direction), (1 << axis, direction)] {
            let square = [base, base | u, base | u | v, base | v];
            let edges: [usize; 4] =
                std::array::from_fn(|i| edge_index(square[i], square[(i + 1) % 4]));
            let mut active = [usize::MAX; 4];
            let mut count = 0;
            for edge in edges {
                if crossings[edge].is_some() {
                    active[count] = edge;
                    count += 1;
                }
            }
            let mut connect = |a: usize, b: usize| {
                let start = crossings[a].unwrap();
                let end = crossings[b].unwrap();
                let tangent = start.normal.cross(face_normal).as_vec3();
                if (end.position - start.position).dot(tangent) > 0.0 {
                    next[a] = b;
                } else {
                    next[b] = a;
                }
            };
            match count {
                2 => connect(active[0], active[1]),
                4 => {
                    for i in (0..4).filter(|&i| occupied[square[i]]) {
                        connect(edges[(i + 3) % 4], edges[i]);
                    }
                }
                _ => {}
            }
        }
    }

    for start in 0..12 {
        let Some(first) = crossings[start].take() else {
            continue;
        };
        let mut contour = vec![first];
        let mut edge = next[start];
        while edge != start {
            contour.push(crossings[edge].take().expect("closed cube contour"));
            edge = next[edge];
        }
        output.contour(grid, &contour);
    }
}

fn edge_index(a: usize, b: usize) -> usize {
    let axis = (a ^ b).trailing_zeros();
    let low = a & b;
    let lower_bits = (1 << axis) - 1;
    axis as usize * 4 + (low & lower_bits) + ((low >> 1) & !lower_bits)
}

struct Output<'a> {
    mesh: &'a mut BakedMesh,
    faces: &'a [Face],
    mappings: &'a [QuadMapping],
    voxel_size: f32,
    offset: Vec3,
}

impl Output<'_> {
    fn contour(&mut self, grid: &Grid<'_>, contour: &[Crossing]) {
        let first = contour[0].source;
        let center = contour.iter().map(|edge| edge.position).sum::<Vec3>() / contour.len() as f32;
        let points: Vec<_> = contour.iter().map(|edge| edge.position).collect();
        let first_material = grid.voxel(first.voxel).index;
        if contour
            .iter()
            .all(|edge| grid.voxel(edge.source.voxel).index == first_material)
        {
            // Edge strips and ordinary corner cuts are planar. Use the cell
            // center as a fan only for larger, potentially nonplanar contours.
            if points.len() <= 4 {
                self.polygon(&points, false, first.quad, Some(first.voxel));
            } else {
                for i in 0..points.len() {
                    self.polygon(
                        &[center, points[i], points[(i + 1) % points.len()]],
                        false,
                        first.quad,
                        Some(first.voxel),
                    );
                }
            }
        } else {
            // Give each crossing a sector with a single source texel so a
            // bevel between two materials cannot interpolate palette indices.
            for (i, edge) in contour.iter().enumerate() {
                let before = points[(i + points.len() - 1) % points.len()];
                let after = points[(i + 1) % points.len()];
                self.polygon(
                    &[
                        center,
                        before.midpoint(edge.position),
                        edge.position,
                        edge.position.midpoint(after),
                    ],
                    false,
                    edge.source.quad,
                    Some(edge.source.voxel),
                );
            }
        }
    }

    fn polygon(&mut self, points: &[Vec3], reverse: bool, quad: usize, voxel: Option<IVec3>) {
        let area = polygon_area(points);
        let normal = if reverse { -area } else { area }.normalize_or_zero();
        if normal == Vec3::ZERO {
            return;
        }
        let face = &self.faces[quad];
        let mapping = &self.mappings[quad];
        let start = self.mesh.positions.len() as u32;
        for &point in points {
            self.mesh
                .positions
                .push((point * self.voxel_size - self.offset).to_array());
            self.mesh.normals.push(normal.to_array());
            let material_point = voxel.map_or(point, |voxel| voxel.as_vec3() + Vec3::splat(0.5));
            self.mesh
                .uv0
                .push(face.uv(mapping.material, material_point));
            if let (Some(uvs), Some(ao)) = (&mut self.mesh.uv1, mapping.ao) {
                uvs.push(face.uv(ao, point));
            }
        }
        for (i, edge) in points[1..].windows(2).enumerate() {
            if (edge[0] - points[0]).cross(edge[1] - points[0]) == Vec3::ZERO {
                continue;
            }
            let i = i as u32 + 1;
            let (a, b) = if reverse { (i + 1, i) } else { (i, i + 1) };
            self.mesh.indices.extend([start, start + a, start + b]);
        }
    }
}

fn polygon_area(points: &[Vec3]) -> Vec3 {
    points[1..]
        .windows(2)
        .map(|edge| (edge[0] - points[0]).cross(edge[1] - points[0]))
        .sum()
}
