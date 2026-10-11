use bevy::{asset::RenderAssetUsages, prelude::*};
use bevy_vox_scene::{
    SDF, VoxBakeSettings, VoxScenePlugin, Voxel, VoxelModelAssets, VoxelModelInstance,
    VoxelPalette, VoxelRegion, VoxelRegionMode, VoxelSceneAssets,
};
use utilities::{PanOrbitCamera, PanOrbitCameraPlugin};
use vox_bake::VoxelTextureMode;

const VOXEL_SIZE: f32 = 0.1;
const BRUSH_RADIUS: i32 = 4;

#[derive(Resource, Default)]
struct VoxelStroke {
    button: Option<PointerButton>,
    entity: Option<Entity>,
    picking_proxy: Option<Entity>,
    last_voxel: Option<IVec3>,
}

#[derive(Component)]
struct VoxelPickingProxy(Entity);

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            MeshPickingPlugin,
            VoxScenePlugin::default(),
            PanOrbitCameraPlugin,
        ))
        .init_resource::<VoxelStroke>()
        .add_systems(Startup, (spawn_camera, spawn_voxels))
        .add_systems(Update, finish_voxel_stroke)
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
        VoxBakeSettings {
            voxel_size: VOXEL_SIZE,
            texture_mode: VoxelTextureMode::Palette, // makes remeshing quicker
            ..default()
        },
        Voxel(1),
    );
    let scene = voxel_assets
        .create_scene(data, palette, true, RenderAssetUsages::default()) // bevy Pickable requires RenderAssetUsages::default()
        .expect("voxel scene created");
    commands
        .spawn(WorldAssetRoot(scene))
        .observe(begin_voxel_stroke)
        .observe(continue_voxel_stroke);
}

/// Press and drag with the left button to draw voxels, or the right button to erase them.
fn begin_voxel_stroke(
    trigger: On<PointerPress>,
    query: Query<(&GlobalTransform, &Mesh3d), With<VoxelModelInstance>>,
    mut commands: Commands,
    mut cameras: Query<&mut PanOrbitCamera>,
    mut stroke: ResMut<VoxelStroke>,
    mut assets: ParamSet<(ResMut<Assets<Mesh>>, VoxelModelAssets)>,
) {
    let entity = trigger.original_event_target();
    let Ok((global_xform, mesh)) = query.get(entity) else {
        return;
    };
    let voxel_point = {
        let voxel_assets = assets.p1();
        hit_voxel_point(
            entity,
            trigger.hit.position,
            trigger.hit.normal,
            global_xform,
            &voxel_assets,
        )
    };
    let Some(voxel_point) = voxel_point else {
        return;
    };
    let snapshot = create_mesh_snapshot(mesh, &mut assets.p0());
    let Some(snapshot) = snapshot else {
        return;
    };

    stroke.button = Some(trigger.button);
    stroke.entity = Some(entity);
    stroke.last_voxel = Some(voxel_point);
    let picking_proxy = commands
        .spawn((
            snapshot,
            Transform::IDENTITY,
            Visibility::Visible,
            Pickable::default(),
            VoxelPickingProxy(entity),
        ))
        .id();
    commands.entity(entity).insert(Pickable::IGNORE);
    commands.entity(entity).add_child(picking_proxy);
    stroke.picking_proxy = Some(picking_proxy);
    for mut camera in &mut cameras {
        camera.enabled = false;
    }
    let mut voxel_assets = assets.p1();
    paint_voxel_line(
        entity,
        voxel_point,
        voxel_point,
        trigger.button,
        &mut voxel_assets,
    );
}

fn create_mesh_snapshot(mesh: &Mesh3d, meshes: &mut Assets<Mesh>) -> Option<Mesh3d> {
    let snapshot = meshes.get(&mesh.0)?.clone();
    Some(Mesh3d(meshes.add(snapshot)))
}

fn continue_voxel_stroke(
    trigger: On<PointerMove>,
    query: Query<&GlobalTransform, With<VoxelModelInstance>>,
    proxies: Query<&VoxelPickingProxy>,
    mut stroke: ResMut<VoxelStroke>,
    mut voxel_assets: VoxelModelAssets,
) {
    let Some(button) = stroke.button else {
        return;
    };
    let hit_entity = trigger.original_event_target();
    let entity = proxies
        .get(hit_entity)
        .map(|proxy| proxy.0)
        .unwrap_or(hit_entity);
    let Ok(global_xform) = query.get(entity) else {
        return;
    };
    let Some(voxel_point) = hit_voxel_point(
        entity,
        trigger.hit.position,
        trigger.hit.normal,
        global_xform,
        &voxel_assets,
    ) else {
        return;
    };
    let previous = if stroke.entity == Some(entity) {
        stroke.last_voxel.unwrap_or(voxel_point)
    } else {
        voxel_point
    };
    if stroke.entity == Some(entity) && previous == voxel_point {
        return;
    }
    paint_voxel_line(entity, previous, voxel_point, button, &mut voxel_assets);
    stroke.entity = Some(entity);
    stroke.last_voxel = Some(voxel_point);
}

fn finish_voxel_stroke(
    mut commands: Commands,
    input: Res<ButtonInput<MouseButton>>,
    mut stroke: ResMut<VoxelStroke>,
    mut cameras: Query<&mut PanOrbitCamera>,
    proxies: Query<&Mesh3d, With<VoxelPickingProxy>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let Some(button) = stroke.button else {
        return;
    };
    let released = match button {
        PointerButton::Primary => input.just_released(MouseButton::Left),
        PointerButton::Secondary => input.just_released(MouseButton::Right),
        PointerButton::Middle => input.just_released(MouseButton::Middle),
    };
    if !released {
        return;
    }
    if let Some(entity) = stroke.entity {
        commands.entity(entity).remove::<Pickable>();
    }
    if let Some(proxy) = stroke.picking_proxy {
        if let Ok(mesh) = proxies.get(proxy) {
            meshes.remove(mesh.id());
        }
        commands.entity(proxy).despawn();
    }
    *stroke = default();
    for mut camera in &mut cameras {
        camera.enabled = true;
    }
}

fn hit_voxel_point(
    entity: Entity,
    global_point: Option<Vec3>,
    global_normal: Option<Vec3>,
    global_xform: &GlobalTransform,
    voxel_assets: &VoxelModelAssets,
) -> Option<IVec3> {
    let model = voxel_assets.get_model(entity)?;
    let global_voxel_center = global_point? - (global_normal? * (VOXEL_SIZE * 0.5));
    Some(model.global_point_to_voxel_space(global_voxel_center, global_xform))
}

fn paint_voxel_line(
    entity: Entity,
    start: IVec3,
    end: IVec3,
    button: PointerButton,
    voxel_assets: &mut VoxelModelAssets,
) {
    let brush = match button {
        PointerButton::Primary => Voxel(2), // draw with the ROSE color we defined above
        PointerButton::Secondary | PointerButton::Middle => Voxel::EMPTY, // eraser
    };
    let radius = IVec3::splat(BRUSH_RADIUS);
    let min = start.min(end) - radius;
    let max = start.max(end) + radius;
    let region = VoxelRegionMode::Box(VoxelRegion {
        origin: min,
        size: max - min + IVec3::ONE,
    });
    let start = start.as_vec3();
    let end = end.as_vec3();
    let brush_radius_squared = (BRUSH_RADIUS * BRUSH_RADIUS) as f32;
    voxel_assets
        .modify(entity, region, move |pos, underlying_vox, _| {
            if point_segment_distance_squared(pos.as_vec3(), start, end) < brush_radius_squared {
                brush.clone()
            } else {
                underlying_vox.clone()
            }
        })
        .expect("canvas model is remeshable");
}

fn point_segment_distance_squared(point: Vec3, start: Vec3, end: Vec3) -> f32 {
    let segment = end - start;
    let length_squared = segment.length_squared();
    if length_squared == 0.0 {
        return point.distance_squared(start);
    }
    let along_segment = ((point - start).dot(segment) / length_squared).clamp(0.0, 1.0);
    point.distance_squared(start + segment * along_segment)
}
