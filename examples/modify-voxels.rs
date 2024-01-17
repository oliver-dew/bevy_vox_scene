use std::time::Duration;

use bevy::{
    core_pipeline::{
        bloom::BloomSettings,
        core_3d::ScreenSpaceTransmissionQuality,
        experimental::taa::{TemporalAntiAliasBundle, TemporalAntiAliasPlugin},
        tonemapping::Tonemapping,
    },
    prelude::*,
    time::common_conditions::on_timer,
};
use bevy_panorbit_camera::{PanOrbitCamera, PanOrbitCameraPlugin};
use bevy_vox_scene::{
    BoxRegion, ModifyVoxelModel, VoxScenePlugin, Voxel, VoxelModel, VoxelModelInstance,
    VoxelRegion, VoxelScene, VoxelSceneBundle, VoxelSceneHook, VoxelSceneHookBundle,
};
use rand::Rng;

// When a snowflake lands on the scenery, it is added to scenery's voxel data, so that snow gradually builds up
fn main() {
    let mut app = App::new();
    // Making this frequency not cleanly divisible by the snowflake speed ensures that expensive collision checks
    // don't all happen on the same frame
    let snow_spawn_freq = Duration::from_secs_f32(0.213);
    app.add_plugins((DefaultPlugins, PanOrbitCameraPlugin, VoxScenePlugin))
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                spawn_snow.run_if(on_timer(snow_spawn_freq)),
                update_snow,
                remove_snow,
            ),
        );
    // *Note:* TAA is not _required_ for specular transmission, but
    // it _greatly enhances_ the look of the resulting blur effects.
    // Sadly, it's not available under WebGL.
    #[cfg(not(all(feature = "webgl2", target_arch = "wasm32")))]
    app.insert_resource(Msaa::Off)
        .add_plugins(TemporalAntiAliasPlugin);

    app.run();
}

#[derive(Resource)]
struct Scenes {
    snowflake: Handle<VoxelScene>,
}

fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn((
        Camera3dBundle {
            camera: Camera {
                hdr: true,
                ..Default::default()
            },
            camera_3d: Camera3d {
                screen_space_specular_transmission_quality: ScreenSpaceTransmissionQuality::High,
                screen_space_specular_transmission_steps: 1,
                ..default()
            },
            transform: Transform::from_xyz(15.0, 40.0, 90.0).looking_at(Vec3::ZERO, Vec3::Y),
            tonemapping: Tonemapping::SomewhatBoringDisplayTransform,
            ..Default::default()
        },
        PanOrbitCamera::default(),
        BloomSettings {
            intensity: 0.3,
            ..default()
        },
        #[cfg(not(all(feature = "webgl2", target_arch = "wasm32")))]
        TemporalAntiAliasBundle::default(),
        EnvironmentMapLight {
            diffuse_map: assets.load("pisa_diffuse.ktx2"),
            specular_map: assets.load("pisa_specular.ktx2"),
        },
    ));
    commands.insert_resource(Scenes {
        snowflake: assets.load("study.vox#snowflake"),
    });

    commands.spawn(VoxelSceneHookBundle {
        // Load a slice of the scene
        scene: assets.load("study.vox#workstation"),
        hook: VoxelSceneHook::new(|entity, commands| {
            if entity.get::<VoxelModelInstance>().is_some() {
                commands.insert(Scenery);
            }
        }),
        ..default()
    });
}

#[derive(Component)]
struct Snowflake;

#[derive(Component)]
struct Scenery;

#[derive(Component)]
struct ToBeDespawned;

fn spawn_snow(mut commands: Commands, scenes: Res<Scenes>) {
    let mut rng = rand::thread_rng();
    let position = Vec3::new(rng.gen_range(-30.0..30.0), 80.0, rng.gen_range(-20.0..20.0)).round()
        + Vec3::splat(0.5);
    commands.spawn((
        Snowflake,
        VoxelSceneBundle {
            scene: scenes.snowflake.clone(),
            transform: Transform::from_translation(position),
            ..default()
        },
    ));
}

fn update_snow(
    mut commands: Commands,
    mut snowflakes: Query<(Entity, &mut Transform), (With<Snowflake>, Without<Scenery>)>,
    scenery: Query<
        (Entity, &GlobalTransform, &VoxelModelInstance),
        (With<Scenery>, Without<Snowflake>),
    >,
    models: Res<Assets<VoxelModel>>,
) {
    for (snowflake, mut snowflake_xform) in snowflakes.iter_mut() {
        let old_ypos = snowflake_xform.translation.y;
        snowflake_xform.translation.y -= 0.1;
        // don't check collisions unless crossing boundary to next voxel
        if old_ypos.trunc() == snowflake_xform.translation.y.trunc() {
            continue;
        }
        for (item, item_xform, item_instance) in scenery.iter() {
            let Some(model) = models.get(item_instance.0.id()) else { continue  };
            let Some(vox_pos_below_snowflake) = model.data.global_point_to_voxel_space(snowflake_xform.translation - Vec3::Y, item_xform) else { continue };
            if model.data.get_voxel_at_point(vox_pos_below_snowflake) == Voxel::EMPTY {
                continue;
            };
            let flake_region = BoxRegion {
                origin: vox_pos_below_snowflake + UVec3::Y,
                size: UVec3::ONE,
            };
            commands.entity(item).insert(ModifyVoxelModel::new(
                VoxelRegion::Box(flake_region),
                |_, _| Voxel(234),
            ));
            commands.entity(snowflake).insert(ToBeDespawned);
        }
    }
}

/// Defer despawning by a frame, so that the flake doesn't flicker when it lands
fn remove_snow(mut commands: Commands, query: Query<Entity, With<ToBeDespawned>>) {
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
}
