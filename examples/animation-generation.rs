use bevy::{
    anti_alias::taa::TemporalAntiAliasing, camera::ScreenSpaceTransmissionQuality,
    core_pipeline::tonemapping::Tonemapping, post_process::bloom::Bloom, prelude::*,
    scene::SceneInstanceReady,
};
use bevy_vox_scene::{
    SDF, VoxLoaderSettings, VoxScenePlugin, Voxel, VoxelContext, VoxelData, VoxelModelInstance,
    create_voxel_animation,
};
use utilities::{PanOrbitCamera, PanOrbitCameraPlugin};

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            PanOrbitCameraPlugin,
            VoxScenePlugin::default(),
        ))
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn((
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
            |trigger: On<SceneInstanceReady>,
             children: Query<&Children>,
             vox_instance: Query<&VoxelModelInstance>,
             mut commands: Commands| {
                for child in children.iter_descendants(trigger.entity) {
                    if let Ok(instance) = vox_instance.get(child) {
                        // we need to wait until `SceneInstanceReady` so that the animation we generate can use the same `VoxelContext` as the scene loaded from disk
                        commands.run_system_cached_with(generate_ripples, instance.context.clone());
                        break;
                    }
                }
            },
        );
}

/// Spawn a 10 frame animation of concentric circles moving outwards
fn generate_ripples(In(context): In<Handle<VoxelContext>>, world: &mut World) {
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
            (models, "ripples".to_string(), context),
        )
        .expect("animation created");
    world.spawn((
        SceneRoot(scene_root),
        Transform::from_xyz(0., 7., 0.), // position the ripples on the surface of the water
    ));
}
