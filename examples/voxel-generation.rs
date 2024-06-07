use bevy::{core_pipeline::bloom::BloomSettings, prelude::*};
use utilities::{PanOrbitCamera, PanOrbitCameraPlugin};
use bevy_vox_scene::{
    VoxScenePlugin, Voxel, VoxelModelCollection, VoxelModelInstance, VoxelPalette, SDF,
};

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, PanOrbitCameraPlugin, VoxScenePlugin))
        .add_systems(Startup, (setup_camera, setup))
        .run();
}

fn setup_camera(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn((
        Camera3dBundle {
            camera: Camera {
                hdr: true,
                ..Default::default()
            },
            transform: Transform::from_xyz(-20.0, 10.0, 60.0).looking_at(Vec3::ZERO, Vec3::Y),
            ..Default::default()
        },
        PanOrbitCamera::default(),
        BloomSettings {
            intensity: 0.3,
            ..default()
        },
        EnvironmentMapLight {
            diffuse_map: assets.load("pisa_diffuse.ktx2"),
            specular_map: assets.load("pisa_specular.ktx2"),
            intensity: 500.0,
        },
    ));
}

fn setup(world: &mut World) {
    let palette = VoxelPalette::from_colors(vec![Color::BLUE, Color::ALICE_BLUE, Color::BISQUE]);
    let data = SDF::cuboid(Vec3::splat(13.0))
        .subtract(SDF::sphere(16.0))
        .map_to_voxels(UVec3::splat(32), |d, _| match d {
            x if x < -1.0 => Voxel(2),
            x if x < 0.0 => Voxel(1),
            x if x >= 0.0 => Voxel::EMPTY,
            _ => Voxel::EMPTY,
        });
    let Some(collection) = VoxelModelCollection::new(world, palette) else {
        return;
    };
    let model_name = "my sdf model";
    let Some(model) =
        VoxelModelCollection::add(world, data, model_name.to_string(), collection.clone())
    else {
        return;
    };
    world.spawn((
        PbrBundle {
            mesh: model.mesh,
            material: model.material,
            ..default()
        },
        // The [`VoxelModelInstance`] component is only needed if you want to be able to modify the model at a later time:
        VoxelModelInstance {
            collection,
            model_name: model_name.to_string(),
        },
    ));
}
