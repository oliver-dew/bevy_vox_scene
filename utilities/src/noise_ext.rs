use bevy::math::Vec3;
use libnoise::Generator;

/// Convenience methods for sampling noise with Bevy vector types.
pub trait NoiseSampleExt: Sized {
    /// Sample three-dimensional noise at `point`.
    fn sample_vec3(&self, point: Vec3) -> f32
    where
        Self: Generator<3>,
    {
        self.sample([point.x.into(), point.y.into(), point.z.into()]) as f32
    }

    /// Sample four-dimensional noise using `time` as the fourth dimension.
    fn sample_vec3_time(&self, point: Vec3, time: f32) -> f32
    where
        Self: Generator<4>,
    {
        self.sample([point.x.into(), point.y.into(), point.z.into(), time.into()]) as f32
    }
}

impl<T> NoiseSampleExt for T {}
