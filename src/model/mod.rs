use crate::{VoxelModelInstance, VoxelModelSource, VoxelModelSourcePolicy};
use bevy::{
    asset::{Asset, Assets, Handle},
    camera::visibility::Visibility,
    ecs::{system::ResMut, world::World},
    image::Image,
    light::FogVolume,
    mesh::Mesh3d,
    pbr::{MeshMaterial3d, StandardMaterial},
    reflect::TypePath,
    transform::components::Transform,
    world_serialization::WorldAsset,
};
#[cfg(feature = "generate_voxels")]
use bevy::{ecs::system::SystemParam, mesh::Mesh};

pub use self::{data::VoxelData, voxel::Voxel};
use flipbook::{FlipbookFrame, FlipbookPlayer};
pub(crate) use voxel::RawVoxel;
pub(super) mod data;
pub(super) mod mesh;
#[cfg(feature = "modify_voxels")]
pub(super) mod modify;
pub(crate) mod queryable;
#[cfg(feature = "generate_voxels")]
pub(super) mod sdf;
#[cfg(feature = "modify_voxels")]
pub use self::queryable::VoxelQueryable;
mod palette;
pub use palette::{VoxelElement, VoxelPalette};
pub(super) mod cloud;
mod voxel;

/// Contains the voxel data for a model
#[derive(Asset, TypePath, Default, Clone, Debug)]
pub struct VoxelModel {
    /// Unique name of the model
    pub name: String,
    /// The voxel data used to generate the mesh
    pub(crate) data: VoxelData,
    /// True if the model contains solid or transmissive voxels
    pub has_mesh: bool,
    /// True if the model contains cloud voxels
    pub has_cloud: bool,
    pub(crate) material: Option<Handle<StandardMaterial>>,
}

/// Asset collections used to create runtime voxel scenes and animations.
#[cfg(feature = "generate_voxels")]
#[derive(SystemParam)]
pub struct VoxelSceneAssets<'w> {
    meshes: ResMut<'w, Assets<Mesh>>,
    materials: ResMut<'w, Assets<StandardMaterial>>,
    images: ResMut<'w, Assets<Image>>,
    models: ResMut<'w, Assets<VoxelModel>>,
    scenes: ResMut<'w, Assets<WorldAsset>>,
    palettes: ResMut<'w, Assets<VoxelPalette>>,
}

#[cfg(feature = "generate_voxels")]
impl VoxelSceneAssets<'_> {
    /// Add a generated voxel palette to the asset collection.
    pub fn add_palette(&mut self, palette: VoxelPalette) -> Handle<VoxelPalette> {
        self.palettes.add(palette)
    }

    /// Create a voxel scene from supplied voxel data.
    pub fn create_scene(
        &mut self,
        data: VoxelData,
        name: impl Into<String>,
        palette_handle: Handle<VoxelPalette>,
    ) -> Handle<WorldAsset> {
        let name = name.into();
        let palette = self
            .palettes
            .get(&palette_handle)
            .expect("Voxel palette exists");
        let (maybe_atlas, average_ior, maybe_cloud) = data.remesh(&palette.baked);
        let (maybe_mesh_handle, maybe_material_handle) =
            maybe_atlas.map_or((None, None), |atlas| {
                let mut material =
                    palette.create_atlas_material(&atlas, |_, image| self.images.add(image));
                let mesh_handle = self.meshes.add(mesh::to_bevy_mesh(
                    atlas.mesh,
                    data.model_source == VoxelModelSourcePolicy::Remeshable,
                ));
                configure_material(&mut material, average_ior, &data);
                (Some(mesh_handle), Some(self.materials.add(material)))
            });
        let cloud_image = maybe_cloud.map(|image| self.images.add(image));
        let model_size = data._size().as_vec3() * data.bake_settings.voxel_size;
        let model_source_policy = data.model_source;
        let model_source = match model_source_policy {
            VoxelModelSourcePolicy::Discard => None,
            model_source_policy => {
                let model = self.models.add(VoxelModel {
                    name: name.clone(),
                    data,
                    has_mesh: maybe_mesh_handle.is_some(),
                    has_cloud: cloud_image.is_some(),
                    material: maybe_material_handle.clone(),
                });
                Some(match model_source_policy {
                    VoxelModelSourcePolicy::Queryable => VoxelModelSource::Queryable {
                        model,
                        palette: palette_handle.clone(),
                    },
                    VoxelModelSourcePolicy::Remeshable => VoxelModelSource::Remeshable {
                        model,
                        palette: palette_handle.clone(),
                    },
                    VoxelModelSourcePolicy::Discard => unreachable!(),
                })
            }
        };

        let mut world = World::new();
        let mut root = world.spawn((Transform::IDENTITY, Visibility::Visible, VoxelModelInstance));
        if let Some(model_source) = model_source {
            root.insert(model_source);
        }
        if let Some(mesh_handle) = maybe_mesh_handle {
            root.insert(Mesh3d(mesh_handle));
            if let Some(material_handle) = maybe_material_handle {
                root.insert(MeshMaterial3d(material_handle));
            }
        }
        if cloud_image.is_some() {
            root.with_child((
                FogVolume {
                    density_texture: cloud_image,
                    absorption: 0.1,
                    ..Default::default()
                },
                Transform::from_scale(model_size),
            ));
        }
        let scene = WorldAsset::new(world);
        self.scenes.add(scene)
    }

    /// Create a voxel animation from supplied voxel frames.
    pub fn create_animation(
        &mut self,
        frames: Vec<VoxelData>,
        name: impl Into<String>,
        palette_handle: Handle<VoxelPalette>,
    ) -> Handle<WorldAsset> {
        let name = name.into();
        let palette = self
            .palettes
            .get(&palette_handle)
            .expect("Voxel palette exists");
        let mut world = World::new();
        let mut root = world.spawn((Transform::IDENTITY, Visibility::Visible));
        let frame_count = frames.len();
        root.with_children(|spawner| {
            for (index, data) in frames.into_iter().enumerate() {
                let (maybe_atlas, average_ior, maybe_cloud) = data.remesh(&palette.baked);
                let (maybe_mesh_handle, maybe_material_handle) =
                    maybe_atlas.map_or((None, None), |atlas| {
                        let mut material = palette
                            .create_atlas_material(&atlas, |_, image| self.images.add(image));
                        let mesh_handle = self.meshes.add(mesh::to_bevy_mesh(
                            atlas.mesh,
                            data.model_source == VoxelModelSourcePolicy::Remeshable,
                        ));
                        configure_material(&mut material, average_ior, &data);
                        (Some(mesh_handle), Some(self.materials.add(material)))
                    });
                let cloud_image = maybe_cloud.map(|image| self.images.add(image));

                let model = VoxelModel {
                    name: format!("{}-{}", name, index),
                    data,
                    has_mesh: maybe_mesh_handle.is_some(),
                    has_cloud: cloud_image.is_some(),
                    material: maybe_material_handle.clone(),
                };
                let model_size = model.model_size();
                let model_source = match model.data.model_source {
                    VoxelModelSourcePolicy::Discard => None,
                    VoxelModelSourcePolicy::Queryable => Some(VoxelModelSource::Queryable {
                        model: self.models.add(model),
                        palette: palette_handle.clone(),
                    }),
                    VoxelModelSourcePolicy::Remeshable => Some(VoxelModelSource::Remeshable {
                        model: self.models.add(model),
                        palette: palette_handle.clone(),
                    }),
                };
                let mut frame = spawner.spawn((
                    VoxelModelInstance,
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
                    frame.insert(Mesh3d(mesh_handle));
                    if let Some(material_handle) = maybe_material_handle {
                        frame.insert(MeshMaterial3d(material_handle));
                    }
                }
                if cloud_image.is_some() {
                    frame.with_child((
                        FogVolume {
                            density_texture: cloud_image,
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
        self.scenes.add(scene)
    }
}

pub(crate) fn configure_material(
    material: &mut StandardMaterial,
    average_ior: Option<f32>,
    data: &VoxelData,
) {
    if let Some(ior) = average_ior {
        material.ior = ior;
        material.thickness = data._size().min_element() as f32 * data.bake_settings.voxel_size;
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
        SDF, VoxLoaderSettings, Voxel, VoxelModel, VoxelModelInstance, VoxelModelSource,
        VoxelPalette, VoxelSceneAssets, test_support::setup_app,
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
            VoxLoaderSettings::default(),
            Voxel(1),
        );
        let world = app.world_mut();
        let mut state = SystemState::<VoxelSceneAssets>::new(world);
        let scene_handle = {
            let mut voxel_assets = state.get_mut(world).expect("voxel asset resources exist");
            let palette_handle = voxel_assets.add_palette(palette);
            voxel_assets.create_scene(tall_box, "tall box", palette_handle)
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
