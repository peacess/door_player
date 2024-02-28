use std::time::Duration;

use ffmpeg::sys::AV_TIME_BASE;
use ffmpeg::Rational;

pub const AUDIO_PACKET_QUEUE_SIZE: usize = 2;
pub const VIDEO_PACKET_QUEUE_SIZE: usize = 12;
pub const AUDIO_FRAME_QUEUE_SIZE: usize = 5;
pub const VIDEO_FRAME_QUEUE_SIZE: usize = 1;

pub const PLAY_MIN_INTERVAL: Duration = Duration::from_millis(20);

/// 视频同步阈值最小 (视频落后于音频的最小时间)
pub const VIDEO_SYNC_THRESHOLD_MIN: f64 = -0.1;
/// 视频同步阈值最大 (视频领先于音频的最大时间)
pub const VIDEO_SYNC_THRESHOLD_MAX: f64 = 0.025;
pub const AV_TIME_BASE_RATIONAL: Rational = Rational(1, AV_TIME_BASE);
pub const MILLISECOND_TIME_BASE: Rational = Rational(1, 1000);
/// if dont move the mouse for [MAX_DIFF_MOVE_MOUSE], then hide the status bar
pub const MAX_DIFF_MOVE_MOUSE: i64 = 1000 * 5; // 5 seconds
