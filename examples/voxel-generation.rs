use bevy::{color::palettes::tailwind, post_process::bloom::Bloom, prelude::*};
use bevy_vox_scene::{
    SDF, VoxLoaderSettings, VoxScenePlugin, Voxel, VoxelElement, VoxelPalette,
    create_voxel_context, create_voxel_scene,
};
use rand::{Rng, random, rng};
use utilities::{PanOrbitCamera, PanOrbitCameraPlugin};

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            PanOrbitCameraPlugin,
            VoxScenePlugin::default(),
        ))
        .add_systems(Startup, (setup_camera, setup))
        .run();
}

fn setup_camera(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(-20.0, 10.0, 60.0).looking_at(Vec3::ZERO, Vec3::Y),
        PanOrbitCamera::default(),
        Bloom {
            intensity: 0.3,
            ..default()
        },
        EnvironmentMapLight {
            diffuse_map: assets.load("pisa_diffuse.ktx2"),
            specular_map: assets.load("pisa_specular.ktx2"),
            intensity: 500.0,
            ..default()
        },
    ));
}

fn setup(world: &mut World) {
    let stop_0 = VoxelElement {
        color: tailwind::SKY_600.into(),
        roughness: 0.1,
        metalness: 0.2,
        translucency: 0.7,
        ..default()
    };
    let stop_1 = VoxelElement {
        color: tailwind::ROSE_600.into(),
        roughness: 0.7,
        metalness: 0.1,
        ..default()
    };
    let stop_2 = VoxelElement {
        color: tailwind::ORANGE_600.into(),
        roughness: 0.1,
        metalness: 0.9,
        ..default()
    };
    let stop_3 = VoxelElement {
        color: tailwind::LIME_600.into(),
        roughness: 0.1,
        metalness: 0.1,
        emission: 1.0,
        ..default()
    };
    let size = 64;
    let palette = VoxelPalette::from_gradient(
        &[
            (0, stop_0),
            (85, stop_1),
            (170, stop_2),
            (255, stop_3),
        ],
        true,
    );
    let data = SDF::cuboid(Vec3::splat(size as f32 * 0.45))
        .subtract(SDF::sphere(size as f32 * 0.5))
        .map_to_voxels(
            UVec3::splat(size as u32),
            VoxLoaderSettings::default(),
            |distance, pos| {
                if distance >= 0.0 {
                    return Voxel::EMPTY;
                };
                // map y coord to 0..256 palette range, and add a bit of random noise to dither the gradient
                let dither = random::<f32>() * 4.;
                let y_normalized = (pos.y.round() / size as f32) + 0.5;
                Voxel((y_normalized * 256. + dither) as u8)
            },
        );
    let context = world
        .run_system_cached_with(create_voxel_context, palette)
        .expect("Context has been generated");
    let model_name = "my sdf model";
    let scene_root = world
        .run_system_cached_with(
            create_voxel_scene,
            (data, model_name.to_string(), context.clone()),
        )
        .expect("Voxel scene created");
    world.spawn(WorldAssetRoot(scene_root));
}
