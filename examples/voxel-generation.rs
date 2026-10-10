use bevy::{color::palettes::tailwind, post_process::bloom::Bloom, prelude::*};
use bevy_vox_scene::{
    SDF, VoxLoaderSettings, VoxScenePlugin, Voxel, VoxelElement, VoxelPalette, VoxelSceneAssets,
};
use libnoise::prelude::*;
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

fn setup(mut commands: Commands, mut voxel_assets: VoxelSceneAssets) {
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
    let palette = voxel_assets.add_palette(VoxelPalette::from_gradient(
        &[(0, stop_0), (85, stop_1), (170, stop_2), (255, stop_3)],
        true,
    ));
    let noise = Source::simplex(67890).fbm(4, 0.008, 2.34, 0.6);
    let data = SDF::cuboid(Vec3::splat(size as f32 * 0.45))
        .subtract(SDF::sphere(size as f32 * 0.5))
        .map_inside_to_voxels(
            UVec3::splat(size as u32),
            VoxLoaderSettings::default(),
            |pos| {
                let n_pos = (pos + Vec3::splat(256.0)) * 0.8;
                let color = (noise.sample([n_pos.x.into(), n_pos.y.into(), n_pos.z.into()]) as f32
                    * 0.8)
                    + 0.5;
                Voxel((color * 255.).clamp(1., 255.) as u8)
            },
        );
    let model_name = "my sdf model";
    let scene_root = voxel_assets.create_scene(data, model_name, palette);
    commands.spawn(WorldAssetRoot(scene_root));
}
