use std::fmt::{Debug, Formatter};

#[derive(Default, Clone)]
pub struct SubtitleFrame {
    pub pts: f64,
    pub duration: f64,
    pub sub_text: String,
}

impl Debug for SubtitleFrame {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SubtitleFrame")
            // .field("color_image", &self.color_image)
            .field("pts", &self.pts)
            .field("duration", &self.duration)
            .field("title", &self.sub_text)
            .finish()
    }
}

impl SubtitleFrame {
    pub fn new(
        sub_text: String,
        pts: f64,
        duration: f64,
    ) -> Self {
        Self {
            sub_text,
            pts,
            duration,
        }
    }
}