use bevy::{
    animation::AnimatedBy,
    anti_alias::taa::TemporalAntiAliasing,
    app::AnimationSystems,
    camera::Exposure,
    input::keyboard::KeyboardInput,
    light::{
        Atmosphere, AtmosphereEnvironmentMapLight, CascadeShadowConfigBuilder,
        DirectionalLightShadowMap, ShadowFilteringMethod, atmosphere::ScatteringMedium,
        light_consts::lux,
    },
    pbr::AtmosphereSettings,
    post_process::{
        bloom::Bloom,
        dof::{DepthOfField, DepthOfFieldMode},
    },
    prelude::*,
    render::view::Tonemapping,
    transform::TransformSystems,
};
use bevy_vox_scene::{VoxLoaderSettings, VoxScenePlugin, VoxelInstanceReady};
use flipbook::FlipbookPlayer;
use vox_bake::{AmbientOcclusionSettings, TexturePackSettings, VoxelTextureMode};

const VOXEL_SIZE: f32 = 0.1;
const TARGET_DIST: f32 = 550. * VOXEL_SIZE;
fn main() {
    App::new()
        .insert_resource(GlobalAmbientLight::NONE)
        .insert_resource(DirectionalLightShadowMap { size: 4096 })
        .add_plugins((
            DefaultPlugins,
            VoxScenePlugin {
                global_settings: Some(VoxLoaderSettings {
                    voxel_size: VOXEL_SIZE,
                    diffuse_roughness: 0.95,
                    texture_mode: VoxelTextureMode::Packed(TexturePackSettings {
                        ambient_occlusion: Some(AmbientOcclusionSettings {
                            depth: 3,
                            ..default()
                        }),
                        ..default()
                    }),
                    ..default()
                }),
            },
        ))
        .add_systems(Startup, setup)
        .add_systems(Update, toggle_pause.run_if(on_message::<KeyboardInput>))
        .add_systems(
            PostUpdate,
            follow_cam
                .after(AnimationSystems)
                .before(TransformSystems::Propagate),
        )
        .run();
}

fn setup(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut scattering_mediums: ResMut<Assets<ScatteringMedium>>,
) {
    commands.spawn((
        Camera3d::default(),
        ShadowFilteringMethod::Hardware2x2,
        // Because `OrthographicProjection` doesn't work well with `Atmosphere`,
        // we are instead using a long lens, 200m super 35 equivalent
        Projection::Perspective(PerspectiveProjection {
            fov: 4.47_f32.to_radians(),
            ..default()
        }),
        AtmosphereSettings::default(),
        Exposure { ev100: 13.0 },
        Bloom::NATURAL,
        AtmosphereEnvironmentMapLight::default(),
        DepthOfField {
            mode: DepthOfFieldMode::Gaussian,
            focal_distance: TARGET_DIST,
            aperture_f_stops: 0.2,
            ..default()
        },
        Msaa::Off,
        TemporalAntiAliasing::default(),
        Tonemapping::AcesFitted,
    ));

    commands.spawn(Atmosphere::earth(
        scattering_mediums.add(ScatteringMedium::default()),
    ));

    // Sun
    commands.spawn((
        DirectionalLight {
            shadow_maps_enabled: true,
            illuminance: lux::RAW_SUNLIGHT,
            ..default()
        },
        Transform::from_xyz(-0.2, 0.6, 1.0).looking_at(Vec3::ZERO, Vec3::Y),
        // Concentrate the shadow texels on this narrow, shallow long-lens view.
        CascadeShadowConfigBuilder {
            num_cascades: 1,
            minimum_distance: 0.05 * VOXEL_SIZE,
            maximum_distance: 1200.0 * VOXEL_SIZE,
            ..default()
        }
        .build(),
    ));

    commands
        .spawn(WorldAssetRoot(assets.load("deer.vox")))
        .observe(play_deer_transform);
}

fn play_deer_transform(
    trigger: On<VoxelInstanceReady>,
    animated_by: Query<&AnimatedBy>,
    mut players: Query<(&mut AnimationPlayer, &AnimationGraphHandle)>,
    graphs: Res<Assets<AnimationGraph>>,
    mut commands: Commands,
) {
    if trigger.model_name.as_deref() != Some("deer") {
        return;
    }
    // add marker component to scope pause action
    commands.entity(trigger.instance).insert(Deer);
    let Ok(AnimatedBy(animated_by)) = animated_by.get(trigger.instance) else {
        return;
    };
    let Ok((mut player, graph_handle)) = players.get_mut(*animated_by) else {
        return;
    };
    let Some(graph) = graphs.get(graph_handle.id()) else {
        return;
    };

    let Some(animation_node) = graph
        .graph
        .node_indices()
        .find(|node| matches!(graph.graph[*node].node_type, AnimationNodeType::Clip(_)))
    else {
        return;
    };

    player.play(animation_node).repeat();
}

/// Press any key to toggle the pause state of the animation
fn toggle_pause(
    keys: Res<ButtonInput<KeyCode>>,
    mut query: Query<(&mut FlipbookPlayer, &AnimatedBy), With<Deer>>,
    mut players: Query<&mut AnimationPlayer>,
) {
    let Ok((mut flipbook, AnimatedBy(animated_by))) = query.single_mut() else {
        return;
    };
    let Ok(mut animation_player) = players.get_mut(*animated_by) else {
        return;
    };
    if keys.get_just_pressed().next().is_some() {
        flipbook.is_paused = !flipbook.is_paused;
        if animation_player.all_paused() {
            animation_player.resume_all();
        } else {
            animation_player.pause_all();
        }
    }
}

fn follow_cam(
    target: Single<&Transform, With<Deer>>,
    camera: Single<&mut Transform, (With<Camera3d>, Without<Deer>)>,
) {
    let mut camera_xform = camera.into_inner();
    let target = target.translation - Vec3::Y * 8. * VOXEL_SIZE;
    camera_xform.translation = target + Vec3::new(1., 1., -1.).clamp_length_min(TARGET_DIST);
    *camera_xform = camera_xform.looking_at(target, Vec3::Y);
}

#[derive(Component)]
struct Deer;
