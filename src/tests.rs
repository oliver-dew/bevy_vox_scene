use std::{f32::consts::FRAC_PI_2, time::Duration};

use super::*;

#[cfg(feature = "modify_voxels")]
use crate::{model::queryable::OutOfBoundsError, VoxelRegion};

use crate::{model::RawVoxel, VoxScenePlugin, VoxelModelInstance};
use bevy::{
    app::App,
    asset::{AssetApp, AssetPlugin, AssetServer, Assets, Handle, LoadState},
    ecs::{hierarchy::Children, name::Name},
    math::{IVec3, Quat, UVec3, Vec3, Vec3A},
    pbr::{FogVolume, MeshMaterial3d, StandardMaterial},
    platform_support::collections::HashSet,
    prelude::{
        Commands, GlobalTransform, InheritedVisibility, Mesh3d, OnAdd, Query, Transform, Trigger,
        ViewVisibility, Visibility,
    },
    render::{mesh::Mesh, texture::ImagePlugin},
    scene::{Scene, ScenePlugin, SceneRoot},
    transform::components::TransformTreeChanged,
    utils::default,
    MinimalPlugins,
};

#[test]
fn test_raw_voxel_conversion() {
    let raw = RawVoxel::EMPTY;
    let vox: Voxel = raw.into();
    assert_eq!(
        vox,
        Voxel::EMPTY,
        "RawVoxel(255) should have wrapped around to Voxel(0)"
    );
    let back_to_raw = RawVoxel::from(vox);
    assert_eq!(
        back_to_raw,
        RawVoxel::EMPTY,
        "Voxel(0) should have wrapped around to RawVoxel(255)"
    );
}

#[async_std::test]
async fn test_load_scene() {
    let mut app = App::new();
    let handle = setup_and_load_voxel_scene(&mut app, "test.vox").await;
    app.update();
    let _scene = app
        .world()
        .resource::<Assets<Scene>>()
        .get(handle.id())
        .expect("retrieve test.vox from Res<Assets>");
    let models = app.world().resource::<Assets<VoxelModel>>();
    assert_eq!(
        models.len(),
        4,
        "Same 4 models are instanced through the scene"
    );
}

#[async_std::test]
async fn test_load_spawn_cloud() {
    let mut app = App::new();
    let handle =
        setup_and_load_voxel_scene(&mut app, "test.vox#outer-group/inner-group/cloud").await;
    app.update();
    let scene_root = app.world_mut().spawn(SceneRoot(handle)).id();
    app.update();
    let entity = app
        .world()
        .get::<Children>(scene_root)
        .expect("children")
        .first()
        .expect("scene root");
    let model_instance = app
        .world()
        .get::<VoxelModelInstance>(*entity)
        .expect("voxel model instance")
        .clone();
    let model = app
        .world()
        .resource::<Assets<VoxelModel>>()
        .get(model_instance.model[0].id())
        .expect("retrieve model from Res<Assets>");
    let fog_entity = app
        .world()
        .get::<Children>(*entity)
        .expect("children")
        .first()
        .expect("fog entity");
    let fog_volume = app
        .world()
        .get::<FogVolume>(*fog_entity)
        .expect("fog volume");

    assert_ne!(
        model.cloud_image, None,
        "Model with cloud voxels should have a cloud image"
    );
    assert_eq!(
        model.cloud_image, fog_volume.density_texture,
        "FogVolume should have spawned with cloud texture from VoxelModel"
    );
    assert_eq!(
        model.mesh, None,
        "Model consisting solely of cloud voxels shouldn't have a mesh"
    );
    assert_eq!(
        model.material, None,
        "Model consisting solely of cloud voxels shouldn't have a material"
    );
}

#[async_std::test]
async fn test_spawn_play_animation() {
    let frame_count: usize = 4;
    let mut app = App::new();
    let handle = setup_and_load_voxel_scene(&mut app, "deer.vox").await;
    app.update();
    let scene_root = app
        .world_mut()
        .spawn(SceneRoot(handle))
        // Use an observer to override the default `VoxelAnimationPlayer` with one that has a very fast `frame_rate`
        // so we can advance a frame on each call to `app.update`
        .observe(
            move |trigger: Trigger<VoxelInstanceReady>, mut commands: Commands| {
                commands
                    .entity(trigger.event().instance)
                    .insert(VoxelAnimationPlayer {
                        frames: (0..frame_count).collect(),
                        frame_rate: Duration::from_millis(1),
                        ..default()
                    });
            },
        )
        .id();
    app.update();
    app.update(); // trigger second frame
    let top_entity = app
        .world()
        .get::<Children>(scene_root)
        .expect("children")
        .first()
        .expect("scene root");
    let entity = app
        .world()
        .get::<Children>(*top_entity)
        .expect("children")
        .first()
        .expect("model entity");
    let model_instance = app
        .world()
        .get::<VoxelModelInstance>(*entity)
        .expect("voxel model instance")
        .clone();
    assert!(model_instance.has_animation());
    assert_eq!(model_instance.model.len(), frame_count);
    let frame_entities = app.world().get::<Children>(*entity).expect("children");
    assert_eq!(frame_entities.len(), frame_count);
    let first_frame_visibility = app
        .world()
        .get::<Visibility>(frame_entities[0])
        .expect("Visibility of first frame");
    assert_eq!(
        first_frame_visibility,
        Visibility::Hidden,
        "Frame 0 invisible"
    );
    let second_frame_visibility = app
        .world()
        .get::<Visibility>(frame_entities[1])
        .expect("Visibility of second frame");
    assert_eq!(
        second_frame_visibility,
        Visibility::Inherited,
        "Frame 1 is showing"
    );
}

#[async_std::test]
async fn test_transmissive_mat() {
    let mut app = App::new();
    let handle =
        setup_and_load_voxel_scene(&mut app, "test.vox#outer-group/inner-group/walls").await;
    let scene_root = app.world_mut().spawn(SceneRoot(handle)).id();
    app.update();
    let entity = app
        .world()
        .get::<Children>(scene_root)
        .expect("children")
        .first()
        .expect("scene root");

    let model_id = &app
        .world()
        .get::<VoxelModelInstance>(*entity)
        .expect("Voxel model instance")
        .model[0];

    let model = app
        .world()
        .resource::<Assets<VoxelModel>>()
        .get(model_id)
        .expect("Walls has a model");
    assert_eq!(
        model.cloud_image, None,
        "Model with no cloud voxels should not have a cloud image"
    );
    let mat_handle = model.material.clone().expect("Model has a material handle");
    let material = app
        .world()
        .resource::<Assets<StandardMaterial>>()
        .get(&mat_handle)
        .expect("material");
    #[cfg(feature = "pbr_transmission_textures")]
    assert!(material.specular_transmission_texture.is_some());
    assert_eq!(material.specular_transmission, 1.0);
    assert!((material.ior - 1.3).abs() / 1.3 <= 0.0001);
    assert!(material.metallic_roughness_texture.is_some());
}

#[async_std::test]
async fn test_opaque_mat() {
    let mut app = App::new();
    let handle =
        setup_and_load_voxel_scene(&mut app, "test.vox#outer-group/inner-group/dice").await;
    let scene_root = app.world_mut().spawn(SceneRoot(handle)).id();
    app.update();
    let entity = app
        .world()
        .get::<Children>(scene_root)
        .expect("children")
        .first()
        .expect("scene root");

    let model_id = &app
        .world()
        .get::<VoxelModelInstance>(*entity)
        .expect("Voxel model instance")
        .model[0];

    let model = app
        .world()
        .resource::<Assets<VoxelModel>>()
        .get(model_id)
        .expect("voxel model");
    let mat_handle = model.material.clone().expect("Model has a material handle");
    let material = app
        .world()
        .resource::<Assets<StandardMaterial>>()
        .get(&mat_handle)
        .expect("material");
    #[cfg(feature = "pbr_transmission_textures")]
    assert!(material.specular_transmission_texture.is_none());
    assert_eq!(material.specular_transmission, 0.0);
    assert!(material.metallic_roughness_texture.is_some());
}

#[async_std::test]
async fn test_spawn_system() {
    let mut app = App::new();
    let handle = setup_and_load_voxel_scene(&mut app, "test.vox#outer-group/inner-group").await;
    app.update();

    assert!(matches!(
        app.world()
            .resource::<AssetServer>()
            .load_state(handle.id()),
        LoadState::Loaded
    ));
    app.add_observer(|trigger: Trigger<OnAdd, Name>, query: Query<&Name>| {
        let name = query.get(trigger.target()).unwrap().as_str();
        let expected_names: [&'static str; 4] = [
            "outer-group/inner-group",
            "outer-group/inner-group/dice",
            "outer-group/inner-group/walls",
            "outer-group/inner-group/cloud",
        ];
        assert!(expected_names.contains(&name));
    });
    let scene_root = app.world_mut().spawn(SceneRoot(handle)).id();
    app.update();
    assert_eq!(
        app.world_mut()
            .query::<&VoxelLayer>()
            .iter(&app.world())
            .len(),
        6,
        "6 voxel nodes spawned in this scene slice"
    );
    assert_eq!(
        app.world_mut().query::<&Name>().iter(&app.world()).len(),
        4,
        "But only 4 of the voxel nodes are named"
    );
    let mut instance_query = app.world_mut().query::<&VoxelModelInstance>();
    assert_eq!(
        instance_query.iter(&app.world()).len(),
        5,
        "5 model instances spawned in this scene slice"
    );
    let models: HashSet<String> = instance_query
        .iter(&app.world())
        .map(|c| c.model[0].id().to_string().clone())
        .collect();
    assert_eq!(models.len(), 3, "Instances point to 3 unique models");
    let entity = app
        .world()
        .get::<Children>(scene_root)
        .expect("children")
        .first()
        .expect("scene root");
    assert_eq!(
        app.world()
            .get::<Name>(*entity)
            .expect("Name component")
            .as_str(),
        "outer-group/inner-group"
    );
    let children = app
        .world()
        .get::<Children>(*entity)
        .expect("children of inner-group")
        .as_ref();
    assert_eq!(children.len(), 5, "inner-group has 5 children");
    assert_eq!(
        app.world()
            .get::<Name>(*children.last().expect("last child"))
            .expect("Name component")
            .as_str(),
        "outer-group/inner-group/cloud"
    );
    app.update(); // fire the hooks
}

#[cfg(feature = "modify_voxels")]
#[async_std::test]
async fn test_modify_voxels() {
    let mut app = App::new();
    let handle =
        setup_and_load_voxel_scene(&mut app, "test.vox#outer-group/inner-group/dice").await;
    app.update();
    let scene_root = app.world_mut().spawn(SceneRoot(handle)).id();
    app.update();
    let entity = app
        .world()
        .get::<Children>(scene_root)
        .expect("children")
        .first()
        .expect("scene root");
    let model_instance = app
        .world()
        .get::<VoxelModelInstance>(*entity)
        .expect("voxel model instance")
        .clone();
    let region = VoxelRegion {
        origin: IVec3::splat(2),
        size: IVec3::ONE,
    };
    app.world_mut().commands().modify_voxel_model(
        model_instance.clone(),
        VoxelRegionMode::Box(region),
        |_pos, _voxel, _model| Voxel(7),
    );
    app.update();
    let model = app
        .world()
        .resource::<Assets<VoxelModel>>()
        .get(model_instance.model[0].id())
        .expect("retrieve model from Res<Assets>");

    assert_eq!(
        model.get_voxel_at_point(IVec3::splat(4)),
        Err(OutOfBoundsError),
        "Max coordinate should be 3,3,3"
    );
    assert_eq!(
        model.get_voxel_at_point(IVec3::splat(-1)),
        Err(OutOfBoundsError),
        "Min coordinate should be 0,0,0"
    );
    let voxel = model
        .get_voxel_at_point(IVec3::splat(2))
        .expect("Retrieve voxel");
    assert_eq!(voxel.0, 7, "Voxel material should've been changed to 7");
}

#[cfg(feature = "generate_voxels")]
#[test]
fn test_generate_voxels() {
    use bevy::render::mesh::MeshAabb;

    let mut app = App::new();
    setup_app(&mut app);
    let palette = VoxelPalette::from_colors(vec![bevy::color::palettes::css::GREEN.into()], true);
    let tall_box = SDF::cuboid(Vec3::new(0.5, 2.5, 0.5)).voxelize(
        UVec3::splat(6),
        VoxLoaderSettings::default(),
        Voxel(1),
    );
    let world = app.world_mut();
    let context = VoxelContext::new(world, palette).expect("Context has been created");
    let (_, tall_box_model) =
        VoxelModel::new(world, tall_box, "tall box".to_string(), context).expect("Add box model");
    assert_eq!(tall_box_model.name, "tall box");
    assert_eq!(tall_box_model.has_translucency, false);
    let mesh_handle = tall_box_model
        .mesh
        .clone()
        .expect("Model has a Mesh handle");
    let mesh = app
        .world()
        .resource::<Assets<Mesh>>()
        .get(&mesh_handle)
        .expect("mesh generated");
    assert_eq!(
        mesh.compute_aabb().expect("aabb").half_extents,
        Vec3A::new(0.5, 2.5, 0.5)
    );
    assert_eq!(
        mesh.count_vertices(),
        6 * 4,
        "resulting mesh should have 6 quads"
    );
}

#[cfg(feature = "generate_voxels")]
#[test]
fn test_sdf_intersect() {
    let box_sphere = SDF::cuboid(Vec3::splat(2.0))
        .intersect(SDF::sphere(2.5))
        .voxelize(UVec3::splat(7), VoxLoaderSettings::default(), Voxel(1));
    let sphere_box = SDF::sphere(2.5)
        .intersect(SDF::cuboid(Vec3::splat(2.0)))
        .voxelize(UVec3::splat(7), VoxLoaderSettings::default(), Voxel(1));
    assert_eq!(box_sphere.voxels, sphere_box.voxels);
}

#[cfg(feature = "generate_voxels")]
#[test]
fn test_sdf_subtract() {
    let thin_box = SDF::cuboid(Vec3::new(1.0, 2.0, 2.0)).voxelize(
        UVec3::splat(6),
        VoxLoaderSettings::default(),
        Voxel(1),
    );
    let halved_cube = SDF::cuboid(Vec3::new(2.0, 2.0, 2.0))
        .subtract(SDF::cuboid(Vec3::new(1.0, 2.0, 2.0)).translate(Vec3::X))
        .translate(Vec3::X)
        .voxelize(UVec3::splat(6), VoxLoaderSettings::default(), Voxel(1));
    assert_eq!(thin_box.voxels, halved_cube.voxels);
}

#[cfg(feature = "generate_voxels")]
#[test]
fn test_sdf_rotate() {
    let tall_box = SDF::cuboid(Vec3::new(0.5, 2.5, 0.5)).voxelize(
        UVec3::splat(6),
        VoxLoaderSettings::default(),
        Voxel(1),
    );
    let deep_box_rotated = SDF::cuboid(Vec3::new(0.5, 0.5, 2.5))
        .rotate(Quat::from_axis_angle(Vec3::X, FRAC_PI_2))
        .voxelize(UVec3::splat(6), VoxLoaderSettings::default(), Voxel(1));
    assert_eq!(tall_box.voxels, deep_box_rotated.voxels);
}

#[cfg(feature = "generate_voxels")]
#[test]
fn test_voxel_queryable() {
    let data = SDF::cuboid(Vec3::splat(2.0)).voxelize(
        UVec3::splat(4),
        VoxLoaderSettings::default(),
        Voxel(1),
    );
    assert!(data.point_in_model(IVec3::new(3, 0, 0)).is_ok());
    assert!(data.point_in_model(IVec3::new(4, 0, 0)).is_err());
    assert_eq!(
        data.local_point_to_voxel_space(Vec3::ZERO),
        IVec3::new(2, 2, 2)
    );
}

async fn setup_and_load_voxel_scene(app: &mut App, filename: &'static str) -> Handle<Scene> {
    setup_app(app);
    let assets = app.world().resource::<AssetServer>();
    assets
        .load_untyped_async(filename)
        .await
        .expect(format!("Loaded {filename}").as_str())
        .typed::<Scene>()
}

fn setup_app(app: &mut App) {
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        ImagePlugin::default(),
        ScenePlugin,
        VoxScenePlugin::default(),
    ))
    .init_asset::<StandardMaterial>()
    .init_asset::<Mesh>()
    .init_asset::<Scene>()
    .register_type::<Visibility>()
    .register_type::<ViewVisibility>()
    .register_type::<InheritedVisibility>()
    .register_type::<Transform>()
    .register_type::<GlobalTransform>()
    .register_type::<TransformTreeChanged>()
    .register_type::<Mesh3d>()
    .register_type::<MeshMaterial3d<StandardMaterial>>();
}
