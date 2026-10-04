// Example that loads the same model as `animation-scene.rs`, except converted offline into a glb.
// Has no dependency on bevy_vox_scene
// Command used to convert model:
// vox-to-gltf assets/deer.vox --ao-depth 3 --voxel-size 0.1 -o flipbook/assets/deer.glb
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
    world_serialization::WorldInstanceReady,
};
use flipbook::{FlipbookPlayer, FlipbookPlugin, on_flipbook_ready};

const VOXEL_SIZE: f32 = 0.1;
const TARGET_DIST: f32 = 550. * VOXEL_SIZE;

#[derive(Component)]
struct AutoplayGltf(Handle<Gltf>);

fn main() {
    App::new()
        .insert_resource(GlobalAmbientLight::NONE)
        .insert_resource(DirectionalLightShadowMap { size: 4096 })
        .add_plugins((DefaultPlugins, FlipbookPlugin))
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

    let path = "deer.glb";
    let gltf = assets.load(path);
    commands
        .spawn((
            WorldAssetRoot(assets.load(GltfAssetLabel::Scene(0).from_asset(path))),
            AutoplayGltf(gltf),
        ))
        .observe(autoplay_gltf)
        .observe(on_flipbook_ready)
        .observe(tag_deer);
}

fn autoplay_gltf(
    ready: On<WorldInstanceReady>,
    autoplay: Query<&AutoplayGltf>,
    gltfs: Res<Assets<Gltf>>,
    children: Query<&Children>,
    mut players: Query<&mut AnimationPlayer>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut commands: Commands,
) {
    let Ok(autoplay) = autoplay.get(ready.entity) else {
        return;
    };
    let Some(gltf) = gltfs.get(&autoplay.0) else {
        return;
    };

    let (graph, nodes) = AnimationGraph::from_clips(gltf.animations.iter().cloned());
    if nodes.is_empty() {
        return;
    };
    let graph = graphs.add(graph);

    for descendant in children.iter_descendants(ready.entity) {
        let Ok(mut player) = players.get_mut(descendant) else {
            continue;
        };
        player.play(nodes[0]).repeat();

        commands
            .entity(descendant)
            .insert(AnimationGraphHandle(graph.clone()));
    }
}

fn tag_deer(
    ready: On<WorldInstanceReady>,
    children: Query<&Children>,
    query: Query<&Name>,
    mut commands: Commands,
) {
    for child in children.iter_descendants(ready.entity) {
        let Ok(name) = query.get(child) else {
            continue;
        };
        if name.as_str() != "deer" {
            continue;
        };
        // add marker component to scope pause action
        commands.entity(child).insert(Deer);
    }
}

/// Press any key to toggle the pause state of the animation
fn toggle_pause(
    keys: Res<ButtonInput<KeyCode>>,
    query: Single<(&mut FlipbookPlayer, &AnimatedBy), With<Deer>>,
    mut players: Query<&mut AnimationPlayer>,
) {
    let (mut flipbook, AnimatedBy(animated_by)) = query.into_inner();
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
