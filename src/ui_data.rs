use std::{fs, path};

use egui::{Event, Key, Ui};

use crate::{AudioDevice, init_audio_device_default, Player, PlayerState};
use crate::player::Streamer;

pub struct AppUi {
    collapse: bool,
    audio_device: Option<AudioDevice>,
    player: Option<Player>,

    media_path: String,
    no_scale: bool,
    // stream_size_scale: f32,
    // seek_frac: f32,
}

impl AppUi {
    pub(crate) fn handle_key_player(ui: &mut Ui, player: &mut Player) {
        ui.input(|k| {
            for e in &k.events {
                let mut seek = 0.0f32;
                match e {
                    Event::Key { key, pressed: true, .. } => {
                        match *key {
                            Key::ArrowLeft => {
                                let mut v = player.video_streamer.lock();
                                let els = v.elapsed_ms().get() as f32 - 1.0;
                                if els > 0.0 {
                                    seek = els / v.duration_ms() as f32;
                                }
                            }
                            Key::ArrowRight => {
                                let mut v = player.video_streamer.lock();
                                let els = v.elapsed_ms().get() as f32 + 1.0;
                                seek = els / v.duration_ms() as f32;
                            }
                            Key::ArrowUp => {}
                            Key::ArrowDown => {}
                            Key::Space => {
                                let state = player.player_state.get();
                                match state {
                                    PlayerState::Stopped => {
                                        player.start();
                                    }
                                    PlayerState::Paused  => {
                                        player.resume();
                                    }
                                    PlayerState::Playing => {
                                        player.pause();
                                    }
                                    _ => {}
                                }
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
                if seek > 0.0 {
                    player.seek(seek);
                }
            }
        });
    }

    pub(crate) fn next_file(file: &str) -> String {
        let path_file = path::PathBuf::from(file);
        if path_file.file_name().is_none() {
            return String::default();
        }
        let mut files = Vec::new();
        match fs::read_dir(path_file.parent().unwrap()) {
            Err(e) => {
                log::error!("{}",e);
                return String::default();
            }
            Ok(read_dir) => {
                for f in read_dir {
                    if let Ok(ff) = f {
                        files.push(ff.file_name());
                    }
                }
            }
        }

        files.sort();
        if let Ok(i) = files.binary_search(&path_file.file_name().unwrap().to_os_string()) {
            let i = {
                if i == files.len() - 1 {
                    0
                } else {
                    i + 1
                }
            };
            let f = files.get(i);
            return path_file.parent().unwrap().join(f.unwrap()).to_string_lossy().to_string();
        }
        return String::default();
    }

    pub(crate) fn pre_file(file: &str) -> String {
        let path_file = path::PathBuf::from(file);
        if path_file.file_name().is_none() {
            return String::default();
        }
        let mut files = Vec::new();
        match fs::read_dir(path_file.parent().unwrap()) {
            Err(e) => {
                log::error!("{}",e);
                return String::default();
            }
            Ok(read_dir) => {
                for f in read_dir {
                    if let Ok(ff) = f {
                        files.push(ff.file_name());
                    }
                }
            }
        }

        files.sort();
        if let Ok(i) = files.binary_search(&path_file.file_name().unwrap().to_os_string()) {
            let i = {
                if i == 0 {
                    files.len() - 1
                } else {
                    i - 1
                }
            };
            let f = files.get(i);
            return path_file.parent().unwrap().join(f.unwrap()).to_string_lossy().to_string();
        }
        return String::default();
    }
}

unsafe impl Send for AppUi {}

unsafe impl Sync for AppUi {}

impl Default for AppUi {
    fn default() -> Self {
        Self {
            collapse: true,
            audio_device: Some(init_audio_device_default().unwrap()),
            player: None,
            media_path: String::default(),
            no_scale: true,
            // stream_size_scale: 1.0,
            // seek_frac: 0.0,
        }
    }
}

impl AppUi {
    fn collapse_str(&self) -> &'static str {
        match self.collapse {
            true => "<",
            false => ">"
        }
    }

    pub fn run_app() {
        // let mut app = App::new();
        // app
        //     .add_plugins(AppUi::plugins())
        //     .add_plugins(EguiPlugin)
        //     .init_resource::<AppUi>()
        //     .add_systems(Startup, AppUi::startup)
        //     .add_systems(Update, AppUi::update);
        // app.run();

        let re = eframe::run_native("Door Player", eframe::NativeOptions::default(),
                                    Box::new(|_| Box::new(AppUi::default())),
        );
        if let Err(e) = re {
            log::error!("{}", e);
        }
    }

    fn compute_player_size(vedio_size: egui::Vec2, ui_size: egui::Vec2) -> egui::Vec2 {
        let mut re = egui::Vec2::splat(0.0);
        if ui_size.x > 0.0 && ui_size.y > 0.0 && vedio_size.x > 0.0 && vedio_size.y > 0.0 {
            let x_ = ui_size.x / vedio_size.x;
            let y_ = ui_size.y / vedio_size.y;
            if x_ > y_ {
                re.x = vedio_size.x * y_;
                re.y = ui_size.y;
            } else if x_ == y_ {
                re.x = ui_size.x;
                re.y = ui_size.y;
            } else {
                re.x = ui_size.x;
                re.y = vedio_size.y * x_;
            }
        }
        return re;
    }
}

impl eframe::App for AppUi {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // ctx.request_repaint();
        // if cfg!(debug_assertions) {
        //     ctx.set_debug_on_hover(true);
        // } else {
        //     ctx.set_debug_on_hover(false);
        // }
        let frame = egui::Frame::default();
        if !self.collapse {
            egui::SidePanel::right("right_panel").frame(frame.clone())
                .min_width(0.0)
                .resizable(true)
                .show(ctx, |ui| {
                    if ui.button("Open").clicked() {
                        if let Some(buf) = rfd::FileDialog::new().add_filter("videos", &["mp4"]).pick_file() {
                            let f = buf.as_path().to_string_lossy().to_string();
                            self.media_path = f;
                            if !self.media_path.is_empty() {
                                let f = self.media_path.clone();
                                // self.media_path = "".to_owned();
                                match Player::new(ctx, &f.replace("\"", "")).and_then(|p| p.with_audio(&mut self.audio_device.as_mut().unwrap())) {
                                    Ok(p) => {
                                        self.player = Some(p);
                                    }
                                    Err(e) => {
                                        log::error!("{}", e);
                                    }
                                }
                            }
                        }
                    }
                    if ui.button("Close").clicked() {
                        self.player = None;
                    }

                    ui.checkbox(&mut self.no_scale, "no scale");

                    if !self.media_path.is_empty() {
                        ui.label(self.media_path.clone());
                        let mut file = String::default();
                        if ui.button("Next file").clicked() {
                            file = AppUi::next_file(&self.media_path);
                        }
                        if ui.button("Pre file").clicked() {
                            file = AppUi::pre_file(&self.media_path);
                        }

                        if !file.is_empty() {
                            self.media_path = file;
                            let f = self.media_path.clone();
                            match Player::new(ctx, &f.replace("\"", "")).and_then(|p| p.with_audio(&mut self.audio_device.as_mut().unwrap())) {
                                Ok(p) => {
                                    self.player = Some(p);
                                }
                                Err(e) => {
                                    log::error!("{}", e);
                                }
                            }
                        }
                    }

                    ui.allocate_rect(ui.available_rect_before_wrap(), egui::Sense::hover());
                });
        }


        egui::CentralPanel::default().frame(frame)
            .show(ctx, |ui| {
                if let Some(player) = &mut self.player {
                    let p = {
                        if self.no_scale {
                            egui::Vec2::new(player.width as f32, player.height as f32)
                        } else {
                            AppUi::compute_player_size(egui::Vec2::new(player.width as f32, player.height as f32),
                                                       egui::Vec2::new(ui.min_rect().width(), ui.min_rect().height()))
                        }
                    };
                    ui.centered_and_justified(|ui| {
                        player.ui(ui, [p.x, p.y]);
                    });

                    AppUi::handle_key_player(ui, player);
                }

                let rect = {
                    const WIDTH: f32 = 30.0;
                    let right_center = egui::Pos2 {
                        x: ui.min_rect().right() - WIDTH / 2.0,
                        y: ui.min_rect().center().y,
                    };
                    egui::Rect::from_center_size(right_center, egui::Vec2::splat(WIDTH))
                };

                let button = ui.put(rect, egui::Button::new(self.collapse_str()).small());
                if button.clicked() {
                    self.collapse = !self.collapse;
                }
                // dont use the following code, the t.response.rect.height() == ui.max_rect().height()
                // let t = ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{
                //     if ui.button(app_ui.collapse_str()).clicked(){
                //         app_ui.collapse = !app_ui.collapse;
                //     }
                // });
                // log::info!("w:{}, h:{}  all: w: {}, h: {}", t.response.rect.width(),t.response.rect.height(), ui.max_rect().width(),ui.max_rect().height());
            });
    }
}