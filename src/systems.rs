use bevy::{
    prelude::{Children, Commands, Entity, Query, Res, Visibility},
    time::Time,
};

use crate::{
    VoxelAnimationPlayer,
    load::{AnimationUpdate, VoxelAnimationFrame},
};

pub(super) fn update_animations(
    mut commands: Commands,
    mut animation_query: Query<(Entity, &mut VoxelAnimationPlayer, &Children)>,
    mut frame_query: Query<(&VoxelAnimationFrame, &mut Visibility)>,
    time: Res<Time>,
) {
    for (entity, mut animation, children) in animation_query.iter_mut() {
        let update = animation.did_advance_frame(time.delta());
        match update {
            AnimationUpdate::SameFrame => (),
            AnimationUpdate::AdvanceFrame(new_frame) => {
                for child in children {
                    let Ok((frame, mut visibility)) = frame_query.get_mut(*child) else {
                        continue;
                    };
                    *visibility = if frame.0 == new_frame {
                        Visibility::Inherited
                    } else {
                        Visibility::Hidden
                    };
                }
            }
            AnimationUpdate::ReachedEnd => {
                if animation.despawn_on_finish {
                    commands.entity(entity).despawn();
                }
            }
        };
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use bevy::{
        app::App,
        ecs::hierarchy::Children,
        prelude::{Add, Commands, On, Visibility},
        utils::default,
        world_serialization::WorldAssetRoot,
    };

    use crate::{VoxelAnimationPlayer, test_support::setup_and_load_voxel_scene};

    #[async_std::test]
    async fn test_spawn_play_animation() {
        let frame_count: usize = 4;
        let mut app = App::new();
        let handle = setup_and_load_voxel_scene(&mut app, "deer.vox").await;
        app.update();
        // Use an observer to override the default `VoxelAnimationPlayer` with one that has a very fast `frame_rate`
        // so we can advance a frame on each call to `app.update`
        app.add_observer(
            move |trigger: On<Add, VoxelAnimationPlayer>, mut commands: Commands| {
                commands
                    .entity(trigger.entity)
                    .insert(VoxelAnimationPlayer {
                        frames: (0..frame_count).collect(),
                        frame_rate: Duration::from_millis(1),
                        ..default()
                    });
            },
        );
        let scene_root = app.world_mut().spawn(WorldAssetRoot(handle)).id();
        app.update();
        app.update();
        let top_entity = app
            .world()
            .get::<Children>(scene_root)
            .expect("children")
            .first()
            .expect("scene root");
        let entity = app
            .world()
            .get::<Children>(*top_entity)
            .expect("children")
            .first()
            .expect("model entity");
        let frame_entities = app.world().get::<Children>(*entity).expect("children");
        assert_eq!(frame_entities.len(), frame_count);
        let first_frame_visibility = app
            .world()
            .get::<Visibility>(frame_entities[0])
            .expect("Visibility of first frame");
        assert_eq!(
            first_frame_visibility,
            Visibility::Hidden,
            "Frame 0 invisible"
        );
        let second_frame_visibility = app
            .world()
            .get::<Visibility>(frame_entities[1])
            .expect("Visibility of second frame");
        assert_eq!(
            second_frame_visibility,
            Visibility::Inherited,
            "Frame 1 is showing"
        );
    }
}
