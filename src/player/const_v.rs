use std::sync::Arc;

use bytemuck::NoUninit;
use egui::ColorImage;
use ffmpeg_the_third::ffi::AV_TIME_BASE;
use ffmpeg_the_third::Rational;
use ringbuf::SharedRb;
use sdl2::audio::{self};

use crate::AudioDeviceCallback;

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum PlayerState {
    /// No playback.
    Stopped,
    /// Streams have reached the end of the file.
    EndOfFile,
    /// Stream is seeking. Inner bool represents whether or not the seek is completed.
    Seeking(bool),
    /// Playback is paused.
    Paused,
    /// Playback is ongoing.
    Playing,
    /// Playback is scheduled to restart.
    Restarting,
}

unsafe impl NoUninit for PlayerState {}

pub const AV_TIME_BASE_RATIONAL: Rational = Rational(1, AV_TIME_BASE);
pub const MILLISEC_TIME_BASE: Rational = Rational(1, 1000);

type RingBufferProducer<T> = ringbuf::Producer<T, Arc<SharedRb<T, Vec<std::mem::MaybeUninit<T>>>>>;
type RingBufferConsumer<T> = ringbuf::Consumer<T, Arc<SharedRb<T, Vec<std::mem::MaybeUninit<T>>>>>;
pub type AudioSampleProducer = RingBufferProducer<f32>;
pub type AudioSampleConsumer = RingBufferConsumer<f32>;

/// The playback device. Needs to be initialized (and kept alive!) for use by a [`Player`].
pub type AudioDevice = audio::AudioDevice<AudioDeviceCallback>;

pub(super) type ApplyVideoFrameFn = Box<dyn FnMut(ColorImage) + Send>;

pub fn is_ffmpeg_eof_error(error: &anyhow::Error) -> bool {
    matches!(
        error.downcast_ref::<ffmpeg_the_third::Error>(),
        Some(ffmpeg_the_third::Error::Eof)
    )
}