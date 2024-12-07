use bevy::{
    asset::RenderAssetUsages,
    image::{Image, ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor},
    math::UVec3,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use ndshape::Shape;

use super::VoxelData;

pub(crate) fn create_cloud_image(cloud_voxels: &Vec<f32>, data: &VoxelData) -> Image {
    let model_size: UVec3 = data.shape.as_array().map(|v| v - 2).into();
    let image_size = Extent3d {
        width: model_size.x,
        height: model_size.y,
        depth_or_array_layers: model_size.z,
    };
    let data = cloud_voxels.iter().flat_map(|d| d.to_le_bytes()).collect();
    let mut image = Image::new(
        image_size,
        TextureDimension::D3,
        data,
        TextureFormat::R32Float,
        RenderAssetUsages::default(),
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::MirrorRepeat,
        address_mode_v: ImageAddressMode::MirrorRepeat,
        address_mode_w: ImageAddressMode::MirrorRepeat,
        mag_filter: ImageFilterMode::Nearest,
        min_filter: ImageFilterMode::Nearest,
        mipmap_filter: ImageFilterMode::Nearest,
        ..Default::default()
    });
    image
}
