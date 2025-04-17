use bevy::{
    core_pipeline::{
        bloom::Bloom,
        core_3d::ScreenSpaceTransmissionQuality,
        experimental::taa::{TemporalAntiAliasPlugin, TemporalAntiAliasing},
        tonemapping::Tonemapping,
    },
    prelude::*,
    scene::SceneInstanceReady,
};
use bevy_vox_scene::{
    create_voxel_animation, VoxLoaderSettings, VoxScenePlugin, Voxel, VoxelContext, VoxelData, SDF,
};
use utilities::{PanOrbitCamera, PanOrbitCameraPlugin};

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            TemporalAntiAliasPlugin,
            PanOrbitCameraPlugin,
            VoxScenePlugin::default(),
        ))
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn((
        Camera {
            hdr: true,
            ..default()
        },
        Camera3d {
            screen_space_specular_transmission_quality: ScreenSpaceTransmissionQuality::High,
            screen_space_specular_transmission_steps: 1,
            ..default()
        },
        Bloom {
            intensity: 0.3,
            ..default()
        },
        TemporalAntiAliasing::default(),
        Msaa::Off,
        Tonemapping::BlenderFilmic,
        Transform::from_xyz(30.0, 30.0, 60.0).looking_at(Vec3::ZERO, Vec3::Y),
        PanOrbitCamera::default(),
        EnvironmentMapLight {
            diffuse_map: assets.load("pisa_diffuse.ktx2"),
            specular_map: assets.load("pisa_specular.ktx2"),
            intensity: 500.0,
            ..default()
        },
    ));

    commands
        .spawn(SceneRoot(assets.load("study.vox#tank")))
        .observe(
            |_trigger: Trigger<SceneInstanceReady>, mut commands: Commands| {
                // we need to wait until `SceneInstanceReady` so that the animation we generate can use the same `VoxelContext` as the scene loaded from disk
                commands.run_system_cached(generate_ripples);
            },
        );
}

/// Spawn a 10 frame animation of concentric circles moving outwards
fn generate_ripples(world: &mut World) {
    let (id, _) = world
        .get_resource::<Assets<VoxelContext>>()
        .expect("Voxel context has been loaded")
        .iter()
        .next()
        .expect("Only one context present");
    let context = Handle::Weak(id);
    let frequency = 10.0;
    let frame_count = frequency as usize;
    let blacklight_radius = 3.0;
    let ripple_centre = Vec3::new(30.0, 0.0, 20.0);
    let models: Vec<VoxelData> = (0..frame_count)
        .map(|frame_index| {
            SDF::new(move |pos| {
                let mut pos2d = pos - ripple_centre;
                pos2d.y = 0.0;
                (((pos2d.length() - frame_index as f32) % frequency) - blacklight_radius).abs()
            })
            .map_to_voxels(
                UVec3::new(70, 2, 50),
                VoxLoaderSettings::default(),
                |distance, pos| {
                    if distance.powf(3.0) < (0.5 - pos.y) * 0.5 {
                        Voxel(192) // water material
                    } else {
                        Voxel::EMPTY
                    }
                },
            )
        })
        .collect();
    let scene_root = world
        .run_system_cached_with(
            create_voxel_animation,
            (models, "ripples".to_string(), context.clone_weak()),
        )
        .expect("animation created");
    world.spawn((
        SceneRoot(scene_root),
        Transform::from_xyz(0., 7., 0.), // position the ripples on the surface of the water
    ));
}
