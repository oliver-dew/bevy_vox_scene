use std::time::Duration;

use bevy::{
    core_pipeline::{
        bloom::Bloom,
        dof::{DepthOfField, DepthOfFieldMode},
        post_process::ChromaticAberration,
        tonemapping::Tonemapping,
    },
    prelude::*,
    scene::SceneInstanceReady,
    time::common_conditions::on_timer,
};
use bevy_vox_scene::{
    ModifyVoxelCommandsExt, VoxScenePlugin, Voxel, VoxelInstanceSpawned, VoxelModel,
    VoxelModelInstance, VoxelQueryable, VoxelRegion, VoxelRegionMode,
};
use rand::Rng;
use utilities::{PanOrbitCamera, PanOrbitCameraPlugin};

#[derive(States, Debug, Clone, Default, Hash, Eq, PartialEq)]
enum AppState {
    #[default]
    Loading,
    Ready,
}

// When a snowflake lands on the scenery, it is added to scenery's voxel data, so that snow gradually builds up
fn main() {
    // Making this frequency not cleanly divisible by the snowflake speed ensures that expensive collisions
    // don't all happen on the same frame
    let snow_spawn_freq = Duration::from_secs_f32(0.213);
    App::new()
        .add_plugins((
            DefaultPlugins,
            PanOrbitCameraPlugin,
            VoxScenePlugin::default(),
        ))
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                spawn_snow.run_if(on_timer(snow_spawn_freq)),
                update_snow,
                focus_camera,
            )
                .run_if(in_state(AppState::Ready)),
        )
        .init_state::<AppState>()
        .run();
}

#[derive(Resource)]
struct Scenes {
    snowflake: Handle<Mesh>,
    voxel_material: Handle<StandardMaterial>,
}

fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn((
        Camera3d::default(),
        Camera {
            hdr: true,
            ..default()
        },
        Transform::from_xyz(15.0, 40.0, 90.0).looking_at(Vec3::ZERO, Vec3::Y),
        Tonemapping::SomewhatBoringDisplayTransform,
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
        DepthOfField {
            mode: DepthOfFieldMode::Bokeh,
            focal_distance: 8.,
            aperture_f_stops: 0.003,
            ..default()
        },
        ChromaticAberration {
            intensity: 0.04,
            ..default()
        },
    ));
    commands.insert_resource(Scenes {
        snowflake: assets.load("study.vox#snowflake@mesh"),
        voxel_material: assets.load("study.vox#snowflake@material"),
    });

    // Scope the observer to this SceneRoot so that it doesn't run
    // againt the snowflakes when they spawn
    commands
        .spawn(
            // Load a slice of the scene
            SceneRoot(assets.load("study.vox#workstation")),
        )
        .observe(identify_scenery)
        .observe(
            |_trigger: Trigger<SceneInstanceReady>, mut app_state: ResMut<NextState<AppState>>| {
                app_state.set(AppState::Ready);
            },
        );
}

/// An observer that marks all objects in the workstation scene with the [`Scenery`] component,
/// and the center of the workstation screen with the [`FocalPoint`] component.
///
/// The advantage of using [`VoxelInstanceSpawned`] as the trigger, rather than an [`OnAdd`] trigger
/// is that because [`VoxelInstanceSpawned`] "bubbles up" through the hierarchy, you can add it as
/// an observer on a [`SceneRoot`] and scope the observer system to just that branch, rather than
/// having to use a global observer on [`OnAdd`] that might require defensive code for other branches
/// of the scene. Remember that the entity you probably want to act on is `trigger.event().entity`
/// (which will be the originator of the event), not `trigger.entity()` (the [`SceneRoot`] that the
/// observer was added to).
fn identify_scenery(trigger: Trigger<VoxelInstanceSpawned>, mut commands: Commands) {
    let Some(name) = &trigger.event().model_name else {
        return;
    };
    match name.as_str() {
        "snowflake" => panic!("This should never be executed, because this observer is scoped to the 'workstation' scene graph"),
        "workstation/computer" => {
            // Focus on the computer screen by suppling the local voxel coordinates of the center of the screen
            commands
                .entity(trigger.event().entity)
                .insert(FocalPoint(Vec3::new(0., 0., 9.)));
        }
        _ => {}
    }
    commands.entity(trigger.event().entity).insert(Scenery);
}

/// A snowflake with an angular velocity represented by a [`Quat`]
#[derive(Component)]
struct Snowflake(Quat);

/// Something solid that the snow can settle on
#[derive(Component)]
struct Scenery;

/// Focal point for the camera to focus on
#[derive(Component)]
struct FocalPoint(Vec3);

fn spawn_snow(mut commands: Commands, scenes: Res<Scenes>) {
    let mut rng = rand::thread_rng();
    let position = Vec3::new(rng.gen_range(-30.0..30.0), 80.0, rng.gen_range(-20.0..20.0)).round()
        + Vec3::splat(0.5);
    let rotation_axis =
        Vec3::new(rng.gen_range(-0.5..0.5), 1.0, rng.gen_range(-0.5..0.5)).normalize();
    let angular_velocity = Quat::from_axis_angle(rotation_axis, 0.01);
    commands.spawn((
        Name::new("snowflake"),
        Snowflake(angular_velocity),
        Mesh3d(scenes.snowflake.clone()),
        MeshMaterial3d::<StandardMaterial>(scenes.voxel_material.clone()),
        Transform::from_translation(position),
    ));
}

fn update_snow(
    mut commands: Commands,
    mut snowflakes: Query<(Entity, &Snowflake, &mut Transform), Without<Scenery>>,
    scenery: Query<(&GlobalTransform, &VoxelModelInstance), (With<Scenery>, Without<Snowflake>)>,
    models: Res<Assets<VoxelModel>>,
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
            let Some(model) = models.get(&item_instance.model) else {
                continue;
            };
            let vox_pos =
                model.global_point_to_voxel_space(snowflake_xform.translation, item_xform);
            // check whether snowflake has landed on something solid
            let pos_below_snowflake = vox_pos - IVec3::Y;
            let Ok(voxel) = model.get_voxel_at_point(pos_below_snowflake) else {
                continue;
            };
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

// Focus the camera on the focal point when the camera is first added and when it moves
fn focus_camera(
    mut camera: Query<(&mut DepthOfField, &GlobalTransform), Changed<Transform>>,
    target: Query<(&GlobalTransform, &FocalPoint)>,
) {
    let Some((target_xform, focal_point)) = target.iter().next() else {
        return;
    };
    let Ok((mut dof, camera_xform)) = camera.get_single_mut() else {
        return;
    };
    let target_point = target_xform.transform_point(focal_point.0);
    dof.focal_distance = camera_xform.translation().distance(target_point);
}
