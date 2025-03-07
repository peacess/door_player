use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use bytemuck::NoUninit;
use ringbuf::traits::{Observer, Producer};

use crate::{
    kits::{Shared, TextureHandleNoMut},
    player::{
        audio::{AudioDevice, AudioPlayFrame},
        consts::VIDEO_SYNC_THRESHOLD_MAX,
        kits::{timestamp_to_millisecond, RingBufferProducer},
        video::VideoPlayFrame,
        Clock, CommandGo, AV_TIME_BASE_RATIONAL, VIDEO_SYNC_THRESHOLD_MIN,
    },
};

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

#[derive(Clone)]
pub struct PlayCtrl {
    pub player_state: Shared<PlayerState>,
    packet_finished: Arc<AtomicBool>,
    video_finished: Arc<AtomicBool>,
    video_clock: Arc<Clock>,
    pub audio_dev: Arc<AudioDevice>,
    audio_finished: Arc<AtomicBool>,
    pub audio_volume: Shared<f64>,
    audio_clock: Arc<Clock>,
    /// The player's texture handle.
    pub texture_handle: TextureHandleNoMut,
    // producer: Arc<Mutex<RingBufferProducer<f32>>>,
    pub duration: i64,
    pub duration_ms: i64,
    pub video_elapsed_ms: Shared<i64>,
    pub audio_elapsed_ms: Shared<i64>,
    pub video_elapsed_ms_override: Shared<i64>,

    pub command_go: Shared<CommandGo>,
    pub video_stream_time_base: Option<ffmpeg::Rational>,
    pub audio_stream_time_base: Option<ffmpeg::Rational>,
}

impl PlayCtrl {
    pub fn new(
        duration: i64,
        audio_dev: Arc<AudioDevice>,
        texture_handle: TextureHandleNoMut,
        video_stream_time_base: Option<ffmpeg::Rational>,
        audio_stream_time_base: Option<ffmpeg::Rational>,
    ) -> Self {
        let demux_finished = Arc::new(AtomicBool::new(false));
        let audio_finished = Arc::new(AtomicBool::new(false));
        let video_finished = Arc::new(AtomicBool::new(false));
        let video_clock = {
            let q2d = match video_stream_time_base {
                None => 0.0,
                Some(t) => f64::from(t),
            };
            Arc::new(Clock::new(q2d))
        };
        let audio_clock = {
            let q2d = match audio_stream_time_base {
                None => 0.0,
                Some(t) => f64::from(t),
            };
            Arc::new(Clock::new(q2d))
        };

        Self {
            player_state: Shared::new(PlayerState::Paused),
            packet_finished: demux_finished,

            video_finished,
            video_clock,
            audio_dev,
            audio_finished,
            audio_volume: Shared::new(0.5),
            audio_clock,
            texture_handle,
            duration,
            duration_ms: timestamp_to_millisecond(duration, AV_TIME_BASE_RATIONAL),
            video_elapsed_ms: Shared::new(0),
            audio_elapsed_ms: Shared::new(0),
            video_elapsed_ms_override: Shared::new(-1),
            command_go: Shared::new(CommandGo::None),
            video_stream_time_base,
            audio_stream_time_base,
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
        self.audio_dev.output_config()
    }
    pub fn play_audio(&self, mut frame: AudioPlayFrame, producer: &mut RingBufferProducer<f32>) -> Result<(), anyhow::Error> {
        if producer.vacant_len() < frame.samples.len() {
            // log::info!("play audio: for : {}", producer.free_len());
            while producer.vacant_len() < frame.samples.len() {
                // spin_sleep::sleep(Duration::from_nanos(10));
                std::thread::sleep(Duration::from_micros(1));
            }
        }
        // log::info!("play audio out: {}", frame.samples.len());
        self.update_audio_clock(frame.pts, frame.duration, frame.timestamp);
        if self.audio_dev.get_mute() {
            frame.samples.as_mut_slice().fill(0.0);
        }
        let mut s = frame.samples.as_slice();
        let done = producer.push_slice(s);
        if done == s.len() {
            // log::info!("play audio done");
        } else {
            // log::info!("play audio not one ");
            s = &s[done..];
            std::thread::sleep(Duration::from_micros(1));
            loop {
                let done = producer.push_slice(s);
                if done == s.len() {
                    log::info!("play audio done");
                    break;
                } else {
                    log::info!("play audio not one ");
                    s = &s[done..];
                    std::thread::sleep(Duration::from_micros(1));
                    // spin_sleep::sleep(Duration::from_nanos(10));
                }
            }
        }
        // spin_sleep::sleep(Duration::from_secs_f64(delay));
        Ok(())
    }

    pub fn play_video(&self, frame: VideoPlayFrame, ctx: &egui::Context) -> Result<(), anyhow::Error> {
        let delay = self.update_video_clock(frame.pts, frame.duration, frame.timestamp);
        self.texture_handle.set(frame.color_image, egui::TextureOptions::LINEAR);
        ctx.request_repaint();
        if delay > 0.0 {
            log::debug!("video delay: {}", delay);
            spin_sleep::sleep(Duration::from_secs_f64(delay));
        }
        Ok(())
    }

    #[inline]
    fn update_audio_clock(&self, pts: i64, duration: i64, timestamp: i64) {
        self.audio_clock.update(pts, duration, timestamp);
        if let Some(time_base) = &self.audio_stream_time_base {
            let t = timestamp_to_millisecond(pts, *time_base);
            self.audio_elapsed_ms.set(t);
            if self.video_stream_time_base.is_none() {
                //if no video stream, we should not update video elapsed time
                self.video_elapsed_ms.set(t);
            }
        }
    }

    #[inline]
    fn update_video_clock(&self, pts: i64, duration: i64, timestamp: i64) -> f64 {
        self.video_clock.update(pts, duration, timestamp);
        if let Some(time_base) = &self.video_stream_time_base {
            let t = timestamp_to_millisecond(pts, *time_base);
            self.video_elapsed_ms.set(t);
        }
        self.compute_video_delay()
    }

    fn compute_video_delay(&self) -> f64 {
        // let cache_frame = self.producer.lock().occupied_len() as i64 / 2000 + 2;
        let cache_frame = 4;
        let audio_clock = self.audio_clock.play_ts(cache_frame);
        let (video_clock, duration) = self.video_clock.play_ts_duration();
        let diff = video_clock - audio_clock;
        if audio_clock == 0.0 || video_clock == 0.0 {
            duration
        } else if diff <= VIDEO_SYNC_THRESHOLD_MIN {
            // 视频时钟落后于音频时钟, 超过了最小阈值
            // 在原来的duration基础上, 减少一定的休眠时间, 来达到追赶播放的目的 (最小休眠时间是0)
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

    // fn compute_video_delay(&self) -> f64 {
    //     let audio_clock = self.audio_clock.timestamp();
    //     let video_clock = self.video_clock.timestamp();
    //     let diff = video_clock - audio_clock;
    //     let duration = self.video_clock.play_duration();
    //     if diff <= VIDEO_SYNC_THRESHOLD_MIN {
    //         // 视频时钟落后于音频时钟, 超过了最小阈值
    //         // 在原来的duration基础上, 减少一定的休眠时间, 来达到追赶播放的目的 (最小休眠时间是0)
    //         0.0f64.max(diff + duration)
    //     }
    //     // 视频时钟超前于音频时钟, 且超过了最大阈值
    //     else if diff >= VIDEO_SYNC_THRESHOLD_MAX {
    //         // 放慢播放速度, 增加一定的休眠时间
    //         duration + VIDEO_SYNC_THRESHOLD_MAX
    //         // diff
    //     }
    //     // 满足阈值范围, 则 正常的延迟时间
    //     else {
    //         duration
    //     }
    // }
}
