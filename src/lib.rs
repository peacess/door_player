#[cfg(feature = "meh_ffmpeg")]
pub extern crate ffmpeg;
#[cfg(not(feature = "meh_ffmpeg"))]
pub extern crate ffmpeg_next as ffmpeg;
// ffmpeg_next, ffmpeg_the_third,rusty_ffmpeg;

pub use app_ui::AppUi;

mod app_ui;
pub mod kits;
pub mod player;

// pub use ffmpeg;
