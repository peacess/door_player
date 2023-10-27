use std::fmt::{Debug, Formatter};

#[derive(Default, Clone)]
pub struct SubtitleFrame {
    pub pts: f64,
    pub duration: i64,
    pub pure_text: String,
    pub ass: String,
    //当数据类型为ass时，的原始数据
    pub width: u32,
    pub height: u32,
}

impl Debug for SubtitleFrame {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SubtitleFrame")
            // .field("color_image", &self.color_image)
            .field("pts", &self.pts)
            .field("duration", &self.duration)
            .field("title", &self.pure_text)
            .finish()
    }
}

impl SubtitleFrame {
    pub fn new(
        sub_text: String,
        pts: f64,
        duration: i64,
    ) -> Self {
        Self {
            pure_text: sub_text,
            pts,
            duration,
            ..Default::default()
        }
    }
}