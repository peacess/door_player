use ffmpeg::frame::Audio;
use ffmpeg::software;

use crate::{AudioSampleProducer, PlayerState, Streamer};
use crate::kits::Shared;

/// Streams audio.
pub struct AudioStreamer {
    pub(super) _video_elapsed_ms: Shared<i64>,
    pub(super) audio_elapsed_ms: Shared<i64>,
    pub(super) audio_stream_index: usize,
    pub(super) duration_ms: i64,
    pub(super) audio_decoder: ffmpeg::decoder::Audio,
    pub(super) re_sampler: software::resampling::Context,
    pub(super) audio_sample_producer: AudioSampleProducer,
    pub(super) input_context: ffmpeg::format::context::Input,
    pub(super) player_state: Shared<PlayerState>,
}

impl Streamer for AudioStreamer {
    type Frame = Audio;
    type ProcessedFrame = ();
    fn is_primary_streamer(&self) -> bool {
        false
    }
    fn stream_index(&self) -> usize {
        self.audio_stream_index
    }
    fn elapsed_ms(&mut self) -> &mut Shared<i64> {
        &mut self.audio_elapsed_ms
    }
    fn duration_ms(&mut self) -> i64 {
        self.duration_ms
    }
    fn decoder(&mut self) -> &mut ffmpeg::decoder::Opened {
        &mut self.audio_decoder.0
    }
    fn input_context(&mut self) -> &mut ffmpeg::format::context::Input {
        &mut self.input_context
    }
    fn player_state(&self) -> &Shared<PlayerState> {
        &self.player_state
    }
    fn decode_frame(&mut self) -> anyhow::Result<Self::Frame> {
        let mut decoded_frame = Audio::empty();
        self.audio_decoder.receive_frame(&mut decoded_frame)?;
        Ok(decoded_frame)
    }
    fn process_frame(&mut self, frame: Self::Frame) -> anyhow::Result<Self::ProcessedFrame> {
        let mut resampled_frame = Audio::empty();
        self.re_sampler.run(&frame, &mut resampled_frame)?;
        let audio_samples = if resampled_frame.is_packed() {
            packed(&resampled_frame)
        } else {
            resampled_frame.plane(0)
        };
        while self.audio_sample_producer.free_len() < audio_samples.len() {
            // std::thread::sleep(std::time::Duration::from_millis(10));
        }
        self.audio_sample_producer.push_slice(audio_samples);
        Ok(())
    }
}

#[inline]
// Thanks https://github.com/zmwangx/rust-ffmpeg/issues/72 <3
// Interpret the audio frame's data as packed (alternating channels, 12121212, as opposed to planar 11112222)
fn packed<T: ffmpeg::frame::audio::Sample>(frame: &Audio) -> &[T] {
    if !frame.is_packed() {
        panic!("data is not packed");
    }

    if !<T as ffmpeg::frame::audio::Sample>::is_valid(frame.format(), frame.channels()) {
        panic!("unsupported type");
    }

    unsafe {
        std::slice::from_raw_parts(
            (*frame.as_ptr()).data[0] as *const T,
            frame.samples() * frame.channels() as usize,
        )
    }
}