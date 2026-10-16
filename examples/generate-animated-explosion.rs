use std::{
    ops::Range,
    time::{Duration, Instant},
};

use bevy::{
    color::palettes::css, post_process::bloom::Bloom, prelude::*, render::view::Tonemapping,
    time::common_conditions::on_timer,
};
use bevy_vox_scene::{
    SDF, VoxBakeSettings, VoxScenePlugin, Voxel, VoxelElement, VoxelInstanceReady, VoxelModel,
    VoxelPalette, VoxelSceneAssets, export_voxel_animation_to_vox,
};
use flipbook::{FlipbookRepeatMode, FlipbookSpeed};
use libnoise::prelude::*;
use rand::{Rng, seq::IndexedRandom};
use rayon::prelude::*;
mod shared;
use shared::{NoiseSampleExt, PanOrbitCamera, PanOrbitCameraPlugin};
use vox_bake::VoxelTextureMode;

#[derive(Resource)]
struct ExplosionAsset(Handle<WorldAsset>);

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            VoxScenePlugin::default(),
            PanOrbitCameraPlugin,
        ))
        .add_systems(Startup, (generate_animation, spawn_camera))
        .add_systems(
            Update,
            spawn_explosion.run_if(on_timer(Duration::from_secs_f32(0.3))),
        )
        .run();
}

fn generate_animation(mut commands: Commands, mut voxel_assets: VoxelSceneAssets) {
    const EMISSION_STRENGTH: f32 = 5.0;
    let stops: Vec<(u8, VoxelElement)> = vec![
        (
            0,
            VoxelElement {
                color: Color::srgba_u8(20, 0, 0, 255),
                ..default()
            },
        ),
        (
            50,
            VoxelElement {
                color: css::RED.into(),
                emission: 4.0 * EMISSION_STRENGTH,
                ..default()
            },
        ),
        (
            100,
            VoxelElement {
                color: css::ORANGE_RED.into(),
                emission: 5.0 * EMISSION_STRENGTH,
                ..default()
            },
        ),
        (
            150,
            VoxelElement {
                color: css::ORANGE.into(),
                emission: 6.0 * EMISSION_STRENGTH,
                ..default()
            },
        ),
        (
            200,
            VoxelElement {
                color: css::YELLOW.into(),
                emission: 7.0 * EMISSION_STRENGTH,
                ..default()
            },
        ),
        (
            254,
            VoxelElement {
                color: css::ANTIQUE_WHITE.into(),
                emission: 8.0 * EMISSION_STRENGTH,
                ..default()
            },
        ),
    ];
    let palette = VoxelPalette::from_gradient(&stops, false);
    let palette_handle = voxel_assets.add_palette(palette.clone());
    let frame_count = 15;
    let max_diameter = 64.;
    let noise_scale: f32 = 0.75;
    let noise_speed: f32 = 1.0;
    let started = Instant::now();
    let frames: Vec<VoxelModel> = (0..frame_count)
        .into_par_iter()
        .map(|i| {
            let noise = Source::simplex(42).fbm(2, 0.083, 2.0, 0.5);
            let inner_noise = Source::simplex(9476).fbm(2, 0.083, 2.0, 0.5);
            let color_noise = Source::simplex(6742).fbm(2, 0.083, 2.0, 0.5);
            let time_norm = i as f32 / frame_count as f32;
            let diameter = max_diameter - ((1.0 - time_norm).powf(2.) * max_diameter * 0.9);
            let distortion = diameter * 0.4;
            let inner_max_diameter = max_diameter * 1.5;
            let inner_diameter =
                inner_max_diameter - ((1.0 - time_norm).powf(0.9) * inner_max_diameter);
            let settings = VoxBakeSettings {
                voxel_size: 0.1,
                texture_mode: VoxelTextureMode::Palette,
                ..default()
            };
            let frame = SDF::sphere(diameter * 0.5)
                .distort(move |sd, pos| {
                    let n_pos = (pos + Vec3::splat(256.0)) * noise_scale;
                    let distort = noise.sample_vec3_time(n_pos, time_norm * noise_speed);
                    sd + (distort * distortion)
                })
                .subtract(SDF::sphere(inner_diameter * 0.5).distort(move |sd, pos| {
                    let n_pos = (pos + Vec3::splat(256.0)) * noise_scale;
                    let distort = inner_noise.sample_vec3_time(n_pos, time_norm * noise_speed);
                    sd + (distort * distortion)
                }))
                .map_inside_to_voxels(
                    UVec3::splat((diameter + (distortion * 2.0)) as u32),
                    settings,
                    |pos| {
                        let pos = (pos + Vec3::splat(256.0)) * 1.4;
                        let color = ((color_noise.sample_vec3_time(pos, time_norm * 0.05) * 1.5)
                            + 0.2)
                            + (time_norm * 0.3);
                        Voxel((color * 255.).clamp(1., 255.) as u8)
                    },
                );
            frame
        })
        .collect();
    info!("voxel generation: {:?}", started.elapsed());
    if let Some(export_path) = std::env::args().nth(1) {
        let bytes = export_voxel_animation_to_vox(&frames, &palette)
            .unwrap_or_else(|error| panic!("failed to export {export_path}: {error}"));
        std::fs::write(export_path, bytes)
            .unwrap_or_else(|error| panic!("failed to write glb: {error}"));
    }
    let started = Instant::now();
    let root = voxel_assets
        .create_animation(frames, palette_handle, default())
        .expect("voxel animation created");
    info!("mesh generation: {:?}", started.elapsed());
    commands.insert_resource(ExplosionAsset(root));
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Bloom::ANAMORPHIC,
        Tonemapping::BlenderFilmic,
        Transform::from_xyz(0., 0., 24.).looking_at(Vec3::ZERO, Vec3::Y),
        PanOrbitCamera {
            radius: 24.,
            ..default()
        },
    ));
}

fn spawn_explosion(mut commands: Commands, asset: Res<ExplosionAsset>) {
    const RADIUS: Range<f32> = -10.0..10.0;
    let mut rng = rand::rng();
    let origin = Vec3::new(
        rng.random_range(RADIUS),
        rng.random_range(RADIUS),
        rng.random_range(RADIUS),
    )
    .clamp_length_max(10.);
    let cardinals = vec![
        Vec3::NEG_Y,
        Vec3::NEG_Y,
        Vec3::NEG_Z,
        Vec3::X,
        Vec3::Y,
        Vec3::Z,
    ];
    let forward = cardinals.choose(&mut rng).expect("direction");
    let rotation = Quat::from_rotation_arc(Vec3::Z, *forward);
    commands
        .spawn((
            WorldAssetRoot(asset.0.clone()),
            Transform::from_translation(origin).with_rotation(rotation),
        ))
        .observe(|ready: On<VoxelInstanceReady>, mut commands: Commands| {
            // Override the default animation settings
            commands.entity(ready.instance).insert((
                FlipbookRepeatMode::DespawnAfterCount(1),
                FlipbookSpeed::from_frames_per_second(12.),
            ));
        });
}
