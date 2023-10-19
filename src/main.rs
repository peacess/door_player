// use bevy::prelude::*;

use door_player::AppUi;

fn main() {
    if let Err(e) = ffmpeg::init() {
        log::error!("{}", e);
        return;
    }
    AppUi::run_app();
}
