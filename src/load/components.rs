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
            context 
        }
    }

    pub(crate) fn has_animation(&self) -> bool {
        self.models.len() > 1
    }
}

/// Plays Voxel Animations
#[derive(Component, Clone, Reflect)]
#[reflect(Component)]
pub struct VoxelAnimation {
    /// Frame indexes
    pub frames: Vec<usize>,
    /// index of currently displayed frame
    pub current_frame: usize,
    /// timer that determines when frame should advance
    pub frame_timer: Stopwatch,
    /// Duration that each frame remains on screen
    pub frame_rate: Duration,
    /// Whether the animation repeats
    pub repeat_mode: RepeatAnimation,
    /// If true (default), and [`VoxelAnimation::repeat_mode`] is not [`RepeatAnimation::Forever`], entity will despawn upon completion
    pub despawn_on_finish: bool,
    /// How many times the animation has played
    pub play_count: u32,
    /// If true, playback is paused
    pub is_paused: bool,
}

impl Default for VoxelAnimation {
    fn default() -> Self {
        Self {
            frames: vec![],
            current_frame: 0,
            frame_timer: Stopwatch::new(),
            frame_rate: Duration::from_secs_f32(1.0 / 8.0),
            repeat_mode: RepeatAnimation::Forever,
            despawn_on_finish: true,
            play_count: 0,
            is_paused: false,
        }
    }
}

pub enum AnimationUpdate {
    SameFrame,
    AdvanceFrame(usize),
    ReachedEnd,
}

impl VoxelAnimation {
    pub(crate) fn did_advance_frame(&mut self, delta: Duration) -> AnimationUpdate {
        if self.is_paused {
            return AnimationUpdate::SameFrame;
        }
        self.frame_timer.tick(delta);
        if self.frame_timer.elapsed() > self.frame_rate {
            self.current_frame += 1;
            if self.current_frame == self.frames.len() {
                match self.repeat_mode {
                    RepeatAnimation::Never => return AnimationUpdate::ReachedEnd,
                    RepeatAnimation::Count(end_count) => {
                        self.play_count += 1;
                        if self.play_count >= end_count {
                            return AnimationUpdate::ReachedEnd;
                        } else {
                            self.current_frame = 0;
                        }
                    }
                    RepeatAnimation::Forever => {
                        self.play_count += 1;
                        self.current_frame = 0;
                    }
                }
            }
            self.frame_timer.reset();
            return AnimationUpdate::AdvanceFrame(self.current_frame);
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
