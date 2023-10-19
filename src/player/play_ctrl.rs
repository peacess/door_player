use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use eframe::epaint::TextureHandle;
use parking_lot::{Condvar, Mutex};

use crate::kits::Shared;
use crate::player::audio::{AudioDevice, AudioFrame};
use crate::player::consts::{VIDEO_SYNC_THRESHOLD_MAX, VIDEO_SYNC_THRESHOLD_MIN};
use crate::player::player::PlayFrame;
use crate::player::video::VideoFrame;
use crate::PlayerState;

#[derive(Clone, Default)]
pub struct Pause {
    pause: Arc<Mutex<bool>>,
    pause_cond: Arc<Condvar>,
}

impl Pause {
    pub fn pause(&self) -> bool {
        *self.pause.lock()
    }

    pub fn set_pause(&self, pause: bool) {
        *self.pause.lock() = pause;
        if !pause {
            self.notify_all();
        }
    }

    pub fn wait(&self) {
        self.pause_cond.wait(&mut self.pause.lock());
    }

    fn notify_all(&self) -> usize {
        self.pause_cond.notify_all()
    }
}

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
    /// 取消请求, 在播放完成时, 会设置为true, 则 相关线程就会退出
    abort_req: Arc<AtomicBool>,
    pause: Pause,
    pub seek_scale: Arc<atomic::Atomic<f64>>,
    /// 解封装(取包)完成
    packet_finished: Arc<AtomicBool>,
    /// 视频播放线程完成
    video_finished: Arc<AtomicBool>,
    /// 控制同步
    video_clock: Arc<Mutex<Clock>>,
    audio_dev: Arc<Mutex<AudioDevice>>,
    audio_finished: Arc<AtomicBool>,
    /// 音量控制
    volume: Arc<atomic::Atomic<f32>>,
    /// 控制同步
    audio_clock: Arc<Mutex<Clock>>,
    /// The player's texture handle.
    pub texture_handle: egui::TextureHandle,
}

impl PlayCtrl {
    pub fn new(
        audio_dev: Arc<Mutex<AudioDevice>>,
        abort_request: Arc<AtomicBool>,
        texture_handle: TextureHandle,
    ) -> Self {
        let start = Instant::now();
        let demux_finished = Arc::new(AtomicBool::new(false));
        let audio_finished = Arc::new(AtomicBool::new(false));
        let video_finished = Arc::new(AtomicBool::new(false));
        let video_clock = Arc::new(Mutex::new(Clock::new(start.clone())));
        let audio_clock = Arc::new(Mutex::new(Clock::new(start.clone())));

        Self {
            player_state: Shared::new(PlayerState::Stopped),
            start,
            abort_req: abort_request,
            pause: Pause::default(),
            seek_scale: Arc::new(atomic::Atomic::new(-1.0)),
            packet_finished: demux_finished,
            video_finished,
            video_clock,
            audio_dev,
            audio_finished,
            audio_clock,
            volume: Arc::new(atomic::Atomic::new(1.0)),
            texture_handle,
        }
    }

    /// 设置静音
    pub fn set_mute(&self, mute: bool) {
        self.audio_dev.lock().set_mute(mute);
    }

    /// 设置音量大小
    pub fn set_volume(&self, volume: f32) {
        self.volume.store(volume, Ordering::Relaxed);
    }

    /// 当前音量
    pub fn volume(&self) -> f32 {
        self.volume.load(Ordering::Relaxed)
    }

    pub fn seek(&mut self, seek_scale: f64) {
        self.seek_scale.store(seek_scale, Ordering::Relaxed);
    }

    /// 设置是否取消播放
    pub fn set_abort_req(&self, abort_req: bool) {
        self.abort_req.store(abort_req, Ordering::Relaxed);
        self.audio_dev.lock().stop();
    }

    /// 是否取消播放
    pub fn abort_req(&self) -> bool {
        self.abort_req.load(Ordering::Relaxed)
    }

    /// 设置是否暂停播放
    pub fn set_pause(&mut self, pause: bool) {
        self.pause.set_pause(pause);
        self.audio_dev.lock().set_pause(pause);
    }

    /// 是否暂停播放
    pub fn pause(&self) -> bool {
        self.pause.pause()
    }

    /// 等待解除暂停的通知
    pub fn wait_notify_in_pause(&self) {
        self.pause.wait();
    }

    /// 设置音频播放线程是否完成
    pub fn set_audio_finished(&self, finished: bool) {
        self.audio_finished.store(finished, Ordering::Relaxed);
    }

    /// 音频播放线程是否完成
    pub fn audio_finished(&self) -> bool {
        self.audio_finished.load(Ordering::Relaxed)
    }

    /// 设置视频播放线程是否完成
    pub fn set_video_finished(&self, finished: bool) {
        self.video_finished.store(finished, Ordering::Relaxed);
    }

    /// 视频播放线程是否完成
    pub fn video_finished(&self) -> bool {
        self.video_finished.load(Ordering::Relaxed)
    }

    /// 设置解封装播放线程是否完成
    pub fn set_demux_finished(&self, demux_finished: bool) {
        self.packet_finished.store(demux_finished, Ordering::Relaxed);
    }

    /// 解封装播放线程是否完成
    pub fn demux_finished(&self) -> bool {
        self.packet_finished.load(Ordering::Relaxed)
    }

    /// 获取声音设备的默认配置
    pub fn audio_default_config(&self) -> cpal::SupportedStreamConfig {
        self.audio_dev.lock().stream_config()
    }

    /// 播放音频帧
    pub fn play_audio(&mut self, frame: AudioFrame) -> Result<(), anyhow::Error> {
        // 更新音频时钟
        let delay = self.update_audio_clock(frame.pts(), frame.duration());
        // 播放
        self.audio_dev.lock().play_source(frame);
        // 休眠
        // spin_sleep::sleep(Duration::from_secs_f64(delay));
        Ok(())
    }

    /// 播放视频帧
    pub fn play_video(&mut self, frame: VideoFrame, ctx: &egui::Context) -> Result<(), anyhow::Error> {
        // 更新视频时钟
        let delay = self.update_video_clock(frame.pts(), frame.duration());

        // 播放
        self.texture_handle.set(frame.color_image, egui::TextureOptions::LINEAR);
        ctx.request_repaint();
        // match self.send_state(PlayState::Video(frame)) {
        //     Ok(_) => {}
        //     Err(SendError::Closed) | Err(SendError::ReceiveClosed) => {
        //         return Err(anyhow::Error::msg("play channel disconnected"));
        //     }
        // }
        // 休眠
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
