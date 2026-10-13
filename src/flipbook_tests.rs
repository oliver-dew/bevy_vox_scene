#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::{VoxelInstanceReady, test_support::setup_and_load_voxel_scene};
    use bevy::{
        app::App,
        ecs::{entity::Entity, hierarchy::Children},
        prelude::{Commands, Component, On, Visibility},
        time::TimeUpdateStrategy,
        world_serialization::WorldAssetRoot,
    };
    use flipbook::{FlipbookPlayer, FlipbookRepeatMode, FlipbookSpeed};

    #[derive(Component)]
    struct TaggedAnimationRoot;

    fn animation_entity(app: &App) -> Entity {
        app.world()
            .iter_entities()
            .find(|entity| entity.contains::<FlipbookPlayer>())
            .map(|entity| entity.id())
            .expect("animation entity")
    }

    #[test]
    fn test_spawn_play_animation() {
        let frame_count: usize = 4;
        let mut app = App::new();
        let handle = setup_and_load_voxel_scene(&mut app, "deer.vox");
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO));
        app.update();
        app.add_observer(|trigger: On<VoxelInstanceReady>, mut commands: Commands| {
            if trigger.model_name.as_deref() == Some("deer") {
                commands
                    .entity(trigger.instance)
                    .insert((TaggedAnimationRoot, FlipbookSpeed(Duration::from_millis(1))));
            }
        });
        let _scene_root = app.world_mut().spawn(WorldAssetRoot(handle)).id();
        app.update();
        app.update();
        let entity = animation_entity(&app);
        app.world_mut().entity_mut(entity).insert((
            FlipbookPlayer::from_frame_count(frame_count),
            FlipbookSpeed(Duration::from_millis(1)),
        ));
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(1)));
        app.update();
        assert!(
            app.world().get::<TaggedAnimationRoot>(entity).is_some(),
            "VoxelInstanceReady should identify the unsuffixed animated-model parent"
        );
        let frame_entities = app.world().get::<Children>(entity).expect("children");
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

    #[test]
    fn test_animation_despawns_on_finish_when_enabled() {
        let mut app = App::new();
        let handle = setup_and_load_voxel_scene(&mut app, "deer.vox");
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO));
        app.update();
        let scene_root = app.world_mut().spawn(WorldAssetRoot(handle)).id();
        app.update();
        let animation = animation_entity(&app);
        app.world_mut().entity_mut(animation).insert((
            TaggedAnimationRoot,
            FlipbookPlayer::from_frame_count(1),
            FlipbookRepeatMode::DespawnAfterCount(1),
            FlipbookSpeed(Duration::from_millis(1)),
        ));
        assert!(
            app.world().get::<Children>(scene_root).is_some(),
            "scene root spawned"
        );
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(2)));
        app.update();
        app.update();
        let mut query = app.world_mut().query::<&TaggedAnimationRoot>();
        assert_eq!(
            query.iter(app.world()).count(),
            0,
            "animation root despawned"
        );
    }

    #[test]
    fn test_animation_kept_on_finish_when_despawn_disabled() {
        let mut app = App::new();
        let handle = setup_and_load_voxel_scene(&mut app, "deer.vox");
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO));
        app.update();
        app.world_mut().spawn(WorldAssetRoot(handle));
        app.update();
        let animation = animation_entity(&app);
        app.world_mut().entity_mut(animation).insert((
            TaggedAnimationRoot,
            FlipbookPlayer::from_frame_count(1),
            FlipbookRepeatMode::Count(1),
            FlipbookSpeed(Duration::from_millis(1)),
        ));
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(2)));
        app.update();
        let mut query = app.world_mut().query::<&TaggedAnimationRoot>();
        assert_eq!(
            query.iter(app.world()).count(),
            1,
            "animation root retained"
        );
    }
}
