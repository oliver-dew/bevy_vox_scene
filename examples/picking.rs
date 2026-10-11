use bevy::{asset::RenderAssetUsages, prelude::*};
use bevy_vox_scene::{
    SDF, VoxLoaderSettings, VoxScenePlugin, Voxel, VoxelModelAssets, VoxelModelSource,
    VoxelPalette, VoxelPaletteSource, VoxelQueryable, VoxelRegion, VoxelRegionMode,
    VoxelSceneAssets,
};
use utilities::{PanOrbitCamera, PanOrbitCameraPlugin};
use vox_bake::VoxelTextureMode;

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
fn spawn_voxels(mut commands: Commands, mut voxel_assets: VoxelSceneAssets) {
    let palette = voxel_assets.add_palette(VoxelPalette::from_colors(
        vec![
            bevy::color::palettes::tailwind::TEAL_600.into(), // Voxel(1)
            bevy::color::palettes::tailwind::ROSE_600.into(), // Voxel(2)
        ],
        true,
    ));
    let data = SDF::sphere(16.0).voxelize(
        UVec3::splat(48),
        VoxLoaderSettings {
            voxel_size: VOXEL_SIZE,
            retain_voxel_data: true,
            mesh_asset_usage: RenderAssetUsages::default(),
            texture_mode: VoxelTextureMode::Palette, // makes remeshing quicker
            ..default()
        },
        Voxel(1),
    );
    let scene = voxel_assets
        .create_scene(data, "canvas", palette)
        .expect("voxel scene created");
    commands.spawn(WorldAssetRoot(scene)).observe(on_tap_voxels);
}

/// Tap with left button to add voxels, right button to erase
fn on_tap_voxels(
    trigger: On<PointerPress>,
    query: Query<(
        &VoxelModelSource,
        &VoxelPaletteSource,
        &Mesh3d,
        &GlobalTransform,
    )>,
    mut voxel_assets: VoxelModelAssets,
) {
    let Some(global_point) = trigger.hit.position else {
        return;
    };
    let Some(global_normal) = trigger.hit.normal else {
        return;
    };
    let Ok((source, palette_source, mesh, global_xform)) =
        query.get(trigger.original_event_target())
    else {
        return;
    };
    let Some(model) = voxel_assets.get_model(source) else {
        return;
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
    let region = VoxelRegionMode::Box(VoxelRegion::from_center_and_half_extent(
        voxel_point,
        IVec3::splat(brush_radius),
    ));
    voxel_assets
        .modify(
            source,
            palette_source,
            Some(mesh),
            region,
            move |pos, underlying_vox, _| {
                if pos.distance_squared(voxel_point) < brush_radius_squared {
                    brush.clone()
                } else {
                    underlying_vox.clone()
                }
            },
        )
        .expect("canvas model is remeshable");
}
