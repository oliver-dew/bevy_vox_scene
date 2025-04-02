use bevy::{
    core_pipeline::{
        bloom::Bloom,
        core_3d::ScreenSpaceTransmissionQuality,
        experimental::taa::{TemporalAntiAliasPlugin, TemporalAntiAliasing},
        tonemapping::Tonemapping,
    },
    input::keyboard::KeyboardInput,
    prelude::*,
};
use bevy_vox_scene::VoxScenePlugin;
use rand::Rng;
use std::f32::consts::PI;
use utilities::{PanOrbitCamera, PanOrbitCameraPlugin};

/// Uses an observer triggered by `VoxelModelInstance` being added to add extra components into the scene graph.
/// Press any key to toggle the fish tank black-light on and off
fn main() {
    let mut app = App::new();

    app.add_plugins((
        DefaultPlugins,
        PanOrbitCameraPlugin,
        VoxScenePlugin::default(),
    ))
    .add_systems(Startup, setup)
    .add_systems(
        Update,
        (
            toggle_black_light.run_if(on_event::<KeyboardInput>),
            swim_fish,
        ),
    );

    // *Note:* TAA is not _required_ for specular transmission, but
    // it _greatly enhances_ the look of the resulting blur effects.
    // Sadly, it's not available under WebGL.
    #[cfg(not(all(feature = "webgl2", target_arch = "wasm32")))]
    app.add_plugins(TemporalAntiAliasPlugin);

    app.run();
}

// Systems

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
        Transform::from_xyz(0.0, 1.5, 8.0).looking_at(Vec3::ZERO, Vec3::Y),
        Tonemapping::SomewhatBoringDisplayTransform,
        PanOrbitCamera::default(),
        Bloom {
            intensity: 0.3,
            ..default()
        },
        #[cfg(not(all(feature = "webgl2", target_arch = "wasm32")))]
        TemporalAntiAliasing::default(),
        #[cfg(not(all(feature = "webgl2", target_arch = "wasm32")))]
        Msaa::Off,
        EnvironmentMapLight {
            diffuse_map: assets.load("pisa_diffuse.ktx2"),
            specular_map: assets.load("pisa_specular.ktx2"),
            intensity: 500.0,
            ..default()
        },
    ));
    commands.spawn((
        // "tank" is the name of the group containing the glass walls, the body of water, the scenery in the tank and the fish
        SceneRoot(assets.load("study.vox#tank")),
        Transform::from_scale(Vec3::splat(0.05)),
    ));
    commands.add_observer(on_spawn_voxel_instance);
}

// Will run against every named child entity that gets spawned in the scene
fn on_spawn_voxel_instance(
    trigger: Trigger<OnAdd, Name>,
    model_query: Query<&Name>,
    mut commands: Commands,
    assets: Res<AssetServer>,
) {
    let mut entity_commands = commands.entity(trigger.target());
    let name = model_query.get(trigger.target()).unwrap().as_str();
    match name {
        "tank/black-light" => {
            entity_commands.insert(EmissiveToggle {
                is_on: true,
                on_material: assets.load("study.vox#material"), // emissive texture
                off_material: assets.load("study.vox#material-no-emission"), // non-emissive texture
            });
        }
        "tank/goldfish" | "tank/tetra" => {
            // Make fish go brrrrr
            let mut rng = rand::rng(); // random speed
            entity_commands.insert(Fish(rng.random_range(5.0..10.0)));
        }
        _ => {}
    }
}

fn toggle_black_light(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut query: Query<(Entity, &mut EmissiveToggle)>,
) {
    if keys.get_just_pressed().next().is_none() {
        return;
    };
    let Ok((entity, mut emissive_toggle)) = query.single_mut() else {
        return;
    };
    emissive_toggle.toggle();
    commands
        .entity(entity)
        .insert(MeshMaterial3d(emissive_toggle.material().clone()));
}

fn swim_fish(mut query: Query<(&mut Transform, &Fish)>, time: Res<Time>) {
    let tank_half_extents = Vec3::new(29.0, 20.0, 25.0);
    for (mut transform, fish) in query.iter_mut() {
        let x_direction = transform.forward().dot(Vec3::X);
        if (x_direction < -0.5 && transform.translation.x < -tank_half_extents.x)
            || (x_direction > 0.5 && transform.translation.x > tank_half_extents.x)
        {
            // change direction at tank edges
            transform.rotate_axis(Dir3::Y, PI);
        }
        // slow down when near the edge
        let slow_down = 1.0
            - ((transform.translation.x.abs() - (tank_half_extents.x - 4.2)) / 5.0).clamp(0.0, 1.0);
        let forward = transform.forward();
        transform.translation += forward.as_vec3() * (time.delta_secs() * fish.0 * slow_down);
        // make them weave up and down
        transform.translation.y = (transform.translation.x * 0.1).sin() * 6.0;
    }
}

// Components

#[derive(Component)]
struct EmissiveToggle {
    is_on: bool,
    on_material: Handle<StandardMaterial>,
    off_material: Handle<StandardMaterial>,
}

impl EmissiveToggle {
    fn toggle(&mut self) {
        self.is_on = !self.is_on;
    }

    fn material(&self) -> &Handle<StandardMaterial> {
        match self.is_on {
            true => &self.on_material,
            false => &self.off_material,
        }
    }
}

#[derive(Component)]
struct Fish(f32);
