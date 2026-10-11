pub use self::voxel::Voxel;
#[cfg(feature = "generate_voxels")]
use crate::VoxelModelInstance;
use crate::model::mesh::MeshAtlas;
#[cfg(any(feature = "generate_voxels", feature = "modify_voxels"))]
use crate::{VoxelCloudSource, VoxelModelSource, VoxelPaletteSource};
#[cfg(any(feature = "generate_voxels", feature = "modify_voxels"))]
use bevy::{
    asset::Assets,
    ecs::{
        entity::Entity,
        query::QueryData,
        system::{Query, ResMut, SystemParam},
    },
    mesh::{Mesh, Mesh3d},
    pbr::MeshMaterial3d,
};
#[cfg(feature = "generate_voxels")]
use bevy::{
    asset::Handle, camera::visibility::Visibility, ecs::world::World, light::FogVolume,
    transform::components::Transform, world_serialization::WorldAsset,
};
use bevy::{
    asset::{Asset, RenderAssetUsages},
    ecs::component::Component,
    image::Image,
    math::{IVec3, UVec3},
    pbr::StandardMaterial,
    prelude::ReflectComponent,
    reflect::{Reflect, TypePath},
};
#[cfg(feature = "generate_voxels")]
use flipbook::{FlipbookFrame, FlipbookPlayer};
#[cfg(feature = "generate_voxels")]
use thiserror::Error;
use vox_bake::{BakeError, BakedModel, BakedPalette, VoxBakeSettings, VoxelGrid};
pub(crate) use voxel::RawVoxel;
pub(super) mod mesh;
#[cfg(feature = "modify_voxels")]
pub(super) mod modify;
pub(crate) mod queryable;
#[cfg(feature = "generate_voxels")]
pub(super) mod sdf;
pub use self::queryable::OutOfBoundsError;
mod palette;
pub use palette::{VoxelElement, VoxelPalette};
pub(super) mod cloud;
mod voxel;

/// A voxel grid and the settings used to bake it.
#[derive(Asset, TypePath, Clone, Debug)]
pub struct VoxelModel {
    pub(crate) grid: VoxelGrid,
    pub(crate) bake_settings: VoxBakeSettings,
}

impl Default for VoxelModel {
    fn default() -> Self {
        Self::new(UVec3::ZERO, VoxBakeSettings::default())
    }
}

impl VoxelModel {
    /// Create a new, empty voxel model.
    pub fn new(size: UVec3, settings: VoxBakeSettings) -> Self {
        Self {
            grid: VoxelGrid::empty(size.into(), settings.mesh_outer_faces),
            bake_settings: settings,
        }
    }

    pub(crate) fn from_grid(grid: VoxelGrid, bake_settings: VoxBakeSettings) -> Self {
        Self {
            grid,
            bake_settings,
        }
    }

    pub(crate) fn _size(&self) -> IVec3 {
        let size: UVec3 = self.grid.model_dimensions.into();
        IVec3::try_from(size).unwrap_or(IVec3::ZERO)
    }

    pub(crate) fn padding(&self) -> u32 {
        self.grid.padding
    }

    pub(crate) fn bake(&self, palette: &BakedPalette) -> Result<BakedModel, BakeError> {
        vox_bake::bake_model(&self.grid, palette, &self.bake_settings)
    }

    pub(crate) fn remesh(
        &self,
        palette: &BakedPalette,
    ) -> Result<(Option<MeshAtlas>, Option<f32>, Option<Image>), BakeError> {
        let baked = self.bake(palette)?;
        let image = baked.volume.as_ref().map(cloud::create_cloud_image);
        Ok((baked.surface, baked.average_ior, image))
    }
}

#[cfg_attr(not(feature = "modify_voxels"), allow(dead_code))]
#[derive(Component, Reflect)]
#[reflect(Component)]
pub(crate) struct VoxelMeshAssetUsage(pub RenderAssetUsages);

#[cfg(any(feature = "generate_voxels", feature = "modify_voxels"))]
#[derive(QueryData)]
struct VoxelInstanceAssets {
    model: &'static VoxelModelSource,
    palette: &'static VoxelPaletteSource,
    mesh: Option<&'static Mesh3d>,
    material: Option<&'static MeshMaterial3d<StandardMaterial>>,
    cloud: Option<&'static VoxelCloudSource>,
    mesh_asset_usage: Option<&'static VoxelMeshAssetUsage>,
}

/// An error encountered while creating a generated voxel scene or animation.
#[cfg(feature = "generate_voxels")]
#[derive(Error, Debug)]
pub enum CreateVoxelSceneError {
    /// The supplied palette handle is not available.
    #[error("voxel palette asset is unavailable")]
    PaletteUnavailable,
    /// The animation contained no frames.
    #[error("voxel animation requires at least one frame")]
    EmptyAnimation,
    /// Baking a voxel model failed.
    #[error(transparent)]
    Bake(#[from] BakeError),
}

/// Asset collections used to inspect and modify retained voxel models.
#[cfg(any(feature = "generate_voxels", feature = "modify_voxels"))]
#[derive(SystemParam)]
pub struct VoxelModelAssets<'w, 's> {
    meshes: ResMut<'w, Assets<Mesh>>,
    materials: ResMut<'w, Assets<StandardMaterial>>,
    images: ResMut<'w, Assets<Image>>,
    models: ResMut<'w, Assets<VoxelModel>>,
    palettes: ResMut<'w, Assets<VoxelPalette>>,
    #[cfg_attr(not(feature = "modify_voxels"), allow(dead_code))]
    instances: Query<'w, 's, VoxelInstanceAssets>,
}

#[cfg(any(feature = "generate_voxels", feature = "modify_voxels"))]
impl VoxelModelAssets<'_, '_> {
    /// Get the retained voxel model attached to an entity.
    pub fn get_model(&self, entity: Entity) -> Option<&VoxelModel> {
        let instance = self.instances.get(entity).ok()?;
        self.models.get(&instance.model.0)
    }
}

/// Asset collections used to create runtime voxel scenes and animations.
#[cfg(feature = "generate_voxels")]
#[derive(SystemParam)]
pub struct VoxelSceneAssets<'w, 's> {
    model_assets: VoxelModelAssets<'w, 's>,
    scenes: ResMut<'w, Assets<WorldAsset>>,
}

#[cfg(feature = "generate_voxels")]
impl<'w, 's> VoxelSceneAssets<'w, 's> {
    /// Access the underlying voxel model assets.
    pub fn model_assets(&mut self) -> &mut VoxelModelAssets<'w, 's> {
        &mut self.model_assets
    }

    /// Add a generated voxel palette to the asset collection.
    pub fn add_palette(&mut self, palette: VoxelPalette) -> Handle<VoxelPalette> {
        self.model_assets.palettes.add(palette)
    }

    /// Create a voxel scene from supplied voxel data.
    pub fn create_scene(
        &mut self,
        model: VoxelModel,
        palette_handle: Handle<VoxelPalette>,
        retain_voxel_data: bool,
        mesh_asset_usage: RenderAssetUsages,
    ) -> Result<Handle<WorldAsset>, CreateVoxelSceneError> {
        let palette = self
            .model_assets
            .palettes
            .get(&palette_handle)
            .ok_or(CreateVoxelSceneError::PaletteUnavailable)?;
        let (maybe_atlas, average_ior, maybe_cloud) = model.remesh(&palette.baked)?;
        let (maybe_mesh_handle, maybe_material_handle) =
            maybe_atlas.map_or((None, None), |atlas| {
                let mut material = palette
                    .create_atlas_material(&atlas, |_, image| self.model_assets.images.add(image));
                let mesh_handle = self
                    .model_assets
                    .meshes
                    .add(mesh::to_bevy_mesh(atlas.mesh, mesh_asset_usage));
                configure_material(&mut material, average_ior, &model);
                (
                    Some(mesh_handle),
                    Some(self.model_assets.materials.add(material)),
                )
            });
        let cloud_image = maybe_cloud.map(|image| self.model_assets.images.add(image));
        let model_size = model._size().as_vec3() * model.bake_settings.voxel_size;
        let model_source =
            retain_voxel_data.then(|| VoxelModelSource(self.model_assets.models.add(model)));

        let mut world = World::new();
        let mut root = world.spawn((
            Transform::IDENTITY,
            Visibility::Visible,
            VoxelModelInstance,
            VoxelPaletteSource(palette_handle),
        ));
        if let Some(model_source) = model_source {
            root.insert(model_source);
        }
        if let Some(mesh_handle) = maybe_mesh_handle {
            root.insert((Mesh3d(mesh_handle), VoxelMeshAssetUsage(mesh_asset_usage)));
            if let Some(material_handle) = maybe_material_handle {
                root.insert(MeshMaterial3d(material_handle));
            }
        }
        if let Some(cloud_image) = cloud_image {
            root.insert(crate::VoxelCloudSource(cloud_image.clone()));
            root.with_child((
                FogVolume {
                    density_texture: Some(cloud_image),
                    absorption: 0.1,
                    ..Default::default()
                },
                Transform::from_scale(model_size),
            ));
        }
        let scene = WorldAsset::new(world);
        Ok(self.scenes.add(scene))
    }

    /// Create a voxel animation from supplied voxel frames.
    pub fn create_animation<I>(
        &mut self,
        frames: I,
        palette_handle: Handle<VoxelPalette>,
        retain_voxel_data: bool,
        mesh_asset_usage: RenderAssetUsages,
    ) -> Result<Handle<WorldAsset>, CreateVoxelSceneError>
    where
        I: IntoIterator<Item = VoxelModel>,
    {
        let frames: Vec<_> = frames.into_iter().collect();
        if frames.is_empty() {
            return Err(CreateVoxelSceneError::EmptyAnimation);
        }
        let palette = self
            .model_assets
            .palettes
            .get(&palette_handle)
            .ok_or(CreateVoxelSceneError::PaletteUnavailable)?;
        let frame_count = frames.len();
        let prepared = frames
            .into_iter()
            .enumerate()
            .map(|(index, model)| {
                let (maybe_atlas, average_ior, maybe_cloud) = model.remesh(&palette.baked)?;
                let (maybe_mesh_handle, maybe_material_handle) =
                    maybe_atlas.map_or((None, None), |atlas| {
                        let mut material = palette.create_atlas_material(&atlas, |_, image| {
                            self.model_assets.images.add(image)
                        });
                        let mesh_handle = self
                            .model_assets
                            .meshes
                            .add(mesh::to_bevy_mesh(atlas.mesh, mesh_asset_usage));
                        configure_material(&mut material, average_ior, &model);
                        (
                            Some(mesh_handle),
                            Some(self.model_assets.materials.add(material)),
                        )
                    });
                let cloud_image = maybe_cloud.map(|image| self.model_assets.images.add(image));
                let model_size = model._size().as_vec3() * model.bake_settings.voxel_size;
                let model_source = retain_voxel_data
                    .then(|| VoxelModelSource(self.model_assets.models.add(model)));
                Ok::<_, CreateVoxelSceneError>((
                    index,
                    model_source,
                    maybe_mesh_handle,
                    maybe_material_handle,
                    cloud_image,
                    model_size,
                ))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut world = World::new();
        let mut root = world.spawn((Transform::IDENTITY, Visibility::Visible));
        root.with_children(|spawner| {
            for (
                index,
                model_source,
                maybe_mesh_handle,
                maybe_material_handle,
                cloud_image,
                model_size,
            ) in prepared
            {
                let mut frame = spawner.spawn((
                    VoxelModelInstance,
                    VoxelPaletteSource(palette_handle.clone()),
                    FlipbookFrame(index),
                    Transform::IDENTITY,
                    if index == 0 {
                        Visibility::Inherited
                    } else {
                        Visibility::Hidden
                    },
                ));
                if let Some(model_source) = model_source {
                    frame.insert(model_source);
                }
                if let Some(mesh_handle) = maybe_mesh_handle {
                    frame.insert((Mesh3d(mesh_handle), VoxelMeshAssetUsage(mesh_asset_usage)));
                    if let Some(material_handle) = maybe_material_handle {
                        frame.insert(MeshMaterial3d(material_handle));
                    }
                }
                if let Some(cloud_image) = cloud_image {
                    frame.insert(crate::VoxelCloudSource(cloud_image.clone()));
                    frame.with_child((
                        FogVolume {
                            density_texture: Some(cloud_image),
                            absorption: 0.1,
                            ..Default::default()
                        },
                        Transform::from_scale(model_size),
                    ));
                }
            }
        });
        root.insert(FlipbookPlayer::from_frame_count(frame_count));
        let scene = WorldAsset::new(world);
        Ok(self.scenes.add(scene))
    }
}

pub(crate) fn configure_material(
    material: &mut StandardMaterial,
    average_ior: Option<f32>,
    model: &VoxelModel,
) {
    if let Some(ior) = average_ior {
        material.ior = ior;
        material.thickness = model._size().min_element() as f32 * model.bake_settings.voxel_size;
    } else {
        #[cfg(feature = "pbr_transmission_textures")]
        {
            material.specular_transmission_texture = None;
        }
        material.specular_transmission = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use bevy::{
        app::App,
        asset::Assets,
        camera::primitives::MeshAabb,
        ecs::hierarchy::Children,
        ecs::system::SystemState,
        math::{UVec3, Vec3, Vec3A},
        mesh::Mesh,
        prelude::Mesh3d,
        world_serialization::WorldAssetRoot,
    };

    use crate::{
        CreateVoxelSceneError, SDF, VoxBakeSettings, Voxel, VoxelModel, VoxelModelInstance,
        VoxelModelSource, VoxelPalette, VoxelPaletteSource, VoxelSceneAssets,
        test_support::setup_app,
    };

    #[cfg(feature = "generate_voxels")]
    #[test]
    fn test_generate_voxels() {
        let mut app = App::new();
        setup_app(&mut app);
        let palette =
            VoxelPalette::from_colors(vec![bevy::color::palettes::css::GREEN.into()], true);
        let tall_box = SDF::cuboid(Vec3::new(0.5, 2.5, 0.5)).voxelize(
            UVec3::splat(6),
            VoxBakeSettings::default(),
            Voxel(1),
        );
        let world = app.world_mut();
        let mut state = SystemState::<VoxelSceneAssets>::new(world);
        let scene_handle = {
            let mut voxel_assets = state.get_mut(world).expect("voxel asset resources exist");
            let palette_handle = voxel_assets.add_palette(palette);
            assert!(matches!(
                voxel_assets.create_animation(
                    std::iter::empty(),
                    palette_handle.clone(),
                    false,
                    bevy::asset::RenderAssetUsages::RENDER_WORLD,
                ),
                Err(CreateVoxelSceneError::EmptyAnimation)
            ));
            voxel_assets
                .create_scene(
                    tall_box,
                    palette_handle,
                    false,
                    bevy::asset::RenderAssetUsages::RENDER_WORLD,
                )
                .expect("voxel scene created")
        };
        state.apply(world);
        let scene_root = world.spawn(WorldAssetRoot(scene_handle)).id();
        app.update();
        let entity = app
            .world()
            .get::<Children>(scene_root)
            .expect("children")
            .first()
            .expect("model entity");

        assert!(app.world().get::<VoxelModelInstance>(*entity).is_some());
        assert!(app.world().get::<VoxelPaletteSource>(*entity).is_some());
        assert!(
            app.world().get::<VoxelModelSource>(*entity).is_none(),
            "default settings discard the source voxel grid"
        );
        assert_eq!(app.world().resource::<Assets<VoxelModel>>().len(), 0);

        let mesh_handle = &app.world().get::<Mesh3d>(*entity).expect("voxel mesh").0;
        let mesh = app
            .world()
            .resource::<Assets<Mesh>>()
            .get(mesh_handle)
            .expect("mesh generated");
        assert_eq!(
            mesh.get_aabb().expect("aabb").half_extents,
            Vec3A::new(0.5, 2.5, 0.5)
        );
        assert_eq!(
            mesh.count_vertices(),
            6 * 4,
            "resulting mesh should have 6 quads"
        );
    }
}
