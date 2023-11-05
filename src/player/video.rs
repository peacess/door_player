use std::fmt::{Debug, Formatter};

#[derive(Default, Clone)]
pub struct VideoFrame {
    pub width: usize,
    pub height: usize,
    pub pts: i64,
    pub duration: i64,
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
        pts: i64,
        duration: i64,
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