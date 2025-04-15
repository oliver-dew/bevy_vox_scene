use bevy::{
    core_pipeline::{bloom::Bloom, tonemapping::Tonemapping},
    prelude::*,
    time::common_conditions::on_timer,
};
use bevy_vox_scene::{
    modify_voxel_model, VoxScenePlugin, Voxel, VoxelModelInstance, VoxelModifier, VoxelRegion,
    VoxelRegionMode,
};
use rand::Rng;
use std::{ops::RangeInclusive, time::Duration};
use utilities::{PanOrbitCamera, PanOrbitCameraPlugin};

//TODO fix
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
            grow_grass
                .pipe(modify_voxel_model)
                .run_if(on_timer(Duration::from_secs_f32(0.1))),
        )
        .run();
}

#[derive(Component)]
struct Floor;

fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    commands.add_observer(on_spawn_voxel_instance);
    commands.spawn((
        Camera3d::default(),
        Camera {
            hdr: true,
            ..default()
        },
        Transform::from_xyz(8.0, 1.5, 8.0).looking_at(Vec3::ZERO, Vec3::Y),
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
        Name::new("camera"),
    ));

    commands.spawn((
        DirectionalLight {
            illuminance: 5000.0,
            shadows_enabled: true,
            ..Default::default()
        },
        Transform::IDENTITY.looking_to(Vec3::new(1.0, -2.5, 0.85), Vec3::Y),
        Name::new("light"),
    ));

    commands.spawn((
        SceneRoot(assets.load("study.vox")),
        Transform::from_scale(Vec3::splat(0.05)),
    ));
}

fn on_spawn_voxel_instance(
    trigger: Trigger<OnAdd, Name>,
    model_query: Query<&Name>,
    mut commands: Commands,
) {
    let Ok(name) = model_query.get(trigger.target()).map(|n| n.as_str()) else {
        return;
    };
    if name == "floor" {
        commands.entity(trigger.target()).insert(Floor);
    }
}

fn grow_grass(query: Query<(&VoxelModelInstance, &Mesh3d), With<Floor>>) -> Option<VoxelModifier> {
    // All the floor tiles are instances of the same model, so we only need one instance
    let Some((instance, mesh)) = query.iter().next() else {
        return None;
    };
    let region = VoxelRegion {
        origin: IVec3::new(0, 4, 0),
        size: IVec3::new(64, 8, 64),
    };
    Some(VoxelModifier::new(
        instance.clone(),
        mesh.0.clone(),
        VoxelRegionMode::Box(region),
        |pos, voxel, model| {
            if *voxel != Voxel::EMPTY {
                // don't overwrite any voxels
                return voxel.clone();
            };
            let mut rng = rand::rng();
            let value: u16 = rng.random_range(0..5000);
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
    ))
}
