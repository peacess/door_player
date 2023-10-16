// use std::sync::Arc;
// use std::sync::atomic::{AtomicBool, Ordering};
// use std::time::Instant;
// use chrono::Duration;
// use kanal::Sender;
// use parking_lot::{Condvar, Mutex, RwLock};
// use crate::player::audio_frame::AudioFrame;
// use crate::player::video_frame::VideoFrame;
//
// #[derive(Debug, Clone)]
// pub enum PlayState {
//     Start,
//     Playing,
//     /// 终止
//     Terminated,
//     Pausing(bool),
//     Video(VideoFrame),
//     Error(anyhow::Error),
// }
//
// impl Default for PlayState {
//     fn default() -> Self {
//         Self::Start
//     }
// }
//
// impl PartialEq for PlayState {
//     fn eq(&self, other: &Self) -> bool {
//         match (self, other) {
//             (Self::Video(_), Self::Video(_)) => true,
//             _ => core::mem::discriminant(self) == core::mem::discriminant(other),
//         }
//     }
// }
//
//
// #[derive(Clone, Default)]
// pub struct Pause {
//     pause: Arc<Mutex<bool>>,
//     pause_cond: Arc<Condvar>,
// }
//
// impl Pause {
//     pub fn pause(&self) -> bool {
//         *self.pause.lock()
//     }
//
//     pub fn set_pause(&self, pause: bool) {
//         *self.pause.lock() = pause;
//         if !pause {
//             self.notify_all();
//         }
//     }
//
//     pub fn wait(&self) {
//         self.pause_cond.wait(&mut self.pause.lock());
//     }
//
//     pub fn notify_all(&self) -> usize {
//         self.pause_cond.notify_all()
//     }
// }
//
// pub struct Clock {
//     /// Clock的起始时间
//     start: Instant,
//     /// 当前帧的显示时间
//     pts: f64,
//     /// 当前帧的持续时间
//     duration: f64,
//     /// 当前帧的更新时间
//     last_update: Duration,
// }
//
// impl Clock {
//     fn new(start: Instant) -> Self {
//         let last_update = start.elapsed().into();
//         Self {
//             start,
//             pts: 0.0,
//             duration: 0.0,
//             last_update,
//         }
//     }
//
//     pub fn current(&self) -> f64 {
//         self.pts + (self.start.elapsed() - self.last_update.into()).as_secs_f64()
//     }
//
//     pub fn update(&mut self, pts: f64, duration: f64) {
//         self.pts = pts;
//         self.duration = duration;
//         self.last_update = self.start.elapsed().into();
//     }
//
//     pub fn duration(&self) -> f64 {
//         self.duration
//     }
// }
//
// #[derive(Clone)]
// pub struct PlayControl {
//     /// 用于和 ui 交互, 发布状态信息
//     state_tx: Sender<PlayState>,
//     /// 解码开始时间, 也是音视频的起始时间
//     start: Instant,
//     /// 取消请求, 在播放完成时, 会设置为true, 则 相关线程就会退出
//     abort_request: Arc<AtomicBool>,
//     /// 暂停播放
//     pause: Pause,
//     /*
//         解封装
//     */
//     /// 解封装(取包)完成
//     demux_finished: Arc<AtomicBool>,
//     /*
//         视频
//     */
//     /// 视频包解码后得到的视频帧 格式转换后 采集到的RGB数据
//     video_frame_tx: Sender<VideoFrame>,
//     /// 视频播放线程完成
//     video_finished: Arc<AtomicBool>,
//     /// 控制同步
//     video_clock: Arc<RwLock<Clock>>,
//     /*
//         音频
//     */
//     /// 音频设备
//     audio_dev: Arc<RwLock<AudioDevice>>,
//     /// 音频播放线程完成
//     audio_finished: Arc<AtomicBool>,
//     /// 音频包解码后得到的音频帧转换成的 音频采样数据
//     audio_frame_tx: Sender<AudioFrame>,
//     /// 音量控制
//     volume: Arc<RwLock<f32>>,
//     /// 控制同步
//     audio_clock: Arc<RwLock<Clock>>,
// }
//
// impl PlayControl {
//     pub fn new(
//         audio_dev: Arc<RwLock<AudioDevice>>,
//         state_tx: Sender<PlayState>,
//         audio_frame_tx: Sender<AudioFrame>,
//         video_frame_tx: Sender<VideoFrame>,
//         abort_request: Arc<AtomicBool>,
//     ) -> Self {
//         let start = Instant::now();
//         let demux_finished = Arc::new(AtomicBool::new(false));
//         let audio_finished = Arc::new(AtomicBool::new(false));
//         let video_finished = Arc::new(AtomicBool::new(false));
//         let video_clock = Arc::new(RwLock::new(Clock::new(start.clone())));
//         let audio_clock = Arc::new(RwLock::new(Clock::new(start.clone())));
//         Self {
//             state_tx,
//             start,
//             abort_request,
//             pause: Pause::default(),
//             demux_finished,
//             video_finished,
//             video_frame_tx,
//             video_clock,
//             audio_dev,
//             audio_frame_tx,
//             audio_finished,
//             audio_clock,
//             volume: Arc::new(RwLock::new(1.0)),
//         }
//     }
//
//     /// 设置静音
//     pub fn set_mute(&self, mute: bool) {
//         self.audio_dev.write().set_mute(mute);
//     }
//
//     /// 设置音量大小
//     pub fn set_volume(&self, volume: f32) {
//         *self.volume.write() = volume;
//     }
//
//     /// 当前音量
//     pub fn volume(&self) -> f32 {
//         *self.volume.read()
//     }
//
//     /// 设置是否取消播放
//     pub fn set_abort_request(&self, abort_request: bool) {
//         self.abort_request.store(abort_request, Ordering::Relaxed);
//         self.audio_dev.write().stop();
//     }
//
//     /// 是否取消播放
//     pub fn abort_request(&self) -> bool {
//         self.abort_request.load(Ordering::Relaxed)
//     }
//
//     /// 设置是否暂停播放
//     pub fn set_pause(&mut self, pause: bool) {
//         self.pause.set_pause(pause);
//         self.audio_dev.write().set_pause(pause);
//         self.state_tx.send(PlayState::Pausing(pause)).ok();
//     }
//
//     /// 是否暂停播放
//     pub fn pause(&self) -> bool {
//         self.pause.pause()
//     }
//
//     /// 等待解除暂停的通知
//     pub fn wait_notify_in_pause(&self) {
//         self.pause.wait();
//     }
//
//     /// 设置音频播放线程是否完成
//     pub fn set_audio_finished(&self, finished: bool) {
//         self.audio_finished.store(finished, Ordering::Relaxed);
//     }
//
//     /// 音频播放线程是否完成
//     pub fn audio_finished(&self) -> bool {
//         self.audio_finished.load(Ordering::Relaxed)
//     }
//
//     /// 设置视频播放线程是否完成
//     pub fn set_video_finished(&self, finished: bool) {
//         self.video_finished.store(finished, Ordering::Relaxed);
//     }
//
//     /// 视频播放线程是否完成
//     pub fn video_finished(&self) -> bool {
//         self.video_finished.load(Ordering::Relaxed)
//     }
//
//     /// 设置解封装播放线程是否完成
//     pub fn set_demux_finished(&self, demux_finished: bool) {
//         self.demux_finished.store(demux_finished, Ordering::Relaxed);
//     }
//
//     /// 解封装播放线程是否完成
//     pub fn demux_finished(&self) -> bool {
//         self.demux_finished.load(Ordering::Relaxed)
//     }
//
//     /// 获取声音设备的默认配置
//     pub fn audio_default_config(&self) -> cpal::SupportedStreamConfig {
//         self.audio_dev.read().default_config()
//     }
//
//     /// 发送音频帧
//     pub fn send_audio(&self, audio: AudioFrame) -> core::result::Result<(), SendError<AudioFrame>> {
//         self.audio_frame_tx.send(audio)
//     }
//
//     /// 发送视频帧
//     pub fn send_video(&self, video: VideoFrame) -> core::result::Result<(), SendError<VideoFrame>> {
//         self.video_frame_tx.send(video)
//     }
//
//     /// 发送播放状态
//     pub fn send_state(
//         &self,
//         state: PlayState,
//     ) -> core::result::Result<(), TrySendError<PlayState>> {
//         self.state_tx.try_send(state)
//     }
//
//     /// 播放音频帧
//     pub fn play_audio(&self, frame: AudioFrame) -> Result<()> {
//         // 更新音频时钟
//         let delay = self.update_audio_clock(frame.pts(), frame.duration());
//         // 播放
//         self.audio_dev.write().play_source(frame);
//         // 休眠
//         spin_sleep::sleep(Duration::from_secs_f64(delay));
//         Ok(())
//     }
//
//     /// 播放视频帧
//     pub fn play_video(&self, frame: VideoFrame) -> Result<()> {
//         // 更新视频时钟
//         let delay = self.update_video_clock(frame.pts(), frame.duration());
//         // 播放
//         match self.send_state(PlayState::Video(frame)) {
//             Ok(_) | Err(TrySendError::Full(_)) => {}
//             Err(TrySendError::Disconnected(_)) => {
//                 return Err(PlayerError::Error("play channel disconnected".to_string()));
//             }
//         }
//         // 休眠
//         spin_sleep::sleep(Duration::from_secs_f64(delay));
//         Ok(())
//     }
//
//     pub fn current_audio_clock(&self) -> f64 {
//         self.audio_clock.write().current()
//     }
//
//     #[inline]
//     fn update_audio_clock(&self, pts: f64, duration: f64) -> f64 {
//         let mut clock = self.audio_clock.write();
//         clock.update(pts, duration);
//         duration
//     }
//
//     fn update_video_clock(&self, pts: f64, duration: f64) -> f64 {
//         self.video_clock.write().update(pts, duration);
//         self.compute_video_delay()
//     }
//
//     fn compute_video_delay(&self) -> f64 {
//         let audio_clock = self.audio_clock.read().current();
//         let video_clock = self.video_clock.read().current();
//         let duration = self.video_clock.read().duration();
//         let diff = video_clock - audio_clock;
//         // 视频时钟落后于音频时钟, 超过了最小阈值
//         if diff <= VIDEO_SYNC_THRESHOLD_MIN {
//             // 在原来的duration基础上, 减少一定的休眠时间, 来达到追赶播放的目的 (最小休眠时间是0)
//             0.0f64.max(duration + diff)
//         }
//         // 视频时钟超前于音频时钟, 且超过了最大阈值
//         else if diff >= VIDEO_SYNC_THRESHOLD_MAX {
//             // 放慢播放速度, 增加一定的休眠时间
//             duration + VIDEO_SYNC_THRESHOLD_MAX
//         }
//         // 满足阈值范围, 则 正常的延迟时间
//         else {
//             duration
//         }
//     }
// }
