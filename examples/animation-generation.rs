use bevy::{
    anti_alias::taa::TemporalAntiAliasing,
    core_pipeline::tonemapping::Tonemapping,
    pbr::{ScreenSpaceTransmission, ScreenSpaceTransmissionQuality},
    post_process::bloom::Bloom,
    prelude::*,
    world_serialization::WorldInstanceReady,
};
use bevy_vox_scene::{
    SDF, VoxLoaderSettings, VoxScenePlugin, Voxel, VoxelData, VoxelModelSource,
    VoxelModelSourcePolicy, VoxelPalette, VoxelSceneAssets,
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
        Camera3d::default(),
        ScreenSpaceTransmission {
            steps: 1,
            quality: ScreenSpaceTransmissionQuality::High,
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
        .spawn(WorldAssetRoot(
            assets
                .load_builder()
                .with_settings(|settings: &mut VoxLoaderSettings| {
                    settings.model_source = VoxelModelSourcePolicy::Queryable;
                })
                .load("study.vox#tank"),
        ))
        .observe(
            |trigger: On<WorldInstanceReady>,
             children: Query<&Children>,
             voxel_source: Query<&VoxelModelSource>,
             mut commands: Commands,
             mut voxel_assets: VoxelSceneAssets| {
                for child in children.iter_descendants(trigger.entity) {
                    if let Ok(source) = voxel_source.get(child) {
                        // we need to wait until `WorldInstanceReady` so that the animation we generate can use the same palette as the scene loaded from disk
                        generate_ripples(
                            source.palette().clone(),
                            &mut commands,
                            &mut voxel_assets,
                        );
                        break;
                    }
                }
            },
        );
}

/// Spawn a 10 frame animation of concentric circles moving outwards
fn generate_ripples(
    palette: Handle<VoxelPalette>,
    commands: &mut Commands,
    voxel_assets: &mut VoxelSceneAssets,
) {
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
    let scene_root = voxel_assets.create_animation(models, "ripples", palette);
    commands.spawn((
        WorldAssetRoot(scene_root),
        Transform::from_xyz(0., 7., 0.), // position the ripples on the surface of the water
    ));
}
