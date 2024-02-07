use std::time::Duration;

use bevy::{
    core_pipeline::{bloom::BloomSettings, tonemapping::Tonemapping},
    prelude::*,
    time::common_conditions::on_timer,
};
use bevy_panorbit_camera::{PanOrbitCamera, PanOrbitCameraPlugin};
use bevy_vox_scene::{
    VoxelModelCollection, ModifyVoxelCommandsExt, VoxScenePlugin, Voxel, VoxelModelInstance,
    VoxelQueryable, VoxelRegion, VoxelRegionMode, VoxelScene, VoxelSceneBundle, VoxelSceneHook,
    VoxelSceneHookBundle,
};
use rand::Rng;

// When a snowflake lands on the scenery, it is added to scenery's voxel data, so that snow gradually builds up
fn main() {
    // Making this frequency not cleanly divisible by the snowflake speed ensures that expensive collisions
    // don't all happen on the same frame
    let snow_spawn_freq = Duration::from_secs_f32(0.213);
    App::new()
        .add_plugins((DefaultPlugins, PanOrbitCameraPlugin, VoxScenePlugin))
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (spawn_snow.run_if(on_timer(snow_spawn_freq)), update_snow),
        )
        .run();
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
            transform: Transform::from_xyz(15.0, 40.0, 90.0).looking_at(Vec3::ZERO, Vec3::Y),
            tonemapping: Tonemapping::SomewhatBoringDisplayTransform,
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
struct Snowflake(Quat);

#[derive(Component)]
struct Scenery;

fn spawn_snow(mut commands: Commands, scenes: Res<Scenes>) {
    let mut rng = rand::thread_rng();
    let position = Vec3::new(rng.gen_range(-30.0..30.0), 80.0, rng.gen_range(-20.0..20.0)).round()
        + Vec3::splat(0.5);
    let rotation_axis =
        Vec3::new(rng.gen_range(-0.5..0.5), 1.0, rng.gen_range(-0.5..0.5)).normalize();
    let angular_velocity = Quat::from_axis_angle(rotation_axis, 0.01);
    commands.spawn((
        Snowflake(angular_velocity),
        VoxelSceneBundle {
            scene: scenes.snowflake.clone(),
            transform: Transform::from_translation(position),
            ..default()
        },
    ));
}

fn update_snow(
    mut commands: Commands,
    mut snowflakes: Query<(Entity, &Snowflake, &mut Transform), Without<Scenery>>,
    scenery: Query<(&GlobalTransform, &VoxelModelInstance), (With<Scenery>, Without<Snowflake>)>,
    model_collections: Res<Assets<VoxelModelCollection>>,
) {
    for (snowflake, snowflake_angular_vel, mut snowflake_xform) in snowflakes.iter_mut() {
        let old_ypos = snowflake_xform.translation.y;
        snowflake_xform.translation.y -= 0.1;
        snowflake_xform.rotation *= snowflake_angular_vel.0;
        // don't check collisions unless crossing boundary to next voxel
        if old_ypos.trunc() == snowflake_xform.translation.y.trunc() {
            continue;
        }
        for (item_xform, item_instance) in scenery.iter() {
            let Some(collection) = model_collections.get(item_instance.collection.id()) else { continue };
            let Some(model) = collection.model(&item_instance.model_name) else { continue  };
            let vox_pos =
                model.global_point_to_voxel_space(snowflake_xform.translation, item_xform);
            // check whether snowflake has landed on something solid
            let pos_below_snowflake = vox_pos - IVec3::Y;
            let Ok(voxel) = model.get_voxel_at_point(pos_below_snowflake) else { continue };
            if voxel == Voxel::EMPTY {
                continue;
            };
            let flake_radius = 2;
            let radius_squared = flake_radius * flake_radius;
            let flake_region = VoxelRegion {
                origin: vox_pos - IVec3::splat(flake_radius),
                size: IVec3::splat(1 + (flake_radius * 2)),
            };
            commands.modify_voxel_model(
                item_instance.clone(),
                VoxelRegionMode::Box(flake_region),
                move |pos, voxel, model| {
                    // a signed distance field for a sphere, but _only_ drawing it on empty cells directly above solid voxels
                    if *voxel == Voxel::EMPTY && pos.distance_squared(vox_pos) <= radius_squared {
                        if let Ok(voxel_below) = model.get_voxel_at_point(pos - IVec3::Y) {
                            if voxel_below != Voxel::EMPTY {
                                // draw our snow material
                                return Voxel(234);
                            }
                        }
                    }
                    // else we return the underlying voxel, unmodified
                    voxel.clone()
                },
            );
            commands.entity(snowflake).despawn();
        }
    }
}
