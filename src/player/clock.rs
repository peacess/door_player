use std::sync::atomic::Ordering;

#[derive(Debug, Default)]
pub struct Clock {
    q2d: f64,

    pts: std::sync::atomic::AtomicI64,
    frame_duration: std::sync::atomic::AtomicI64,
    timestamp: std::sync::atomic::AtomicI64,
    /// 播放时刻
    play_ts: atomic::Atomic<f64>,
    /// frame的播放时长
    play_duration: atomic::Atomic<f64>,
}

impl Clock {
    pub fn new(q2d: f64) -> Self {
        Self { q2d, ..Default::default() }
    }

    pub fn play_ts(&self, frames: i64) -> f64 {
        self.play_ts.load(Ordering::Relaxed) + frames as f64 * self.q2d
    }

    pub fn play_ts_duration(&self) -> (f64, f64) {
        (self.play_ts.load(Ordering::Relaxed), self.play_duration.load(Ordering::Relaxed))
    }

    pub fn update(&self, pts: i64, frame_duration: i64, timestamp: i64) {
        self.pts.store(pts, Ordering::Relaxed);
        self.frame_duration.store(frame_duration, Ordering::Relaxed);
        self.play_ts.store(pts as f64 * self.q2d, Ordering::Relaxed);
        self.play_duration.store(frame_duration as f64 * self.q2d, Ordering::Relaxed);
        self.timestamp.store(timestamp, Ordering::Relaxed);
    }

    pub fn timestamp(&self) -> f64 {
        self.timestamp.load(Ordering::Relaxed) as f64 / ffmpeg::sys::AV_TIME_BASE as f64
    }

    pub fn play_duration(&self) -> f64 {
        self.play_duration.load(Ordering::Relaxed)
    }
}
