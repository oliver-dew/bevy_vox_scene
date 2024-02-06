use super::*;

#[cfg(feature = "modify_voxels")]
use crate::{model::queryable::OutOfBoundsError, VoxelRegion};

use crate::{model::RawVoxel, scene::VoxelModelInstance, VoxScenePlugin};
use bevy::{
    app::App,
    asset::{AssetApp, AssetPlugin, AssetServer, Assets, Handle, LoadState},
    core::Name,
    ecs::system::{Commands, Res, RunSystemOnce},
    hierarchy::Children,
    math::IVec3,
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
        .world
        .resource::<Assets<VoxelScene>>()
        .get(handle)
        .expect("retrieve test.vox from Res<Assets>");
    let collection = app
        .world
        .resource::<Assets<ModelCollection>>()
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
        .world
        .resource::<Assets<VoxelScene>>()
        .get(handle)
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
        .world
        .resource::<Assets<VoxelScene>>()
        .get(handle)
        .expect("retrieve scene from Res<Assets>");
    let walls = &scene.root;
    let model_id = walls.model_id.expect("Walls has a model id");
    let collection = app
        .world
        .resource::<Assets<ModelCollection>>()
        .get(scene.model_collection.id())
        .expect("Retrieve collection");
    let model = collection.models.get(model_id).expect("Walls has a model");
    let mat_handle = &model.material;
    let material = app
        .world
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
        .world
        .resource::<Assets<VoxelScene>>()
        .get(handle)
        .expect("retrieve scene from Res<Assets>");
    let dice = &scene.root;
    let model_id = dice.model_id.expect("Dice has a model id");
    let collection = app
        .world
        .resource::<Assets<ModelCollection>>()
        .get(scene.model_collection.id())
        .expect("Retrieve collection");
    let model = collection
        .models
        .get(model_id)
        .expect("retrieve model from collection");
    let mat_handle = &model.material;
    let material = app
        .world
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
        app.world
            .resource::<AssetServer>()
            .load_state(handle.clone()),
        LoadState::Loaded
    );
    let entity = app
        .world
        .spawn(VoxelSceneHookBundle {
            scene: handle,
            hook: VoxelSceneHook::new(move |entity, _| {
                let Some(name) = entity.get::<Name>() else { return };
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
    assert!(app.world.get::<Handle<VoxelScene>>(entity).is_none());
    assert_eq!(
        app.world.query::<&VoxelLayer>().iter(&app.world).len(),
        5,
        "5 voxel nodes spawned in this scene slice"
    );
    assert_eq!(
        app.world.query::<&Name>().iter(&app.world).len(),
        3,
        "But only 3 of the voxel nodes are named"
    );
    let mut instance_query = app.world.query::<&VoxelModelInstance>();
    assert_eq!(
        instance_query.iter(&app.world).len(),
        4,
        "4 model instances spawned in this scene slice"
    );
    let models: HashSet<usize> = instance_query
        .iter(&app.world)
        .map(|c| c.model_index.clone())
        .collect();
    assert_eq!(models.len(), 2, "Instances point to 2 unique models");
    assert_eq!(
        app.world
            .get::<Name>(entity)
            .expect("Name component")
            .as_str(),
        "outer-group/inner-group"
    );
    let children = app
        .world
        .get::<Children>(entity)
        .expect("children of inner-group")
        .as_ref();
    assert_eq!(children.len(), 4, "inner-group has 4 children");
    assert_eq!(
        app.world
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
    let mut app = App::new();
    let handle =
        setup_and_load_voxel_scene(&mut app, "test.vox#outer-group/inner-group/dice").await;
    app.update();
    app.world.run_system_once(modify_voxels);
    app.update();
    let scene = app
        .world
        .resource::<Assets<VoxelScene>>()
        .get(handle)
        .expect("retrieve scene from Res<Assets>");
    let model_id = scene.root.model_id.expect("Root should have a model");
    let collection = app
        .world
        .resource::<Assets<ModelCollection>>()
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
fn modify_voxels(
    mut commands: Commands,
    scenes: Res<Assets<VoxelScene>>,
    models: Res<Assets<ModelCollection>>,
) {
    let (_, scene) = scenes.iter().next().expect("a scene has been added");
    let collection_id = &scene.model_collection;
    let collection = models
        .get(collection_id.id())
        .expect("A model collection has been added");
    let model_index: usize = collection
        .models
        .iter()
        .enumerate()
        .filter_map(|(index, model)| {
            if model.size() == IVec3::splat(4) {
                Some(index)
            } else {
                None
            }
        })
        .next()
        .expect("There should be a dice model the size of which is 4 x 4 x 4");
    let region = VoxelRegion {
        origin: IVec3::splat(2),
        size: IVec3::ONE,
    };
    let instance = VoxelModelInstance {
        collection: collection_id.clone(),
        model_index,
    };
    commands.modify_voxel_model(
        instance,
        VoxelRegionMode::Box(region),
        |_pos, _voxel, _model| Voxel(7),
    );
}

async fn setup_and_load_voxel_scene(app: &mut App, filename: &'static str) -> Handle<VoxelScene> {
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        ImagePlugin::default(),
        VoxScenePlugin,
    ))
    .init_asset::<StandardMaterial>()
    .init_asset::<Mesh>();
    let assets = app.world.resource::<AssetServer>();
    assets
        .load_untyped_async(filename)
        .await
        .expect(format!("Loaded {filename}").as_str())
        .typed::<VoxelScene>()
}
