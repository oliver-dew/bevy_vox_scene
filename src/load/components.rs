use std::time::Duration;

use bevy::{
    animation::RepeatAnimation,
    asset::Handle,
    ecs::component::Component,
    prelude::{ReflectComponent, Transform, Visibility},
    reflect::Reflect,
    time::Stopwatch,
};

use crate::{VoxelContext, VoxelModel};

#[derive(Debug, Clone)]
pub(crate) struct LayerInfo {
    pub name: Option<String>,
    pub is_hidden: bool,
}

/// An instance of a [`VoxelModel`], or an animation consisting of a series of models.
#[derive(Component, Clone, Reflect)]
#[reflect(Component)]
#[require(Transform, Visibility)]
pub struct VoxelModelInstance {
    /// Handle to the model
    pub models: Vec<Handle<VoxelModel>>,
    /// Handle to the context
    pub context: Handle<VoxelContext>,
}

impl VoxelModelInstance {
    /// Create a new instance for a single model (no animation frames)
    pub fn new(model: Handle<VoxelModel>, context: Handle<VoxelContext>) -> Self {
        Self {
            models: vec![model],
            context,
        }
    }

    pub(crate) fn has_animation(&self) -> bool {
        self.models.len() > 1
    }
}

/// Plays Voxel Animations
#[derive(Component, Clone, Reflect)]
#[reflect(Component)]
pub struct VoxelAnimationPlayer {
    /// Frame indices
    pub frames: Vec<usize>,
    /// Duration that each frame remains on screen
    pub frame_rate: Duration,
    /// Whether the animation repeats
    pub repeat_mode: RepeatAnimation,
    /// If true (default), and [`VoxelAnimation::repeat_mode`] is not [`RepeatAnimation::Forever`], entity will despawn upon completion
    pub despawn_on_finish: bool,
    /// If true, playback is paused
    pub is_paused: bool,
    /// timer that determines when frame should advance
    pub timer: AnimationTimer,
}

impl Default for VoxelAnimationPlayer {
    fn default() -> Self {
        Self {
            frames: vec![],
            frame_rate: Duration::from_secs_f32(1.0 / 8.0),
            repeat_mode: RepeatAnimation::Forever,
            despawn_on_finish: true,
            is_paused: false,
            timer: AnimationTimer::default(),
        }
    }
}

#[derive(Clone, Reflect)]
pub struct AnimationTimer {
    current_frame_index: usize,
    stopwatch: Stopwatch,
    play_count: u32,
}

impl Default for AnimationTimer {
    fn default() -> Self {
        Self {
            current_frame_index: 0,
            stopwatch: Stopwatch::new(),
            play_count: 0,
        }
    }
}

pub enum AnimationUpdate {
    SameFrame,
    AdvanceFrame(usize),
    ReachedEnd,
}

impl VoxelAnimationPlayer {
    pub(crate) fn did_advance_frame(&mut self, delta: Duration) -> AnimationUpdate {
        if self.is_paused {
            return AnimationUpdate::SameFrame;
        }
        self.timer.stopwatch.tick(delta);
        if self.timer.stopwatch.elapsed() > self.frame_rate {
            self.timer.current_frame_index += 1;
            if self.timer.current_frame_index == self.frames.len() {
                match self.repeat_mode {
                    RepeatAnimation::Never => return AnimationUpdate::ReachedEnd,
                    RepeatAnimation::Count(end_count) => {
                        self.timer.play_count += 1;
                        if self.timer.play_count >= end_count {
                            return AnimationUpdate::ReachedEnd;
                        } else {
                            self.timer.current_frame_index = 0;
                        }
                    }
                    RepeatAnimation::Forever => {
                        self.timer.play_count += 1;
                        self.timer.current_frame_index = 0;
                    }
                }
            }
            self.timer.stopwatch.reset();
            return AnimationUpdate::AdvanceFrame(self.frames[self.timer.current_frame_index]);
        }
        AnimationUpdate::SameFrame
    }
}

#[derive(Component)]
pub struct VoxelAnimationFrame(pub usize);

/// A component specifying which layer the Entity belongs to, with an optional name.
///
/// This can be configured in the Magica Voxel world editor.
#[derive(Component, Clone, Reflect)]
#[reflect(Component)]
pub struct VoxelLayer {
    /// The identifier for the layer. Magic Voxel 0.99.6 allows you to assign nodes to one of 8 layers,
    /// so this value will be an index in the range 0 through 7.
    pub id: u32,
    /// An optional name for the Layer, assignable in Magica Voxel layer editor.
    pub name: Option<String>,
}
