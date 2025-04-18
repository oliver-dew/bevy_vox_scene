use bevy::prelude::*;
use bevy_vox_scene::{
    SDF, VoxLoaderSettings, VoxScenePlugin, Voxel, VoxelModel, VoxelModelInstance, VoxelModifier,
    VoxelPalette, VoxelQueryable, VoxelRegion, VoxelRegionMode, create_voxel_context,
    create_voxel_scene, modify_voxel_model,
};
use utilities::{PanOrbitCamera, PanOrbitCameraPlugin};

const VOXEL_SIZE: f32 = 0.1;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            MeshPickingPlugin,
            VoxScenePlugin::default(),
            PanOrbitCameraPlugin,
        ))
        .add_systems(Startup, (spawn_camera, spawn_voxels))
        .run();
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        PanOrbitCamera::default(),
        MeshPickingCamera,
        Transform::from_xyz(2.0, 3.0, 6.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    commands.spawn((
        DirectionalLight::default(),
        Transform::IDENTITY.looking_to(Vec3::new(2.5, -1., 0.85), Vec3::Y),
    ));
}

/// Spawn a voxel sphere that will act as a canvas to draw on
fn spawn_voxels(world: &mut World) {
    let palette = VoxelPalette::from_colors(
        vec![
            bevy::color::palettes::tailwind::TEAL_600.into(), // Voxel(1)
            bevy::color::palettes::tailwind::ROSE_600.into(), // Voxel(2)
        ],
        true,
    );
    let context = world
        .run_system_cached_with(create_voxel_context, palette)
        .unwrap();
    let data = SDF::sphere(16.0).voxelize(
        UVec3::splat(48),
        VoxLoaderSettings {
            voxel_size: VOXEL_SIZE,
            supports_remeshing: true, // we need to set this in order to update the mesh below
            ..default()
        },
        Voxel(1),
    );
    let scene = world
        .run_system_cached_with(create_voxel_scene, (data, format!("canvas"), context))
        .unwrap();
    world
        .spawn(SceneRoot(scene))
        .observe(on_tap_voxels.pipe(modify_voxel_model));
}

/// Tap with left button to add voxels, right button to erase
fn on_tap_voxels(
    trigger: Trigger<Pointer<Pressed>>,
    query: Query<(&VoxelModelInstance, &Mesh3d, &GlobalTransform)>,
    models: Res<Assets<VoxelModel>>,
) -> Option<VoxelModifier> {
    let Some(global_point) = trigger.hit.position else {
        return None;
    };
    let Some(global_normal) = trigger.hit.normal else {
        return None;
    };
    let Ok((instance, mesh, global_xform)) = query.get(trigger.target) else {
        return None;
    };
    let Some(model) = models.get(&instance.model) else {
        return None;
    };
    // pick a point close to the center of the voxel whose surface we tapped:
    let global_voxel_center = global_point - (global_normal * (VOXEL_SIZE * 0.5));
    // and convert it into voxel space:
    let voxel_point = model.global_point_to_voxel_space(global_voxel_center, global_xform);
    let brush_radius = 4;
    let brush_radius_squared = brush_radius * brush_radius;
    let brush: Voxel = match trigger.button {
        PointerButton::Primary => Voxel(2), // draw with the ROSE color we defined above
        PointerButton::Secondary | PointerButton::Middle => Voxel::EMPTY, // eraser
    };
    // define a region to update that is no bigger than our brush to keep the operation performant
    let region = VoxelRegionMode::Box(VoxelRegion::from_center(
        voxel_point,
        IVec3::splat(brush_radius),
    ));
    let modifier = VoxelModifier::new(
        instance.clone(),
        mesh.0.clone(),
        region,
        move |pos, underlying_vox, _| {
            if pos.distance_squared(voxel_point) < brush_radius_squared {
                brush.clone()
            } else {
                underlying_vox.clone()
            }
        },
    );
    Some(modifier)
}
