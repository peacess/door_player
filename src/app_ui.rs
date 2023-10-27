use std::{fs, path};
use std::path::PathBuf;

use eframe::Frame;
use egui::{Context, DroppedFile, Event, Key, PointerButton, Ui};

use crate::kits::Shared;
use crate::player::{CommandGo, CommandUi, Player, PlayerState};

pub struct AppUi {
    collapse: bool,
    player: Option<Player>,

    media_path: String,
    no_scale: bool,

    command_ui: Shared<CommandUi>,
    /// ui界面使用
    pub command_go_ui: Shared<CommandGo>,
}

impl AppUi {
    pub(crate) fn handle_key_player(&mut self, ui: &mut Ui, ctx: &egui::Context) {
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

            ui.input(|k| {
                for e in &k.events {
                    match e {
                        Event::Key { key, pressed: true, .. } => {
                            match key {
                                Key::ArrowLeft => {
                                    player.go_back_ui(&self.command_go_ui);
                                }
                                Key::ArrowRight => {
                                    player.go_ahead_ui(&self.command_go_ui);
                                }
                                Key::ArrowUp => {}
                                Key::ArrowDown => {}
                                Key::Space => {
                                    let state = player.player_state.get();
                                    match state {
                                        PlayerState::Stopped => {
                                            player.start();
                                        }
                                        PlayerState::Paused => {
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
                }
            });
        }

        ui.input(|k| {
            for e in &k.events {
                match e {
                    Event::Key { key, pressed: true, .. } => {
                        match key {
                            Key::PageDown => {
                                let file = AppUi::next_file(&self.media_path);
                                self.open_file(ctx, file.into());
                            }
                            Key::PageUp => {
                                let file = AppUi::pre_file(&self.media_path);
                                self.open_file(ctx, file.into());
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        });
    }

    pub(crate) fn handle_key_no_player(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        ui.input(|k| {
            for e in &k.events {
                match e {
                    Event::Key { key: Key::Space, pressed: true, .. } | Event::PointerButton { button: PointerButton::Primary, pressed: true, .. } => {
                        if let Some(buf) = Self::select_file() {
                            self.open_file(ctx, buf);
                        }
                    }
                    _ => {}
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

    fn handle_command_ui(&mut self, _frame: &mut Frame) {
        {
            let cmd = self.command_ui.get();
            self.command_ui.set(CommandUi::None);

            match cmd {
                CommandUi::None => {}
                CommandUi::FullscreenToggle => _frame.set_fullscreen(!_frame.info().window_info.fullscreen),
                CommandUi::FullscreenTrue => _frame.set_fullscreen(true),
                CommandUi::FullscreenFalse => _frame.set_fullscreen(true),
                CommandUi::MaximizedToggle => _frame.set_maximized(!_frame.info().window_info.maximized),
                CommandUi::MaximizedTrue => _frame.set_maximized(true),
                CommandUi::MaximizedFalse => _frame.set_maximized(false),
                CommandUi::MinimizedToggle => _frame.set_minimized(!_frame.info().window_info.minimized),
                CommandUi::MinimizedTrue => _frame.set_minimized(true),
                CommandUi::MinimizedFalse => _frame.set_minimized(false),
                CommandUi::Close => { _frame.close(); }
            }
        }
    }

    fn select_file() -> Option<PathBuf> {
        rfd::FileDialog::new().add_filter("videos", &["mp4", "mkv"]).pick_file()
    }

    fn open_file(&mut self, ctx: &Context, buf: PathBuf) {
        self.media_path = buf.to_string_lossy().to_string();
        if !self.media_path.is_empty() {
            let texture_handle = match &self.player {
                Some(p) => p.texture_handle.clone(),
                None => Player::default_texture_handle(ctx),
            };
            match Player::new(ctx, texture_handle, self.command_ui.clone(), &self.media_path) {
                Ok(p) => {
                    if let Some(mut p) = self.player.replace(p) {
                        p.stop();
                    }
                }
                Err(e) => {
                    log::error!("{}", e);
                }
            }
        }
    }
}

unsafe impl Send for AppUi {}

unsafe impl Sync for AppUi {}

impl Default for AppUi {
    fn default() -> Self {
        Self {
            collapse: true,
            player: None,
            media_path: String::default(),
            no_scale: false,
            command_ui: Shared::new(CommandUi::None),
            command_go_ui: Shared::new(CommandGo::Packet(1)),
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
        let mut ops = eframe::NativeOptions::default();
        ops.centered = true;
        let re = eframe::run_native("Door Player", ops,
                                    Box::new(|_| Box::new(AppUi::default())), );
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
        // if cfg!(debug_assertions) {
        //     ctx.set_debug_on_hover(true);
        // } else {
        //     ctx.set_debug_on_hover(false);
        // }

        self.handle_command_ui(_frame);

        let frame = egui::Frame::default();
        if !self.collapse {
            egui::SidePanel::right("right_panel").frame(frame.clone())
                .min_width(0.0)
                .resizable(true)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        if ui.button("Open").clicked() {
                            if let Some(buf) = Self::select_file() {
                                self.open_file(ctx, buf);
                            }
                        }
                    });
                    ui.horizontal(|ui| {
                        if ui.button("Stop").clicked() {
                            self.player = None;
                        }
                    });
                    ui.checkbox(&mut self.no_scale, "no scale");

                    if !self.media_path.is_empty() {
                        ui.label(self.media_path.clone());
                        ui.horizontal(|ui| {
                            if ui.button("Pre file").clicked() {
                                let file = AppUi::pre_file(&self.media_path);
                                self.open_file(ctx, file.into());
                            }
                        });
                        ui.horizontal(|ui| {
                            if ui.button("ReOpen").clicked() {
                                self.open_file(ctx, self.media_path.clone().into());
                            }
                        });
                        ui.horizontal(|ui| {
                            if ui.button("Next file").clicked() {
                                let file = AppUi::next_file(&self.media_path);
                                self.open_file(ctx, file.into());
                            }
                        });
                    }
                    if let Some(player) = &mut self.player {
                        ui.horizontal(|ui| {
                            let (mut go_amount, mut go_packet) = match self.command_go_ui.get() {
                                CommandGo::Packet(v) => (v, true),
                                _ => (10, false),
                            };
                            if ui.checkbox(&mut go_packet, "go packets: ").changed() {
                                if go_packet {
                                    self.command_go_ui.set(CommandGo::Packet(go_amount));
                                } else {
                                    self.command_go_ui.set(CommandGo::None);
                                }
                            }
                            if go_packet {
                                let mut str_amount = format!("{}", go_amount);
                                if ui.add(egui::TextEdit::singleline(&mut str_amount)).changed() {
                                    if let Ok(v) = str_amount.parse() {
                                        go_amount = v;
                                        self.command_go_ui.set(CommandGo::Packet(go_amount));
                                    }
                                }
                            }
                        });
                        ui.horizontal(|ui| {
                            let (mut go_amount, mut go_frame) = match self.command_go_ui.get() {
                                CommandGo::Frame(v) => (v, true),
                                _ => (5, false),
                            };
                            if ui.checkbox(&mut go_frame, "go frames: ").changed() {
                                if go_frame {
                                    self.command_go_ui.set(CommandGo::Frame(go_amount));
                                } else {
                                    self.command_go_ui.set(CommandGo::None);
                                }
                            }
                            if go_frame {
                                let mut str_amount = format!("{}", go_amount);
                                if ui.add(egui::TextEdit::singleline(&mut str_amount)).changed() {
                                    if let Ok(v) = str_amount.parse() {
                                        go_amount = v;
                                        self.command_go_ui.set(CommandGo::Frame(go_amount));
                                    }
                                }
                            }
                        });

                        ui.horizontal(|ui| {
                            let (mut go_amount, mut seek_ms) = match self.command_go_ui.get() {
                                CommandGo::GoMs(v) => (v, true),
                                _ => (5000, false),
                            };
                            if ui.checkbox(&mut seek_ms, "go ms: ").changed() {
                                if seek_ms {
                                    self.command_go_ui.set(CommandGo::GoMs(go_amount));
                                } else {
                                    self.command_go_ui.set(CommandGo::None);
                                }
                            }
                            if seek_ms {
                                let mut str_amount = format!("{}", go_amount);
                                if ui.add(egui::TextEdit::singleline(&mut str_amount)).changed() {
                                    if let Ok(v) = str_amount.parse() {
                                        go_amount = v;
                                        self.command_go_ui.set(CommandGo::GoMs(go_amount));
                                    }
                                }
                            }
                        });

                        ui.horizontal(|ui| {
                            if ui.button("Go").clicked() {
                                player.go_ahead_ui(&self.command_go_ui);
                            }
                        });
                    }

                    ui.allocate_rect(ui.available_rect_before_wrap(), egui::Sense::hover());
                });
        }

        egui::CentralPanel::default().frame(frame)
            .show(ctx, |ui| {
                {
                    ui.input(|state| {
                        if !state.raw.dropped_files.is_empty() {
                            match state.raw.dropped_files.first() {
                                Some(DroppedFile { path: Some(first), .. }) => {
                                    self.open_file(ctx, first.clone())
                                }
                                _ => {}
                            }
                        }
                        for e in &state.events {
                            match e {
                                Event::Key { key: Key::Escape, pressed: true, .. } => {
                                    self.command_ui.set(CommandUi::Close);
                                    break;
                                }
                                _ => {}
                            }
                        }
                    });
                }
                if self.player.is_some() {
                    self.handle_key_player(ui, ctx);
                }
                let none = ui.allocate_rect(ui.available_rect_before_wrap(), egui::Sense::hover());

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

                if !button.hovered() && self.player.is_none() && none.hovered() {
                    self.handle_key_no_player(ui, ctx);
                }
            });
    }
}