use bevy::{
    asset::RenderAssetUsages,
    image::{Image, ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor},
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use vox_bake::BakedVolume;

pub(crate) fn create_cloud_image(volume: &BakedVolume) -> Image {
    let image_size = Extent3d {
        width: volume.dimensions[0],
        height: volume.dimensions[1],
        depth_or_array_layers: volume.dimensions[2],
    };
    let data = volume
        .densities
        .iter()
        .flat_map(|density| density.to_le_bytes())
        .collect();
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
