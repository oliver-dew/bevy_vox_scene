use bevy::{
    core_pipeline::{
        bloom::Bloom,
        experimental::taa::{TemporalAntiAliasPlugin, TemporalAntiAliasing},
        tonemapping::Tonemapping,
    },
    pbr::{FogVolume, VolumetricFog, VolumetricLight},
    prelude::*,
};
use bevy_vox_scene::{VoxScenePlugin, VoxelInstanceReady};
use utilities::{PanOrbitCamera, PanOrbitCameraPlugin};

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            TemporalAntiAliasPlugin,
            VoxScenePlugin::default(),
            PanOrbitCameraPlugin,
        ))
        .register_type::<FogVolume>()
        .add_systems(Startup, setup)
        .add_systems(Update, scroll_fog)
        .run();
}

fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn((
        Camera {
            hdr: true,
            ..default()
        },
        Camera3d::default(),
        Transform::from_xyz(-40., 4.5, 16.).looking_to(
            Dir3::new_unchecked(Vec3::new(0.873, 0.288, -0.393).normalize()),
            Vec3::Y,
        ),
        PanOrbitCamera {
            radius: 60.0,
            ..default()
        },
        Bloom {
            intensity: 0.3,
            ..default()
        },
        Tonemapping::BlenderFilmic,
        Msaa::Off,
        TemporalAntiAliasing::default(),
        EnvironmentMapLight {
            diffuse_map: assets.load("pisa_diffuse.ktx2"),
            specular_map: assets.load("pisa_specular.ktx2"),
            intensity: 500.0,
            ..default()
        },
        VolumetricFog {
            ambient_intensity: 0.0,
            jitter: 0.5,
            ..default()
        },
    ));

    commands.spawn((
        DirectionalLight {
            illuminance: 3000.0,
            shadows_enabled: true,
            ..Default::default()
        },
        Transform::IDENTITY.looking_to(Vec3::new(-2.5, -1., 0.85), Vec3::Y),
        VolumetricLight,
    ));

    commands
        .spawn(SceneRoot(assets.load("cloud.vox")))
        .observe(add_point_lights);
}

// replace "point_light" marker models with point lights
fn add_point_lights(trigger: Trigger<VoxelInstanceReady>, mut commands: Commands) {
    let Some(name) = &trigger.event().model_name else {
        return;
    };
    if name.contains("point_light") {
        commands
            .entity(trigger.event().instance)
            .remove::<Mesh3d>()
            .remove::<MeshMaterial3d<StandardMaterial>>()
            .insert((
                PointLight {
                    color: Color::linear_rgb(251. / 255., 226. / 255., 81. / 255.),
                    intensity: 10000.,
                    radius: 0.5,
                    range: 150.,
                    shadows_enabled: true,
                    ..default()
                },
                VolumetricLight,
                Visibility::Visible,
            ));
    }
}

/// Moves fog density texture offset every frame.
fn scroll_fog(time: Res<Time>, mut query: Query<&mut FogVolume>) {
    for mut fog_volume in query.iter_mut() {
        fog_volume.density_texture_offset += Vec3::new(0.01, -0.04, 0.02) * time.delta_secs();
    }
}
