use bevy::{
    core_pipeline::{bloom::BloomSettings, tonemapping::Tonemapping},
    prelude::*,
    time::common_conditions::on_timer,
};
use bevy_vox_scene::{
    ModifyVoxelCommandsExt, VoxScenePlugin, Voxel, VoxelModelInstance, VoxelRegion,
    VoxelRegionMode, VoxelSceneBundle,
};
use rand::Rng;
use std::{ops::RangeInclusive, time::Duration};
use utilities::{PanOrbitCamera, PanOrbitCameraPlugin};

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            PanOrbitCameraPlugin,
            VoxScenePlugin::default(),
        ))
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            grow_grass.run_if(on_timer(Duration::from_secs_f32(0.1))),
        )
        .run();
}

#[derive(Component)]
struct Floor;

fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn((
        Camera3dBundle {
            camera: Camera {
                hdr: true,
                ..Default::default()
            },
            transform: Transform::from_xyz(8.0, 1.5, 8.0).looking_at(Vec3::ZERO, Vec3::Y),
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
            intensity: 500.0,
        },
    ));

    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight {
            illuminance: 5000.0,
            shadows_enabled: true,
            ..Default::default()
        },
        transform: Transform::IDENTITY.looking_to(Vec3::new(1.0, -2.5, 0.85), Vec3::Y),
        ..default()
    });

    commands.spawn(VoxelSceneBundle {
        scene: assets.load("study.vox"),
        transform: Transform::from_scale(Vec3::splat(0.05)),
        ..default()
    });
    commands.observe(on_spawn_voxel_instance);
}

fn on_spawn_voxel_instance(
    trigger: Trigger<OnAdd, VoxelModelInstance>,
    model_query: Query<&VoxelModelInstance>,
    mut commands: Commands,
) {
    let name = model_query
        .get(trigger.entity())
        .unwrap()
        .model_name
        .as_str();
    if name == "floor" {
        commands.entity(trigger.entity()).insert(Floor);
    }
}

fn grow_grass(mut commands: Commands, query: Query<&VoxelModelInstance, With<Floor>>) {
    // All the floor tiles are instances of the same model, so we only need one instance
    let Some(instance) = query.iter().next() else {
        return;
    };
    let region = VoxelRegion {
        origin: IVec3::new(0, 4, 0),
        size: IVec3::new(64, 8, 64),
    };
    commands.modify_voxel_model(
        instance.clone(),
        VoxelRegionMode::Box(region),
        |pos, voxel, model| {
            if *voxel != Voxel::EMPTY {
                // don't overwrite any voxels
                return voxel.clone();
            };
            let mut rng = rand::thread_rng();
            let value: u16 = rng.gen_range(0..5000);
            if value > 20 {
                return Voxel::EMPTY;
            };
            let vox_below = model
                .get_voxel_at_point(pos - IVec3::Y)
                .unwrap_or(Voxel::EMPTY);
            let grass_voxels: RangeInclusive<u8> = 161..=165;
            let grow_grass = grass_voxels.contains(&vox_below.0);
            let mut plant_grass = !grow_grass && value < 5 && vox_below != Voxel::EMPTY;
            if plant_grass {
                // poisson disk effect: don't plant grass if too near other blades
                'check_neighbors: for direction in [IVec3::NEG_X, IVec3::X, IVec3::NEG_Z, IVec3::Z]
                {
                    let neighbor = model
                        .get_voxel_at_point(pos + direction)
                        .unwrap_or(Voxel::EMPTY);
                    if grass_voxels.contains(&neighbor.0) {
                        plant_grass = false;
                        break 'check_neighbors;
                    }
                }
            }
            if plant_grass || grow_grass {
                Voxel((161 + value % 5) as u8)
            } else {
                Voxel::EMPTY
            }
        },
    );
}
