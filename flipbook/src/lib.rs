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
            .register_type::<FlipbookRepeatMode>()
            .register_type::<FlipbookSpeed>()
            .register_type::<FlipbookFrame>()
            .add_systems(Update, update_animations);
    }
}

/// Plays Flipbook Animations
#[derive(Component, Clone, Reflect, Default)]
#[reflect(Component)]
#[require(FlipbookRepeatMode, FlipbookSpeed)]
pub struct FlipbookPlayer {
    /// Frame indices
    frames: Vec<usize>,
    /// If true, playback is paused
    pub is_paused: bool,
    /// timer that determines when frame should advance
    timer: FlipbookTimer,
}

impl FlipbookPlayer {
    /// New FlipbookPlayer from a collection of frame indices
    pub fn new(frames: Vec<usize>) -> Self {
        FlipbookPlayer {
            frames,
            is_paused: false,
            timer: FlipbookTimer::default(),
        }
    }

    /// New FlipbookPlayer with frame indices 0..count
    pub fn from_frame_count(count: usize) -> Self {
        FlipbookPlayer::new((0..count).collect())
    }
}

#[derive(Component, Clone, Reflect, Default)]
#[reflect(Component)]
pub enum FlipbookRepeatMode {
    /// The animation will finish after running "n" times.
    Count(u32),
    /// The animation will finish and the entity will depsawn after running "n" times.
    DespawnAfterCount(u32),
    /// The animation will loop forever.
    #[default]
    Forever,
}

/// The speed at which the animation plays, defined as how long each frame in on screen.
/// Defaults to 8 frames-per-second equivalent
#[derive(Component, Clone, Reflect)]
#[reflect(Component)]
pub struct FlipbookSpeed(pub Duration);

impl FlipbookSpeed {
    /// Create FlipbookSpeed from frames-per-second
    pub fn from_frames_per_second(fps: f32) -> Self {
        FlipbookSpeed(Duration::from_secs_f32(1.0 / fps))
    }
}

impl Default for FlipbookSpeed {
    fn default() -> Self {
        FlipbookSpeed::from_frames_per_second(8.0)
    }
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
    fn did_advance_frame(
        &mut self,
        delta: Duration,
        repeat_mode: &FlipbookRepeatMode,
        speed: &FlipbookSpeed,
    ) -> FlipbookUpdate {
        if self.is_paused || self.frames.is_empty() {
            return FlipbookUpdate::SameFrame;
        }
        if let FlipbookRepeatMode::Count(end_count)
        | FlipbookRepeatMode::DespawnAfterCount(end_count) = repeat_mode
            && self.timer.play_count >= *end_count
        {
            return FlipbookUpdate::ReachedEnd;
        }
        self.timer.stopwatch.tick(delta);
        let elapsed = self.timer.stopwatch.elapsed();
        let steps = if speed.0.is_zero() {
            u128::from(!elapsed.is_zero())
        } else {
            elapsed.as_nanos() / speed.0.as_nanos()
        };
        if steps == 0 {
            return FlipbookUpdate::SameFrame;
        }
        let remainder = if speed.0.is_zero() {
            Duration::ZERO
        } else {
            duration_from_nanos(elapsed.as_nanos() % speed.0.as_nanos())
        };
        self.timer.stopwatch.set_elapsed(remainder);

        let frame_count = self.frames.len() as u128;
        if let FlipbookRepeatMode::Count(end_count)
        | FlipbookRepeatMode::DespawnAfterCount(end_count) = repeat_mode
        {
            let remaining_plays = end_count.saturating_sub(self.timer.play_count) as u128;
            let steps_until_end = remaining_plays
                .saturating_mul(frame_count)
                .saturating_sub(self.timer.current_frame_index as u128);
            if steps >= steps_until_end {
                self.timer.play_count = *end_count;
                self.timer.current_frame_index = self.frames.len() - 1;
                return FlipbookUpdate::ReachedEnd;
            }
        }

        let next = self.timer.current_frame_index as u128 + steps;
        let completed_plays = next / frame_count;
        self.timer.current_frame_index = (next % frame_count) as usize;
        self.timer.play_count = self
            .timer
            .play_count
            .saturating_add(completed_plays.min(u32::MAX as u128) as u32);
        FlipbookUpdate::AdvanceFrame(self.frames[self.timer.current_frame_index])
    }
}

fn duration_from_nanos(nanos: u128) -> Duration {
    const NANOS_PER_SECOND: u128 = 1_000_000_000;
    Duration::new(
        (nanos / NANOS_PER_SECOND) as u64,
        (nanos % NANOS_PER_SECOND) as u32,
    )
}

/// A frame of a flipbook animation, the parameter is its index. Must have an immediate parent with a [`FlipbookPlayer`].
#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct FlipbookFrame(pub usize);

fn update_animations(
    mut commands: Commands,
    mut animation_query: Query<(
        Entity,
        &mut FlipbookPlayer,
        &FlipbookRepeatMode,
        &FlipbookSpeed,
        &Children,
    )>,
    mut frame_query: Query<(&FlipbookFrame, &mut Visibility)>,
    time: Res<Time>,
) {
    for (entity, mut animation, repeat_mode, speed, children) in animation_query.iter_mut() {
        let update = animation.did_advance_frame(time.delta(), repeat_mode, speed);
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
                if let FlipbookRepeatMode::DespawnAfterCount(_) = repeat_mode {
                    commands.entity(entity).despawn();
                }
            }
        };
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{FlipbookPlayer, FlipbookRepeatMode, FlipbookSpeed, FlipbookUpdate};

    #[test]
    fn test_did_advance_frame_empty_frames_returns_same_frame() {
        let mut player = FlipbookPlayer::default();
        let update = player.did_advance_frame(
            Duration::from_millis(10),
            &FlipbookRepeatMode::Forever,
            &FlipbookSpeed(Duration::from_millis(1)),
        );
        assert!(matches!(update, FlipbookUpdate::SameFrame));
    }

    #[test]
    fn test_did_advance_frame_count_mode_reaches_end() {
        let mut player = FlipbookPlayer::from_frame_count(1);

        let update = player.did_advance_frame(
            Duration::from_millis(10),
            &FlipbookRepeatMode::Count(1),
            &FlipbookSpeed(Duration::from_millis(1)),
        );
        assert!(matches!(update, FlipbookUpdate::ReachedEnd));
    }

    #[test]
    fn test_did_advance_frame_count_mode_advances_before_end() {
        let mut player = FlipbookPlayer::new(vec![3, 7]);

        let update = player.did_advance_frame(
            Duration::from_millis(1),
            &FlipbookRepeatMode::Count(2),
            &FlipbookSpeed(Duration::from_millis(1)),
        );
        assert!(matches!(update, FlipbookUpdate::AdvanceFrame(7)));
    }

    #[test]
    fn exact_frame_duration_advances() {
        let mut player = FlipbookPlayer::from_frame_count(3);

        let update = player.did_advance_frame(
            Duration::from_millis(100),
            &FlipbookRepeatMode::Forever,
            &FlipbookSpeed(Duration::from_millis(100)),
        );

        assert!(matches!(update, FlipbookUpdate::AdvanceFrame(1)));
    }

    #[test]
    fn long_updates_advance_multiple_frames_and_preserve_remainder() {
        let mut player = FlipbookPlayer::from_frame_count(4);
        let speed = FlipbookSpeed(Duration::from_millis(100));

        let first = player.did_advance_frame(
            Duration::from_millis(350),
            &FlipbookRepeatMode::Forever,
            &speed,
        );
        let second = player.did_advance_frame(
            Duration::from_millis(50),
            &FlipbookRepeatMode::Forever,
            &speed,
        );

        assert!(matches!(first, FlipbookUpdate::AdvanceFrame(3)));
        assert!(matches!(second, FlipbookUpdate::AdvanceFrame(0)));
    }
}
