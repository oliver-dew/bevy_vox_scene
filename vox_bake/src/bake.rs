use std::{error::Error, fmt};

use crate::{
    BakedImage, BakedMesh, BakedMeshAtlas, BakedPalette, BakedPixelFormat, BakedQuad, BakedVolume,
    TextureMipPolicy, VoxBakeSettings, VoxelGrid, VoxelTextureMode, ambient_occlusion_quad_uvs,
    ambient_occlusion_stencil, bake_quad_geometry, classify_voxels, pack_atlas_rectangles,
    packed_quad_uvs, paint_ambient_occlusion_island, paint_palette_island, palette_quad_uvs,
    sanitized_ambient_occlusion_intensity,
};

/// Fully baked surface, volume, and optical data for one voxel model.
#[derive(Clone, Debug, PartialEq)]
pub struct BakedModel {
    /// Surface mesh and material atlases, when the model has surface voxels.
    pub surface: Option<BakedMeshAtlas>,
    /// Cloud-density volume, when the model contains media voxels.
    pub volume: Option<BakedVolume>,
    /// Mean index of refraction across translucent surface voxels.
    pub average_ior: Option<f32>,
    /// Unpadded model dimensions in right-handed Y-up coordinates.
    pub dimensions: [u32; 3],
}

/// Failure encountered while baking a voxel model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BakeError {
    /// Material texture islands could not fit within the configured maximum dimension.
    MaterialAtlasTooLarge,
    /// Ambient-occlusion texture islands could not fit within the configured maximum dimension.
    AmbientOcclusionAtlasTooLarge,
}

impl fmt::Display for BakeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MaterialAtlasTooLarge => {
                formatter.write_str("voxel mesh atlas exceeds the maximum texture size")
            }
            Self::AmbientOcclusionAtlasTooLarge => formatter
                .write_str("voxel ambient-occlusion atlas exceeds the maximum texture size"),
        }
    }
}

impl Error for BakeError {}

/// Bake one engine-independent voxel model into surface and volume representations.
pub fn bake_model(
    grid: &VoxelGrid,
    palette: &BakedPalette,
    settings: &VoxBakeSettings,
) -> Result<BakedModel, BakeError> {
    let classified = classify_voxels(
        &grid.voxels,
        settings.texture_mode,
        &palette.indices_of_refraction,
        &palette.density_for_voxel,
    );
    let volume = if classified.has_cloud {
        extract_volume(grid, &palette.density_for_voxel)
    } else {
        None
    };
    let surface = classified
        .has_surface
        .then(|| bake_surface(grid, &classified.voxels, settings))
        .transpose()?;
    Ok(BakedModel {
        surface,
        volume,
        average_ior: classified.average_ior,
        dimensions: grid.model_dimensions,
    })
}

fn bake_surface(
    grid: &VoxelGrid,
    voxels: &[crate::VisibleVoxel],
    settings: &VoxBakeSettings,
) -> Result<BakedMeshAtlas, BakeError> {
    let quads = crate::greedy_mesh_quads(voxels, grid.dimensions);
    let requested_ao = settings.ambient_occlusion.and_then(|ao| {
        let intensity = sanitized_ambient_occlusion_intensity(ao.intensity);
        (ao.depth > 0 && intensity > 0.0).then_some((ao.depth.min(8), intensity))
    });
    let pack_settings = match settings.texture_mode {
        VoxelTextureMode::Packed(mut pack) => {
            if pack.mip_policy.max_lod() > 0 {
                let mip_alignment = pack.mip_policy.alignment();
                pack.padding = pack.padding.max(mip_alignment);
                pack.alignment = common_alignment(pack.alignment, mip_alignment);
            }
            if requested_ao.is_some() {
                pack.padding = pack.padding.max(1);
            }
            Some(pack)
        }
        VoxelTextureMode::Palette => None,
    };
    let ao_settings = pack_settings.and(requested_ao);
    let material_layout = match pack_settings {
        Some(pack) => pack_atlas_rectangles(
            &quads
                .iter()
                .map(|quad| (quad.width, quad.height))
                .collect::<Vec<_>>(),
            pack.padding,
            pack.alignment,
            pack.max_dimension,
            pack.mip_policy.alignment(),
        )
        .ok_or(BakeError::MaterialAtlasTooLarge)?,
        None => crate::PackedAtlas {
            width: 16,
            height: 16,
            rectangles: vec![crate::AtlasRect { x: 0, y: 0 }; quads.len()],
        },
    };
    let ao_layout = if let Some((_, _)) = ao_settings {
        let pack = pack_settings.expect("AO is available only for packed textures");
        Some(
            pack_atlas_rectangles(
                &quads
                    .iter()
                    .map(|quad| (quad.width + 1, quad.height + 1))
                    .collect::<Vec<_>>(),
                pack.padding,
                pack.alignment,
                pack.max_dimension,
                pack.mip_policy.alignment(),
            )
            .ok_or(BakeError::AmbientOcclusionAtlasTooLarge)?,
        )
    } else {
        None
    };

    let vertex_count = quads.len() * 4;
    let mut mesh = BakedMesh {
        positions: Vec::with_capacity(vertex_count),
        normals: Vec::with_capacity(vertex_count),
        uv0: Vec::with_capacity(vertex_count),
        uv1: ao_settings.map(|_| Vec::with_capacity(vertex_count)),
        indices: Vec::with_capacity(quads.len() * 6),
    };
    let mut palette_indices = if pack_settings.is_some() {
        vec![0; (material_layout.width * material_layout.height) as usize]
    } else {
        (0..=u8::MAX).collect()
    };
    let mut used = [false; 256];
    let mut ao_image = ao_layout.as_ref().map(|layout| BakedImage {
        width: layout.width,
        height: layout.height,
        format: BakedPixelFormat::R8Linear,
        pixels: vec![u8::MAX; (layout.width * layout.height) as usize],
    });
    let position_offset = std::array::from_fn(|axis| {
        grid.model_dimensions[axis] as f32
            * settings.voxel_size
            * settings.mesh_offset.to_array()[axis]
            + (grid.padding / 2) as f32 * settings.voxel_size
    });
    let ao_stencils = ao_settings.map(|(depth, _)| {
        (0..6)
            .map(|face| ambient_occlusion_stencil(face, depth))
            .collect::<Vec<_>>()
    });

    for (index, quad) in quads.iter().enumerate() {
        append_geometry(&mut mesh, quad, settings.voxel_size, position_offset);
        let rect = material_layout.rectangles[index];
        if let Some(pack) = pack_settings {
            mesh.uv0.extend_from_slice(&packed_quad_uvs(
                rect,
                pack.padding,
                [quad.width, quad.height],
                [material_layout.width, material_layout.height],
            ));
            paint_palette_island(
                &mut palette_indices,
                material_layout.width,
                rect,
                quad,
                voxels,
                grid.dimensions,
                pack.padding,
                &mut used,
            );
            if let (Some(layout), Some(image), Some(uvs), Some(stencils), Some((_, intensity))) = (
                ao_layout.as_ref(),
                ao_image.as_mut(),
                mesh.uv1.as_mut(),
                ao_stencils.as_ref(),
                ao_settings,
            ) {
                let ao_rect = layout.rectangles[index];
                uvs.extend_from_slice(&ambient_occlusion_quad_uvs(
                    ao_rect,
                    pack.padding,
                    [quad.width, quad.height],
                    [layout.width, layout.height],
                ));
                paint_ambient_occlusion_island(
                    image,
                    ao_rect,
                    quad,
                    voxels,
                    grid.dimensions,
                    pack.padding,
                    &stencils[quad.face_index],
                    intensity,
                );
            }
        } else {
            let palette_index = voxels[grid.linearize(quad.minimum)].index;
            used[palette_index as usize] = true;
            mesh.uv0.extend_from_slice(&palette_quad_uvs(palette_index));
        }
    }

    Ok(BakedMeshAtlas {
        mesh,
        width: material_layout.width,
        height: material_layout.height,
        palette_indices,
        used_palette_indices: used
            .iter()
            .enumerate()
            .filter_map(|(i, used)| used.then_some(i as u8))
            .collect(),
        ambient_occlusion: ao_image,
        mip_policy: pack_settings.map_or(TextureMipPolicy::Disabled, |pack| pack.mip_policy),
    })
}

fn append_geometry(mesh: &mut BakedMesh, quad: &BakedQuad, voxel_size: f32, offset: [f32; 3]) {
    let geometry = bake_quad_geometry(quad, voxel_size, offset, mesh.positions.len() as u32);
    mesh.positions.extend_from_slice(&geometry.positions);
    mesh.normals.extend_from_slice(&geometry.normals);
    mesh.indices.extend_from_slice(&geometry.indices);
}

fn extract_volume(grid: &VoxelGrid, density: &[Option<f32>]) -> Option<BakedVolume> {
    if grid.padding == 2 {
        return crate::extract_cloud_volume(&grid.voxels, grid.dimensions, density);
    }
    let densities = grid
        .voxels
        .iter()
        .map(|voxel| {
            let value = density[voxel.0 as usize];
            value.unwrap_or(0.0)
        })
        .collect();
    Some(BakedVolume {
        dimensions: grid.model_dimensions,
        densities,
    })
}

fn common_alignment(left: u32, right: u32) -> u32 {
    let left = left.max(1);
    let right = right.max(1);
    let mut a = left;
    let mut b = right;
    while b != 0 {
        (a, b) = (b, a % b);
    }
    left.saturating_div(a).saturating_mul(right)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BakedMaterial, RawVoxel};

    #[test]
    fn complete_bake_produces_surface_and_volume_results() {
        let mut surface_grid = VoxelGrid::empty([1; 3], true);
        let center = surface_grid.linearize([1; 3]);
        surface_grid.voxels[center] = RawVoxel(0);
        let palette = BakedPalette::new(vec![BakedMaterial::default()], true);
        let surface = bake_model(&surface_grid, &palette, &VoxBakeSettings::default()).unwrap();
        let atlas = surface.surface.expect("opaque voxel surface");
        assert_eq!(atlas.mesh.positions.len(), 24);
        assert_eq!(atlas.mesh.indices.len(), 36);
        assert!(surface.volume.is_none());

        let cloud_palette = BakedPalette::new(
            vec![BakedMaterial {
                density: 0.5,
                ..Default::default()
            }],
            true,
        );
        let cloud = bake_model(&surface_grid, &cloud_palette, &VoxBakeSettings::default()).unwrap();
        assert!(cloud.surface.is_none());
        assert_eq!(cloud.volume.unwrap().densities, vec![0.5]);
    }
}
