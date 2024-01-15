use super::*;
use crate::{VoxScenePlugin, scene::VoxelModelInstance};
use bevy::{
    app::App,
    asset::{AssetApp, AssetPlugin, AssetServer, Assets, LoadState, Handle},
    core::Name,
    hierarchy::Children,
    render::{texture::ImagePlugin, mesh::Mesh},
    MinimalPlugins, pbr::StandardMaterial, utils::hashbrown::HashSet,
};

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
    let all_models: Vec<&VoxelModel> = app
        .world
        .resource::<Assets<VoxelModel>>()
        .iter()
        .map(|(_, asset)| asset)
        .collect();
    assert_eq!(
        all_models.len(),
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
    let model = app
        .world
        .resource::<Assets<VoxelModel>>()
        .get(walls.model.as_ref().expect("Walls has a model handle"))
        .expect("retrieve model from Res<Assets>");
    let mat_handle = &model.material;
    let material = app
        .world
        .resource::<Assets<StandardMaterial>>()
        .get(mat_handle)
        .expect("material");
    assert!(material.specular_transmission_texture.is_some());
    assert_eq!(material.specular_transmission, 1.0);
    assert!((material.ior - 1.3).abs() / 1.3 <= 0.00001);
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
    let model = app
        .world
        .resource::<Assets<VoxelModel>>()
        .get(dice.model.as_ref().expect("Walls has a model handle"))
        .expect("retrieve model from Res<Assets>");
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
    let models: HashSet<Handle<VoxelModel>> = instance_query.iter(&app.world).map(|c| c.0.clone()).collect();
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

/// `await` the response from this and then call `app.update()`
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
