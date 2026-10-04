#[cfg(feature = "vox_gltf")]
mod vox_gltf_extras;

#[cfg(feature = "vox_gltf")]
pub use vox_gltf_extras::on_flipbook_ready;

use bevy::{
    app::{App, Plugin, Update},
    camera::visibility::Visibility,
    ecs::{
        component::Component,
        entity::Entity,
        hierarchy::Children,
        system::{Commands, Query, Res},
    },
    prelude::ReflectComponent,
    reflect::Reflect,
    time::{Stopwatch, Time},
};
use std::time::Duration;

pub struct FlipbookPlugin;

impl Plugin for FlipbookPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<FlipbookPlayer>()
            .register_type::<FlipbookFrame>()
            .add_systems(Update, update_animations);
    }
}

/// Plays Flipbook Animations
#[derive(Component, Clone, Reflect)]
#[reflect(Component)]
pub struct FlipbookPlayer {
    /// Frame indices
    pub frames: Vec<usize>,
    /// Duration that each frame remains on screen
    pub frame_rate: Duration,
    /// Whether the animation repeats
    pub repeat_mode: FlipbookRepeatMode,
    /// If true (default), and [`FlipbookPlayer::repeat_mode`] is not [`FlipbookRepeatMode::Forever`], entity will despawn upon completion
    pub despawn_on_finish: bool,
    /// If true, playback is paused
    pub is_paused: bool,
    /// timer that determines when frame should advance
    pub timer: FlipbookTimer,
}

impl Default for FlipbookPlayer {
    fn default() -> Self {
        Self {
            frames: vec![],
            frame_rate: Duration::from_secs_f32(1.0 / 8.0),
            repeat_mode: FlipbookRepeatMode::Forever,
            despawn_on_finish: true,
            is_paused: false,
            timer: FlipbookTimer::default(),
        }
    }
}

#[derive(Clone, Reflect)]
pub enum FlipbookRepeatMode {
    /// The animation will finish after running "n" times.
    Count(u32),
    /// The animation will never finish.
    Forever,
}

#[derive(Clone, Reflect)]
pub struct FlipbookTimer {
    current_frame_index: usize,
    stopwatch: Stopwatch,
    play_count: u32,
}

impl Default for FlipbookTimer {
    fn default() -> Self {
        Self {
            current_frame_index: 0,
            stopwatch: Stopwatch::new(),
            play_count: 0,
        }
    }
}

pub enum FlipbookUpdate {
    SameFrame,
    AdvanceFrame(usize),
    ReachedEnd,
}

impl FlipbookPlayer {
    fn did_advance_frame(&mut self, delta: Duration) -> FlipbookUpdate {
        if self.is_paused || self.frames.is_empty() {
            return FlipbookUpdate::SameFrame;
        }
        if let FlipbookRepeatMode::Count(end_count) = self.repeat_mode
            && self.timer.play_count >= end_count
        {
            return FlipbookUpdate::ReachedEnd;
        }
        self.timer.stopwatch.tick(delta);
        if self.timer.stopwatch.elapsed() > self.frame_rate {
            self.timer.current_frame_index += 1;
            if self.timer.current_frame_index == self.frames.len() {
                match self.repeat_mode {
                    FlipbookRepeatMode::Count(end_count) => {
                        self.timer.play_count += 1;
                        if self.timer.play_count >= end_count {
                            return FlipbookUpdate::ReachedEnd;
                        } else {
                            self.timer.current_frame_index = 0;
                        }
                    }
                    FlipbookRepeatMode::Forever => {
                        self.timer.play_count += 1;
                        self.timer.current_frame_index = 0;
                    }
                }
            }
            self.timer.stopwatch.reset();
            return FlipbookUpdate::AdvanceFrame(self.frames[self.timer.current_frame_index]);
        }
        FlipbookUpdate::SameFrame
    }
}

/// A frame of a flipbook animation, the parameter is its index. Must have an immediate parent with a [`FlipbookPlayer`].
#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct FlipbookFrame(pub usize);

fn update_animations(
    mut commands: Commands,
    mut animation_query: Query<(Entity, &mut FlipbookPlayer, &Children)>,
    mut frame_query: Query<(&FlipbookFrame, &mut Visibility)>,
    time: Res<Time>,
) {
    for (entity, mut animation, children) in animation_query.iter_mut() {
        let update = animation.did_advance_frame(time.delta());
        match update {
            FlipbookUpdate::SameFrame => (),
            FlipbookUpdate::AdvanceFrame(new_frame) => {
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
            FlipbookUpdate::ReachedEnd => {
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

    use super::{FlipbookPlayer, FlipbookRepeatMode, FlipbookUpdate};

    #[test]
    fn test_did_advance_frame_empty_frames_returns_same_frame() {
        let mut player = FlipbookPlayer {
            frames: vec![],
            frame_rate: Duration::from_millis(1),
            ..Default::default()
        };
        let update = player.did_advance_frame(Duration::from_millis(10));
        assert!(matches!(update, FlipbookUpdate::SameFrame));
    }

    #[test]
    fn test_did_advance_frame_count_mode_reaches_end() {
        let mut player = FlipbookPlayer {
            frames: vec![0],
            frame_rate: Duration::from_millis(1),
            repeat_mode: FlipbookRepeatMode::Count(1),
            ..Default::default()
        };

        let update = player.did_advance_frame(Duration::from_millis(10));
        assert!(matches!(update, FlipbookUpdate::ReachedEnd));
    }

    #[test]
    fn test_did_advance_frame_count_mode_advances_before_end() {
        let mut player = FlipbookPlayer {
            frames: vec![3, 7],
            frame_rate: Duration::from_millis(1),
            repeat_mode: FlipbookRepeatMode::Count(2),
            ..Default::default()
        };

        let update = player.did_advance_frame(Duration::from_millis(10));
        assert!(matches!(update, FlipbookUpdate::AdvanceFrame(7)));
    }
}
