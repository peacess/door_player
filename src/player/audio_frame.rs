use std::vec::IntoIter;

#[derive(Clone)]
pub struct AudioFrame {
    pub samples: IntoIter<f32>,
    pub channels: u16,
    pub sample_rate: u32,
    pub pts: f64,
    pub duration: f64,
}