use bevy::{core_pipeline::bloom::BloomSettings, prelude::*};
use bevy_panorbit_camera::{PanOrbitCamera, PanOrbitCameraPlugin};
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
        },
    ));
}

fn setup(world: &mut World) {
    let palette =
        VoxelPalette::new_from_colors(vec![Color::BLUE, Color::ALICE_BLUE, Color::BISQUE]);

    let data = SDF::cuboid(Vec3::splat(13.0))
        .subtract(SDF::sphere(16.0))
        .map_to_voxels(UVec3::splat(32), |d| match d {
            x if x < -1.0 => Voxel(2),
            x if x < 0.0 => Voxel(1),
            x if x >= 0.0 => Voxel::EMPTY,
            _ => Voxel::EMPTY,
        });
    let Some((mut collection, collection_handle)) = VoxelModelCollection::new(world, palette) else { return };
    let model_name = "my sdf model";
    let Some(model) = collection.add(data, "my sdf model", world) else { return };
    world.spawn((
        PbrBundle {
            mesh: model.mesh,
            material: model.material,
            ..default()
        },
        VoxelModelInstance {
            collection: collection_handle,
            model_name: model_name.to_string(),
        },
    ));
}
