use bevy::utils::HashMap;
use block_mesh::{VoxelVisibility, MergeVoxel, Voxel as BlockyVoxel};
use dot_vox::Model;
use ndshape::RuntimeShape;
use ndshape::Shape;

#[derive(Clone, PartialEq)]
pub struct Voxel(u8);

impl Voxel {
    pub const EMPTY: Voxel = Voxel(255);
}

// trait implementation rules requires the use of a newtype to allow meshing.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct VisibleVoxel {
    pub index: u8,
    visibility: VoxelVisibility,
}

impl BlockyVoxel for VisibleVoxel {
    fn get_visibility(&self) -> VoxelVisibility {
        self.visibility
    }
}

impl MergeVoxel for VisibleVoxel {
    type MergeValue = VisibleVoxel;

    fn merge_value(&self) -> Self::MergeValue {
        *self
    }
}

pub struct VoxelData {
    pub shape: RuntimeShape<u32, 3>,
    pub voxels: Vec<Voxel>,
    pub ior_for_voxel: HashMap<u8, f32>,
}

impl VoxelData {
    /// Returns the [`VoxelVisibility`] of each Voxel, and, if the model contains
    /// translucent voxels, the average Index of Refraction.
    pub(crate) fn visible_voxels(&self) -> (Vec<VisibleVoxel>, Option<f32>) {
        let mut refraction_indices: Vec<f32> = Vec::new();
        let voxels: Vec<VisibleVoxel> = self.voxels.iter().map(|v| {
            VisibleVoxel {
                index: v.0,
                visibility: if *v == Voxel::EMPTY { 
                    VoxelVisibility::Empty
                } else if let Some(ior) = self.ior_for_voxel.get(&v.0) {
                    refraction_indices.push(*ior);
                    VoxelVisibility::Translucent
                } else {
                    VoxelVisibility::Opaque
                }
            }
        }).collect();
        let average_ior: Option<f32> = if refraction_indices.is_empty() { 
            None 
        } else { 
            let ior = 1.0
                        + (refraction_indices
                            .iter()
                            .cloned()
                            .reduce(|acc, e| acc + e)
                            .unwrap_or(0.0)
                            / refraction_indices.len() as f32);
            Some(ior)
        };
        (voxels, average_ior)
    }
}

pub(crate) fn load_from_model(
    model: &Model,
    ior_for_voxel: &HashMap<u8, f32>,
) -> VoxelData {
    let shape = RuntimeShape::<u32, 3>::new([model.size.x + 2, model.size.z + 2, model.size.y + 2]);
    let mut voxels = vec![Voxel::EMPTY; shape.size() as usize];

    model.voxels.iter().for_each(|voxel| {
        let index = shape.linearize([
            model.size.x - voxel.x as u32,
            voxel.z as u32 + 1,
            voxel.y as u32 + 1,
        ]) as usize;
        voxels[index] = Voxel(voxel.i);
    });

    VoxelData { 
        shape, 
        voxels, 
        ior_for_voxel: ior_for_voxel.clone() 
    }
}
