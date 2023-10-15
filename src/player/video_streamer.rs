use egui::{Color32, ColorImage};
use ffmpeg::decoder;
use ffmpeg::format::context::input::Input;
use ffmpeg::format::Pixel;
use ffmpeg::software::scaling::{context::Context, flag::Flags};
use ffmpeg::util::frame::video::Video;

use crate::{PlayerState, Streamer};
use crate::kits::Shared;
use crate::player::ApplyVideoFrameFn;

/// Streams video.
pub struct VideoStreamer {
    pub(super) video_decoder: decoder::Video,
    pub(super) video_stream_index: usize,
    pub(super) player_state: Shared<PlayerState>,
    pub(super) duration_ms: i64,
    pub(super) input_context: Input,
    pub(super) video_elapsed_ms: Shared<i64>,
    pub(super) _audio_elapsed_ms: Shared<i64>,
    pub(super) apply_video_frame_fn: Option<ApplyVideoFrameFn>,
}

impl Streamer for VideoStreamer {
    type Frame = Video;
    type ProcessedFrame = ColorImage;
    fn is_primary_streamer(&self) -> bool {
        true
    }
    fn stream_index(&self) -> usize {
        self.video_stream_index
    }
    fn elapsed_ms(&mut self) -> &mut Shared<i64> {
        &mut self.video_elapsed_ms
    }
    fn duration_ms(&mut self) -> i64 {
        self.duration_ms
    }
    fn decoder(&mut self) -> &mut decoder::Opened {
        &mut self.video_decoder.0
    }
    fn input_context(&mut self) -> &mut Input {
        &mut self.input_context
    }
    fn player_state(&self) -> &Shared<PlayerState> {
        &self.player_state
    }
    fn decode_frame(&mut self) -> anyhow::Result<Self::Frame> {
        let mut decoded_frame = Video::empty();
        self.video_decoder.receive_frame(&mut decoded_frame)?;
        Ok(decoded_frame)
    }
    fn process_frame(&mut self, frame: Self::Frame) -> anyhow::Result<Self::ProcessedFrame> {
        let mut rgb_frame = Video::empty();
        let mut context = Context::get(
            frame.format(),
            frame.width(),
            frame.height(),
            Pixel::RGB24,
            frame.width(),
            frame.height(),
            Flags::BILINEAR,
        )?;
        context.run(&frame, &mut rgb_frame)?;

        let image = video_frame_to_image(rgb_frame);
        Ok(image)
    }
    fn apply_frame(&mut self, frame: Self::ProcessedFrame) {
        if let Some(apply_video_frame_fn) = self.apply_video_frame_fn.as_mut() {
            apply_video_frame_fn(frame)
        }
    }
}

fn video_frame_to_image(frame: Video) -> ColorImage {
    let size = [frame.width() as usize, frame.height() as usize];
    let data = frame.data(0);
    let stride = frame.stride(0);
    let pixel_size_bytes = 3;
    let byte_width: usize = pixel_size_bytes * frame.width() as usize;
    let height: usize = frame.height() as usize;
    let mut pixels = vec![];
    for line in 0..height {
        let begin = line * stride;
        let end = begin + byte_width;
        let data_line = &data[begin..end];
        pixels.extend(
            data_line
                .chunks_exact(pixel_size_bytes)
                .map(|p| Color32::from_rgb(p[0], p[1], p[2])),
        )
    }
    ColorImage { size, pixels }
}
