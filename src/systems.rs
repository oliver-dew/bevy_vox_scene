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
