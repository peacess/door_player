use std::io::Write;

use env_logger::Env;

use door_player::AppUi;
use door_player::ffmpeg;

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
    AppUi::run_app();
}
