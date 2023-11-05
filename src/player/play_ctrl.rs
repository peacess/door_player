use std::mem::MaybeUninit;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration};

use bytemuck::NoUninit;
use eframe::epaint::TextureHandle;
use parking_lot::Mutex;
use ringbuf::SharedRb;

use crate::kits::Shared;
use crate::player::{AV_TIME_BASE_RATIONAL, CommandGo, RingBufferProducer, timestamp_to_millisecond};
use crate::player::audio::{AudioDevice, AudioFrame};
use crate::player::consts::VIDEO_SYNC_THRESHOLD_MAX;
use crate::player::video::VideoFrame;

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

#[derive(Debug, Default)]
pub struct Clock {
    pts: i64,
    frame_duration: i64,
    ///
    q2d: f64,
    /// 播放时刻
    play_ts: f64,
    /// frame的播放时长
    play_duration: f64,
}

impl Clock {
    fn new(q2d: f64) -> Self {
        Self {
            q2d,
            ..Default::default()
        }
    }

    pub fn play_ts(&self, frames: i64) -> f64 {
        self.play_ts + frames as f64 * self.q2d
    }

    pub fn play_ts_duration(&self) -> (f64, f64) {
        (self.play_ts, self.play_duration)
    }

    pub fn update(&mut self, pts: i64, frame_duration: i64) {
        self.pts = pts;
        self.frame_duration = frame_duration;
        self.play_ts = pts as f64 * self.q2d;
        self.play_duration = frame_duration as f64 * self.q2d;
    }
}

#[derive(Clone)]
pub struct PlayCtrl {
    pub player_state: Shared<PlayerState>,
    packet_finished: Arc<AtomicBool>,
    video_finished: Arc<AtomicBool>,
    video_clock: Arc<Mutex<Clock>>,
    pub(crate) audio_dev: Arc<AudioDevice>,
    audio_finished: Arc<AtomicBool>,
    pub audio_volume: Shared<f32>,
    audio_clock: Arc<Mutex<Clock>>,
    /// The player's texture handle.
    pub texture_handle: egui::TextureHandle,
    producer: Arc<Mutex<RingBufferProducer<f32>>>,

    pub duration: i64,
    pub duration_ms: i64,
    pub video_elapsed_ms: Shared<i64>,
    pub audio_elapsed_ms: Shared<i64>,
    pub video_elapsed_ms_override: Shared<i64>,

    pub command_go: Shared<CommandGo>,
}

impl PlayCtrl {
    pub fn new(
        duration: i64,
        producer: ringbuf::Producer<f32, Arc<SharedRb<f32, Vec<MaybeUninit<f32>>>>>,
        audio_dev: Arc<AudioDevice>,
        texture_handle: TextureHandle,
        video_q2d: f64, audio_q2d: f64,
    ) -> Self {
        let demux_finished = Arc::new(AtomicBool::new(false));
        let audio_finished = Arc::new(AtomicBool::new(false));
        let video_finished = Arc::new(AtomicBool::new(false));
        let video_clock = Arc::new(Mutex::new(Clock::new(video_q2d)));
        let audio_clock = Arc::new(Mutex::new(Clock::new(audio_q2d)));

        Self {
            player_state: Shared::new(PlayerState::Paused),
            // start,
            packet_finished: demux_finished,
            video_finished,
            video_clock,
            audio_dev,
            audio_finished,
            audio_clock,
            audio_volume: Shared::new(0.5),
            texture_handle,
            producer: Arc::new(Mutex::new(producer)),
            video_elapsed_ms: Shared::new(0),
            audio_elapsed_ms: Shared::new(0),

            command_go: Shared::new(CommandGo::None),
            duration,
            duration_ms: timestamp_to_millisecond(duration, AV_TIME_BASE_RATIONAL),
            video_elapsed_ms_override: Shared::new(-1),
        }
    }

    pub fn set_mute(&self, mute: bool) {
        self.audio_dev.set_mute(mute);
    }
    pub fn elapsed_ms(&self) -> i64 {
        match self.video_elapsed_ms_override.get() {
            -1 => self.video_elapsed_ms.get(),
            t => t,
        }
    }

    pub fn set_audio_finished(&self, finished: bool) {
        self.audio_finished.store(finished, Ordering::Relaxed);
    }
    pub fn audio_finished(&self) -> bool {
        self.audio_finished.load(Ordering::Relaxed)
    }
    pub fn set_video_finished(&self, finished: bool) {
        self.video_finished.store(finished, Ordering::Relaxed);
    }
    pub fn video_finished(&self) -> bool {
        self.video_finished.load(Ordering::Relaxed)
    }
    pub fn set_packet_finished(&self, demux_finished: bool) {
        self.packet_finished.store(demux_finished, Ordering::Relaxed);
    }
    pub fn packet_finished(&self) -> bool {
        self.packet_finished.load(Ordering::Relaxed)
    }

    pub fn audio_config(&self) -> cpal::SupportedStreamConfig {
        self.audio_dev.stream_input_config()
    }
    pub fn play_audio(&mut self, mut frame: AudioFrame) -> Result<(), anyhow::Error> {
        let mut producer = self.producer.lock();
        while producer.free_len() < frame.samples.len() {
            // log::info!("play audio: for : {}", producer.free_len());
            // spin_sleep::sleep(Duration::from_nanos(10));
            std::thread::sleep(Duration::from_micros(1));
        }
        // log::info!("play audio out: {}", frame.samples.len());
        let _ = self.update_audio_clock(frame.pts, frame.duration);
        if self.audio_dev.get_mute() {
            frame.samples.as_mut_slice().fill(0.0);
        }
        let mut s = frame.samples.as_slice();
        loop {
            let done = producer.push_slice(s);
            if done == s.len() {
                break;
            } else {
                s = &s[done..];
                std::thread::sleep(Duration::from_micros(1));
                // spin_sleep::sleep(Duration::from_nanos(10));
            }
        }


        // spin_sleep::sleep(Duration::from_secs_f64(delay));
        Ok(())
    }

    pub fn play_video(&mut self, frame: VideoFrame, ctx: &egui::Context) -> Result<(), anyhow::Error> {
        let delay = self.update_video_clock(frame.pts, frame.duration);
        self.texture_handle.set(frame.color_image, egui::TextureOptions::LINEAR);
        ctx.request_repaint();
        if delay > 0.0 {
            log::debug!("video delay: {}", delay);
            spin_sleep::sleep(Duration::from_secs_f64(delay));
        }
        Ok(())
    }

    #[inline]
    fn update_audio_clock(&self, pts: i64, duration: i64) {
        let mut clock = self.audio_clock.lock();
        clock.update(pts, duration);
    }

    #[inline]
    fn update_video_clock(&self, pts: i64, duration: i64) -> f64 {
        self.video_clock.lock().update(pts, duration);
        self.compute_video_delay()
    }

    fn compute_video_delay(&self) -> f64 {
        let cache_frame = self.producer.lock().len() as i64 / 2000 + 2;
        let audio_clock = self.audio_clock.lock().play_ts(cache_frame);
        let (video_clock, duration) = self.video_clock.lock().play_ts_duration();
        let diff = video_clock - audio_clock;
        // 视频时钟落后于音频时钟, 超过了最小阈值
        // if diff <= VIDEO_SYNC_THRESHOLD_MIN {
        if diff <= 0.0 {
            // 在原来的duration基础上, 减少一定的休眠时间, 来达到追赶播放的目的 (最小休眠时间是0)
            // 0.0
            0.0f64.max(duration + diff)
        }
        // 视频时钟超前于音频时钟, 且超过了最大阈值
        else if diff >= VIDEO_SYNC_THRESHOLD_MAX {
            // 放慢播放速度, 增加一定的休眠时间
            duration + VIDEO_SYNC_THRESHOLD_MAX
            // diff
        }
        // 满足阈值范围, 则 正常的延迟时间
        else {
            // 0.0
            duration
        }
    }
}
