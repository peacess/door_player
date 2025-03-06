#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] //hide console window on Windows in release version

use std::io::Write;

use door_player::{ffmpeg, AppUi};
use env_logger::Env;

fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("info"))
        .format(|buf, record| {
            writeln!(
                buf,
                "{}:{} {} [{}] - {}",
                chrono::Local::now().format("%Y-%m-%dT%H:%M:%S"),
                record.file().unwrap_or("unknown"),
                record.line().unwrap_or(0),
                record.level(),
                record.args()
            )
        })
        .init();
    if let Err(e) = ffmpeg::init() {
        log::error!("{}", e);
        return;
    }
    log::info!("ffmpeg version : {:?}", unsafe { std::ffi::CStr::from_ptr(ffmpeg::ffi::av_version_info()) });
    AppUi::run_app();
}
