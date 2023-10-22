use std::mem::MaybeUninit;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use bytemuck::NoUninit;
use eframe::epaint::TextureHandle;
use parking_lot::Mutex;
use ringbuf::SharedRb;

use crate::kits::Shared;
use crate::player::{AV_TIME_BASE_RATIONAL, CommandGo, RingBufferProducer, timestamp_to_millisecond};
use crate::player::audio::{AudioDevice, AudioFrame};
use crate::player::consts::{VIDEO_SYNC_THRESHOLD_MAX, VIDEO_SYNC_THRESHOLD_MIN};
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

pub struct Clock {
    /// Clock的起始时间
    start: Instant,
    /// 当前帧的显示时间
    pts: f64,
    /// 当前帧的持续时间
    duration: f64,
    /// 当前帧的更新时间
    last_update: Duration,
}

impl Clock {
    fn new(start: Instant) -> Self {
        let last_update = start.elapsed();
        Self {
            start,
            pts: 0.0,
            duration: 0.0,
            last_update,
        }
    }

    pub fn current(&self) -> f64 {
        self.pts + (self.start.elapsed() - self.last_update.into()).as_secs_f64()
    }

    pub fn update(&mut self, pts: f64, duration: f64) {
        self.pts = pts;
        self.duration = duration;
        self.last_update = self.start.elapsed().into();
    }

    pub fn duration(&self) -> f64 {
        self.duration
    }
}

#[derive(Clone)]
pub struct PlayCtrl {
    pub player_state: Shared<PlayerState>,
    /// 解码开始时间, 也是音视频的起始时间
    start: Instant,
    /// 解封装(取包)完成
    packet_finished: Arc<AtomicBool>,
    /// 视频播放线程完成
    video_finished: Arc<AtomicBool>,
    /// 控制同步
    video_clock: Arc<Mutex<Clock>>,
    pub(crate) audio_dev: Arc<AudioDevice>,
    audio_finished: Arc<AtomicBool>,
    /// 音量控制
    pub audio_volume: Shared<f32>,
    /// 控制同步
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
    /// ui界面使用
    pub command_go_ui: Shared<CommandGo>,

}

impl PlayCtrl {
    pub fn new(
        duration: i64,
        producer: ringbuf::Producer<f32, Arc<SharedRb<f32, Vec<MaybeUninit<f32>>>>>,
        audio_dev: Arc<AudioDevice>,
        texture_handle: TextureHandle,
    ) -> Self {
        let start = Instant::now();
        let demux_finished = Arc::new(AtomicBool::new(false));
        let audio_finished = Arc::new(AtomicBool::new(false));
        let video_finished = Arc::new(AtomicBool::new(false));
        let video_clock = Arc::new(Mutex::new(Clock::new(start.clone())));
        let audio_clock = Arc::new(Mutex::new(Clock::new(start.clone())));

        Self {
            player_state: Shared::new(PlayerState::Paused),
            start,
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

            command_go: Shared::new(CommandGo::Packet(1)),
            command_go_ui: Shared::new(CommandGo::Packet(1)),
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

    /// 解封装播放线程是否完成
    pub fn packet_finished(&self) -> bool {
        self.packet_finished.load(Ordering::Relaxed)
    }

    /// 获取声音设备的默认配置
    pub fn audio_config(&self) -> cpal::SupportedStreamConfig {
        self.audio_dev.stream_config()
    }

    /// 播放音频帧
    pub fn play_audio(&mut self, mut frame: AudioFrame) -> Result<(), anyhow::Error> {
        // 更新音频时钟
        let _ = self.update_audio_clock(frame.pts, frame.duration);
        let mut producer = self.producer.lock();
        while producer.free_len() < frame.samples.len() {
            // log::info!("play audio: for : {}", producer.free_len());
            spin_sleep::sleep(Duration::from_millis(10));
        }
        // log::info!("play audio out: {}", frame.samples.len());
        if self.audio_dev.get_mute() {
            for f in frame.samples.as_mut_slice() {
                *f = 0.0;
            }
        }
        producer.push_slice(frame.samples.as_slice());
        // spin_sleep::sleep(Duration::from_secs_f64(delay));
        Ok(())
    }

    /// 播放视频帧
    pub fn play_video(&mut self, frame: VideoFrame, ctx: &egui::Context) -> Result<(), anyhow::Error> {
        let _ = self.update_video_clock(frame.pts, frame.duration);
        self.texture_handle.set(frame.color_image, egui::TextureOptions::LINEAR);
        ctx.request_repaint();
        // spin_sleep::sleep(Duration::from_secs_f64(delay));
        Ok(())
    }

    pub fn current_audio_clock(&self) -> f64 {
        self.audio_clock.lock().current()
    }

    #[inline]
    fn update_audio_clock(&self, pts: f64, duration: f64) -> f64 {
        let mut clock = self.audio_clock.lock();
        clock.update(pts, duration);
        duration
    }

    fn update_video_clock(&self, pts: f64, duration: f64) -> f64 {
        self.video_clock.lock().update(pts, duration);
        self.compute_video_delay()
    }

    fn compute_video_delay(&self) -> f64 {
        let audio_clock = self.audio_clock.lock().current();
        let video_clock = self.video_clock.lock().current();
        let duration = self.video_clock.lock().duration();
        let diff = video_clock - audio_clock;
        // 视频时钟落后于音频时钟, 超过了最小阈值
        if diff <= VIDEO_SYNC_THRESHOLD_MIN {
            // 在原来的duration基础上, 减少一定的休眠时间, 来达到追赶播放的目的 (最小休眠时间是0)
            0.0f64.max(duration + diff)
        }
        // 视频时钟超前于音频时钟, 且超过了最大阈值
        else if diff >= VIDEO_SYNC_THRESHOLD_MAX {
            // 放慢播放速度, 增加一定的休眠时间
            duration + VIDEO_SYNC_THRESHOLD_MAX
        }
        // 满足阈值范围, 则 正常的延迟时间
        else {
            duration
        }
    }
}
