use std::fmt::{Debug, Formatter};

use crate::player::player::PlayFrame;

#[derive(Default, Clone)]
pub struct VideoFrame {
    pub width: usize,
    pub height: usize,
    pub pts: f64,
    pub duration: f64,
    pub color_image: egui::ColorImage,
}

impl Debug for VideoFrame {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        //todo
        f.debug_struct("VideoFrame")
            // .field("color_image", &self.color_image)
            .field("pts", &self.pts)
            .field("duration", &self.duration)
            .finish()
    }
}

impl VideoFrame {
    pub fn new(
        color_image: egui::ColorImage,
        width: usize,
        height: usize,
        pts: f64,
        duration: f64,
    ) -> Self {
        Self {
            color_image,
            width,
            height,
            pts,
            duration,
        }
    }
}

impl PlayFrame for VideoFrame {
    fn pts(&self) -> f64 {
        self.pts
    }

    fn duration(&self) -> f64 {
        self.duration
    }

    fn mem_size(&self) -> usize {
        //todo
        self.color_image.pixels.len()
    }
}
