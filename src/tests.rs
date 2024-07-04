use std::f32::consts::FRAC_PI_2;

use super::*;

#[cfg(feature = "modify_voxels")]
use crate::{model::queryable::OutOfBoundsError, VoxelRegion};

use crate::{model::RawVoxel, scene::VoxelModelInstance, VoxScenePlugin};
use bevy::{
    app::App,
    asset::{AssetApp, AssetPlugin, AssetServer, Assets, Handle, LoadState},
    core::Name,
    ecs::system::{Commands, Res},
    hierarchy::Children,
    math::{IVec3, Quat, UVec3, Vec3, Vec3A},
    pbr::StandardMaterial,
    render::{mesh::Mesh, texture::ImagePlugin},
    utils::hashbrown::HashSet,
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
    let scene = app
        .world()
        .resource::<Assets<VoxelScene>>()
        .get(handle.id())
        .expect("retrieve test.vox from Res<Assets>");
    let collection = app
        .world()
        .resource::<Assets<VoxelModelCollection>>()
        .get(scene.model_collection.id())
        .expect("Retrieve collection");
    assert_eq!(
        collection.models.len(),
        3,
        "Same 3 models are instanced through the scene"
    );
    assert_eq!(scene.layers.len(), 8);
    assert_eq!(
        scene
            .layers
            .first()
            .unwrap()
            .name
            .as_ref()
            .expect("Layer 0 name"),
        "scenery"
    );
    let outer_group = scene.root.children.first().expect("First object in scene");
    assert_eq!(
        outer_group.name.as_ref().expect("Name of first obj"),
        "outer-group"
    );
    assert_eq!(outer_group.children.len(), 3);
    let inner_group = outer_group
        .children
        .first()
        .expect("First child of outer-group");
    assert_eq!(
        inner_group.name.as_ref().expect("name of inner group"),
        "outer-group/inner-group"
    );
}

#[async_std::test]
async fn test_load_scene_slice() {
    let mut app = App::new();
    let handle = setup_and_load_voxel_scene(&mut app, "test.vox#outer-group/inner-group").await;
    app.update();
    let scene = app
        .world()
        .resource::<Assets<VoxelScene>>()
        .get(handle.id())
        .expect("retrieve test.vox from Res<Assets>");
    assert_eq!(scene.layers.len(), 8);
    assert_eq!(
        scene
            .layers
            .first()
            .unwrap()
            .name
            .as_ref()
            .expect("Layer 0 name"),
        "scenery"
    );
    let inner_group = &scene.root;
    assert_eq!(
        inner_group.name.as_ref().expect("Name of first obj"),
        "outer-group/inner-group"
    );
    assert_eq!(inner_group.children.len(), 4);
    let dice = inner_group
        .children
        .last()
        .expect("Last child of inner-group");
    assert_eq!(
        dice.name.as_ref().expect("name of dice"),
        "outer-group/inner-group/dice"
    );
}

#[async_std::test]
async fn test_transmissive_mat() {
    let mut app = App::new();
    let handle =
        setup_and_load_voxel_scene(&mut app, "test.vox#outer-group/inner-group/walls").await;
    app.update();
    let scene = app
        .world()
        .resource::<Assets<VoxelScene>>()
        .get(handle.id())
        .expect("retrieve scene from Res<Assets>");
    let walls = &scene.root;
    let model_id = walls.model_id.expect("Walls has a model id");
    let collection = app
        .world()
        .resource::<Assets<VoxelModelCollection>>()
        .get(scene.model_collection.id())
        .expect("Retrieve collection");
    let model = collection.models.get(model_id).expect("Walls has a model");
    let mat_handle = &model.material;
    let material = app
        .world()
        .resource::<Assets<StandardMaterial>>()
        .get(mat_handle)
        .expect("material");
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
    app.update();
    let scene = app
        .world()
        .resource::<Assets<VoxelScene>>()
        .get(handle.id())
        .expect("retrieve scene from Res<Assets>");
    let dice = &scene.root;
    let model_id = dice.model_id.expect("Dice has a model id");
    let collection = app
        .world()
        .resource::<Assets<VoxelModelCollection>>()
        .get(scene.model_collection.id())
        .expect("Retrieve collection");
    let model = collection
        .models
        .get(model_id)
        .expect("retrieve model from collection");
    let mat_handle = &model.material;
    let material = app
        .world()
        .resource::<Assets<StandardMaterial>>()
        .get(mat_handle)
        .expect("material");
    assert!(material.specular_transmission_texture.is_none());
    assert_eq!(material.specular_transmission, 0.0);
    assert!(material.metallic_roughness_texture.is_some());
}

#[async_std::test]
async fn test_spawn_system() {
    let mut app = App::new();
    let handle = setup_and_load_voxel_scene(&mut app, "test.vox#outer-group/inner-group").await;
    app.update();

    assert_eq!(
        app.world()
            .resource::<AssetServer>()
            .load_state(handle.id()),
        LoadState::Loaded
    );
    let entity = app
        .world_mut()
        .spawn(VoxelSceneHookBundle {
            scene: handle,
            hook: VoxelSceneHook::new(move |entity, _| {
                let Some(name) = entity.get::<Name>() else {
                    return;
                };
                let expected_names: [&'static str; 3] = [
                    "outer-group/inner-group",
                    "outer-group/inner-group/dice",
                    "outer-group/inner-group/walls",
                ];
                assert!(expected_names.contains(&name.as_str()));
            }),
            ..Default::default()
        })
        .id();
    app.update();
    assert!(app.world().get::<Handle<VoxelScene>>(entity).is_none());
    assert_eq!(
        app.world_mut().query::<&VoxelLayer>().iter(&app.world()).len(),
        5,
        "5 voxel nodes spawned in this scene slice"
    );
    assert_eq!(
        app.world_mut().query::<&Name>().iter(&app.world()).len(),
        3,
        "But only 3 of the voxel nodes are named"
    );
    let mut instance_query = app.world_mut().query::<&VoxelModelInstance>();
    assert_eq!(
        instance_query.iter(&app.world()).len(),
        4,
        "4 model instances spawned in this scene slice"
    );
    let models: HashSet<String> = instance_query
        .iter(&app.world())
        .map(|c| c.model_name.clone())
        .collect();
    assert_eq!(models.len(), 2, "Instances point to 2 unique models");
    assert_eq!(
        app.world()
            .get::<Name>(entity)
            .expect("Name component")
            .as_str(),
        "outer-group/inner-group"
    );
    let children = app
        .world()
        .get::<Children>(entity)
        .expect("children of inner-group")
        .as_ref();
    assert_eq!(children.len(), 4, "inner-group has 4 children");
    assert_eq!(
        app.world()
            .get::<Name>(*children.last().expect("last child"))
            .expect("Name component")
            .as_str(),
        "outer-group/inner-group/dice"
    );
    app.update(); // fire the hooks
}

#[cfg(feature = "modify_voxels")]
#[async_std::test]
async fn test_modify_voxels() {
    use bevy::ecs::system::RunSystemOnce;

    let mut app = App::new();
    let handle =
        setup_and_load_voxel_scene(&mut app, "test.vox#outer-group/inner-group/dice").await;
    app.update();
    app.world_mut().run_system_once(modify_voxels);
    app.update();
    let scene = app
        .world()
        .resource::<Assets<VoxelScene>>()
        .get(handle.id())
        .expect("retrieve scene from Res<Assets>");
    let model_id = scene.root.model_id.expect("Root should have a model");
    let collection = app
        .world()
        .resource::<Assets<VoxelModelCollection>>()
        .get(scene.model_collection.id())
        .expect("Retrieve collection");

    let model = collection
        .models
        .get(model_id)
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

#[cfg(feature = "modify_voxels")]
fn modify_voxels(mut commands: Commands, scenes: Res<Assets<VoxelScene>>) {
    let (_, scene) = scenes.iter().next().expect("a scene has been added");
    let collection_id = &scene.model_collection;
    let region = VoxelRegion {
        origin: IVec3::splat(2),
        size: IVec3::ONE,
    };
    let instance = VoxelModelInstance {
        collection: collection_id.clone(),
        model_name: "outer-group/inner-group/dice".to_string(),
    };
    commands.modify_voxel_model(
        instance,
        VoxelRegionMode::Box(region),
        |_pos, _voxel, _model| Voxel(7),
    );
}

#[cfg(feature = "generate_voxels")]
#[test]
fn test_generate_voxels() {
    let mut app = App::new();
    setup_app(&mut app);
    let palette = VoxelPalette::from_colors(vec![bevy::color::palettes::css::GREEN.into()]);
    let tall_box = SDF::cuboid(Vec3::new(0.5, 2.5, 0.5)).voxelize(UVec3::splat(6), Voxel(1));
    let world = app.world_mut();
    let collection = VoxelModelCollection::new(world, palette).expect("create collection");
    let tall_box_model =
        VoxelModelCollection::add(world, tall_box, "tall box".to_string(), collection)
            .expect("Add box model");
    assert_eq!(tall_box_model.name, "tall box");
    assert_eq!(tall_box_model.has_translucency, false);
    let mesh = app
        .world()
        .resource::<Assets<Mesh>>()
        .get(tall_box_model.mesh.id())
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
        .voxelize(UVec3::splat(7), Voxel(1));
    let sphere_box = SDF::sphere(2.5)
        .intersect(SDF::cuboid(Vec3::splat(2.0)))
        .voxelize(UVec3::splat(7), Voxel(1));
    assert_eq!(box_sphere.voxels, sphere_box.voxels);
}

#[cfg(feature = "generate_voxels")]
#[test]
fn test_sdf_subtract() {
    let thin_box = SDF::cuboid(Vec3::new(1.0, 2.0, 2.0)).voxelize(UVec3::splat(6), Voxel(1));
    let halved_cube = SDF::cuboid(Vec3::new(2.0, 2.0, 2.0))
        .subtract(SDF::cuboid(Vec3::new(1.0, 2.0, 2.0)).translate(Vec3::X))
        .translate(Vec3::X)
        .voxelize(UVec3::splat(6), Voxel(1));
    assert_eq!(thin_box.voxels, halved_cube.voxels);
}

#[cfg(feature = "generate_voxels")]
#[test]
fn test_sdf_rotate() {
    let tall_box = SDF::cuboid(Vec3::new(0.5, 2.5, 0.5)).voxelize(UVec3::splat(6), Voxel(1));
    let deep_box_rotated = SDF::cuboid(Vec3::new(0.5, 0.5, 2.5))
        .rotate(Quat::from_axis_angle(Vec3::X, FRAC_PI_2))
        .voxelize(UVec3::splat(6), Voxel(1));
    assert_eq!(tall_box.voxels, deep_box_rotated.voxels);
}

#[cfg(feature = "generate_voxels")]
#[test]
fn test_voxel_queryable() {
    let data = SDF::cuboid(Vec3::splat(2.0)).voxelize(UVec3::splat(4), Voxel(1));
    assert!(data.point_in_model(IVec3::new(3, 0, 0)).is_ok());
    assert!(data.point_in_model(IVec3::new(4, 0, 0)).is_err());
    assert_eq!(
        data.local_point_to_voxel_space(Vec3::ZERO),
        IVec3::new(2, 2, 2)
    );
}

async fn setup_and_load_voxel_scene(app: &mut App, filename: &'static str) -> Handle<VoxelScene> {
    setup_app(app);
    let assets = app.world().resource::<AssetServer>();
    assets
        .load_untyped_async(filename)
        .await
        .expect(format!("Loaded {filename}").as_str())
        .typed::<VoxelScene>()
}

fn setup_app(app: &mut App) {
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        ImagePlugin::default(),
        VoxScenePlugin,
    ))
    .init_asset::<StandardMaterial>()
    .init_asset::<Mesh>();
}
