use bevy::{
    app::{App, Plugin},
    asset::{
        AssetApp,
        io::{AsyncWriteExt, Writer},
        processor::{Process, ProcessContext, ProcessError},
    },
    gltf::{GltfLoader, GltfLoaderSettings},
    reflect::TypePath,
};
use vox_bake::{VoxBakeSettings, export_vox_to_glb};

/// Registers [`VoxToGlbProcessor`] as the default processor for `.vox` assets.
pub struct VoxGltfProcessorPlugin;

impl Plugin for VoxGltfProcessorPlugin {
    fn build(&self, app: &mut App) {
        app.register_asset_processor(VoxToGlbProcessor)
            .set_default_asset_processor::<VoxToGlbProcessor>("vox");
    }
}

/// Converts MagicaVoxel assets into GLB assets during Bevy asset processing.
#[derive(TypePath)]
pub struct VoxToGlbProcessor;

impl Process for VoxToGlbProcessor {
    type Settings = VoxBakeSettings;
    type OutputLoader = GltfLoader;

    async fn process(
        &self,
        context: &mut ProcessContext<'_>,
        settings: &Self::Settings,
        writer: &mut Writer,
    ) -> Result<GltfLoaderSettings, ProcessError> {
        let mut source = Vec::new();
        context
            .asset_reader()
            .read_to_end(&mut source)
            .await
            .map_err(|error| ProcessError::AssetTransformError(Box::new(error)))?;

        let exported = export_vox_to_glb(&source, settings)
            .map_err(|error| ProcessError::AssetTransformError(Box::new(error)))?;

        writer
            .write_all(&exported.bytes)
            .await
            .map_err(|error| ProcessError::AssetSaveError(error.into()))?;

        Ok(GltfLoaderSettings::default())
    }
}
