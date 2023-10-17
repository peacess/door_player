

/// 视频同步阈值最小 (视频落后于音频的最小时间)
pub const VIDEO_SYNC_THRESHOLD_MIN: f64 = -0.1;
/// 视频同步阈值最大 (视频领先于音频的最大时间)
pub const VIDEO_SYNC_THRESHOLD_MAX: f64 = 0.025;