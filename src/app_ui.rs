use std::default::Default;
use std::path::PathBuf;
use std::{fs, path};

use eframe::Theme;

use crate::kits::Shared;
use crate::player::{kits::FfmpegKit, CommandGo, CommandUi, Player, PlayerState};
use crate::{kits, player};

pub struct AppUi {
    collapse: bool,
    player: Option<Player>,

    media_path: String,
    no_scale: bool,
    auto_play_next: bool,
    title: String,

    command_ui: Shared<CommandUi>,
    /// ui界面使用
    pub command_go_ui: Shared<CommandGo>,
}

impl AppUi {
    pub(crate) fn handle_key_player(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        for e in &ui.input(|k| k.events.clone()) {
            if let Some(player) = &mut self.player {
                match e {
                    egui::Event::Key {
                        key, pressed: true, modifiers, ..
                    } => match key {
                        egui::Key::Escape => {
                            self.command_ui.set(CommandUi::Close);
                        }
                        egui::Key::ArrowLeft => {
                            player.go_back_ui(&self.command_go_ui);
                        }
                        egui::Key::ArrowRight => {
                            player.go_ahead_ui(&self.command_go_ui);
                        }
                        egui::Key::Tab => {
                            if modifiers.ctrl {
                                player.tab_seek_ms = player.elapsed_ms();
                            } else {
                                player.tab_seek();
                            }
                        }
                        egui::Key::ArrowUp | egui::Key::Plus => {
                            let v = player::kits::Volume::plus_volume(player.audio_volume.get());
                            player.audio_volume.set(v);
                        }
                        egui::Key::ArrowDown | egui::Key::Minus => {
                            let v = player::kits::Volume::minus_volume(player.audio_volume.get());
                            player.audio_volume.set(v);
                        }
                        egui::Key::Space => {
                            player.clicked_player();
                        }
                        egui::Key::PageDown => {
                            let file = AppUi::next_file(&self.media_path);
                            self.open_file(ctx, file.into());
                        }
                        egui::Key::PageUp => {
                            let file = AppUi::pre_file(&self.media_path);
                            self.open_file(ctx, file.into());
                        }
                        egui::Key::F1 => {
                            self.command_ui.set(CommandUi::FullscreenToggle);
                        }
                        _ => {}
                    },
                    _ => {}
                }
            } else {
                match e {
                    egui::Event::Key { key, pressed: true, .. } => match key {
                        egui::Key::Space => {
                            if let Some(buf) = Self::select_file() {
                                self.open_file(ctx, buf);
                            }
                        }
                        egui::Key::Escape => {
                            self.command_ui.set(CommandUi::Close);
                        }
                        egui::Key::F1 => {
                            self.command_ui.set(CommandUi::FullscreenToggle);
                        }
                        _ => {}
                    },
                    egui::Event::PointerButton {
                        button: egui::PointerButton::Primary,
                        pressed: false,
                        ..
                    } => {
                        if self.player.is_none() {
                            if ui.rect_contains_pointer(ctx.available_rect()) {
                                if let Some(buf) = Self::select_file() {
                                    self.open_file(ctx, buf);
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    pub(crate) fn next_file(file: &str) -> String {
        let path_file = path::PathBuf::from(file);
        if path_file.file_name().is_none() {
            return String::default();
        }
        let mut files = Vec::new();
        match fs::read_dir(path_file.parent().unwrap()) {
            Err(e) => {
                log::error!("{}", e);
                return String::default();
            }
            Ok(read_dir) => {
                for f in read_dir.flatten() {
                    files.push(f.file_name());
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
        String::default()
    }

    pub(crate) fn pre_file(file: &str) -> String {
        let path_file = path::PathBuf::from(file);
        if path_file.file_name().is_none() {
            return String::default();
        }
        let mut files = Vec::new();
        match fs::read_dir(path_file.parent().unwrap()) {
            Err(e) => {
                log::error!("{}", e);
                return String::default();
            }
            Ok(read_dir) => {
                for f in read_dir.flatten() {
                    files.push(f.file_name());
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
        String::default()
    }

    fn handle_command_ui(&mut self, ctx: &egui::Context) {
        let cmd = self.command_ui.get();
        self.command_ui.set(CommandUi::None);
        if cmd == CommandUi::None {
            //check play finish
            if self.auto_play_next {
                if let Some(p) = &self.player {
                    if p.play_ctrl.video_finished() {
                        let file = AppUi::next_file(&self.media_path);
                        if self.open_file(ctx, file.into()) {
                            if let Some(p) = &mut self.player {
                                p.start();
                            }
                        }
                    }
                }
            }
            return;
        }
        let view = ctx.input(|c| c.viewport().clone());
        match cmd {
            CommandUi::None => {}
            CommandUi::FullscreenToggle => {
                let b = view.fullscreen.unwrap_or_default();
                ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!b));
                if !b {
                    self.collapse = true;
                }
            }
            CommandUi::FullscreenTrue => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
                self.collapse = true;
            }
            CommandUi::FullscreenFalse => ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false)),
            CommandUi::MaximizedToggle => {
                let b = view.maximized.unwrap_or_default();
                ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!b));
            }
            CommandUi::MaximizedTrue => ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(true)),
            CommandUi::MaximizedFalse => ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(false)),
            CommandUi::MinimizedToggle => {
                let b = view.minimized.unwrap_or_default();
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(!b));
            }
            CommandUi::MinimizedTrue => ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true)),
            CommandUi::MinimizedFalse => ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false)),
            CommandUi::Close => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
        }
    }

    fn title_bar(&mut self, ctx: &egui::Context, frame: egui::Frame) {
        use egui::{Align, Button, Layout, RichText};
        if ctx.input(|c| c.viewport().fullscreen.unwrap_or_default()) {
            return;
        }

        let title_bar_height = 32.0;
        egui::TopBottomPanel::top("title_bar_frame")
            .frame(frame)
            .show_separator_line(false)
            .exact_height(title_bar_height)
            .show(ctx, |ui| {
                let title_bar_rect = {
                    let mut rect = ui.max_rect();
                    rect.max.y = rect.min.y + title_bar_height;
                    rect
                };
                let title_bar_response = ui.interact(title_bar_rect, egui::Id::new("title_bar"), egui::Sense::click_and_drag());

                ui.painter().text(
                    title_bar_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    &self.title,
                    egui::FontId::proportional(16.0),
                    ui.style().visuals.text_color(),
                );

                if title_bar_response.double_clicked() {
                    //the double click do not work,why?
                    let is_full = ui.input(|i| i.viewport().fullscreen.unwrap_or(false));
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Fullscreen(!is_full));
                } else if title_bar_response.is_pointer_button_down_on() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }

                ui.allocate_ui_at_rect(title_bar_rect, |ui| {
                    let button_height = 16.0;
                    let space = 8.0;
                    let close_text = " ❌ ";
                    let maximize_text = " 🗖 ";
                    let minimize_text = " 🗕 ";
                    let restore_text = " 🗗 ";
                    ui.columns(2, |cols| {
                        cols.get_mut(0).expect("").with_layout(Layout::left_to_right(Align::Center), |ui| {
                            ui.visuals_mut().button_frame = false;
                            ui.add_space(space);
                            let close_response = ui
                                .add(Button::new(RichText::new(close_text).size(button_height)))
                                .on_hover_text("Close the window");
                            if close_response.clicked() {
                                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                            }

                            let is_maximized = ui.input(|i| i.viewport().maximized.unwrap_or(false));
                            if is_maximized {
                                let maximized_response = ui
                                    .add(Button::new(RichText::new(restore_text).size(button_height)))
                                    .on_hover_text("Restore window");
                                if maximized_response.clicked() {
                                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(false));
                                }
                            } else {
                                let maximized_response = ui
                                    .add(Button::new(RichText::new(maximize_text).size(button_height)))
                                    .on_hover_text("Maximize window");
                                if maximized_response.clicked() {
                                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(true));
                                }
                            }

                            let minimized_response = ui
                                .add(Button::new(RichText::new(minimize_text).size(button_height)))
                                .on_hover_text("Minimize the window");
                            if minimized_response.clicked() {
                                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                            }
                        });
                        cols.get_mut(1).expect("").with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.visuals_mut().button_frame = false;
                            ui.add_space(space);
                            let close_response = ui
                                .add(Button::new(RichText::new(close_text).size(button_height)))
                                .on_hover_text("Close the window");
                            if close_response.clicked() {
                                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                            }

                            let is_maximized = ui.input(|i| i.viewport().maximized.unwrap_or(false));
                            if is_maximized {
                                let maximized_response = ui
                                    .add(Button::new(RichText::new(restore_text).size(button_height)))
                                    .on_hover_text("Restore window");
                                if maximized_response.clicked() {
                                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(false));
                                }
                            } else {
                                let maximized_response = ui
                                    .add(Button::new(RichText::new(maximize_text).size(button_height)))
                                    .on_hover_text("Maximize window");
                                if maximized_response.clicked() {
                                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(true));
                                }
                            }

                            let minimized_response = ui
                                .add(Button::new(RichText::new(minimize_text).size(button_height)))
                                .on_hover_text("Minimize the window");
                            if minimized_response.clicked() {
                                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                            }
                        });
                    });
                });
            });
    }

    fn main_frame(&mut self, ctx: &egui::Context, frame: egui::Frame) {
        self.right_panel(ctx, frame);
        egui::CentralPanel::default().frame(frame).show(ctx, |rigth_ui| {
            {
                let file = rigth_ui.input(|s| match s.raw.dropped_files.first() {
                    Some(egui::DroppedFile { path: Some(first), .. }) => Some(first.clone()),
                    _ => None,
                });
                if let Some(f) = file {
                    self.open_file(ctx, f);
                }
            }
            if let Some(player) = &mut self.player {
                let p = {
                    if self.no_scale {
                        egui::Vec2::new(player.width as f32, player.height as f32)
                    } else {
                        AppUi::compute_player_size(
                            egui::Vec2::new(player.width as f32, player.height as f32),
                            egui::Vec2::new(rigth_ui.min_rect().width(), rigth_ui.min_rect().height()),
                        )
                    }
                };
                rigth_ui.centered_and_justified(|ui| player.ui(ui, [p.x, p.y]));
            }

            let rect = {
                const WIDTH: f32 = 30.0;
                let right_center = egui::Pos2 {
                    x: rigth_ui.min_rect().right() - WIDTH / 2.0,
                    y: rigth_ui.min_rect().center().y,
                };
                egui::Rect::from_center_size(right_center, egui::Vec2::splat(WIDTH))
            };
            let button = rigth_ui.put(rect, egui::Button::new(self.collapse_str()).small());
            if button.clicked() {
                self.collapse = !self.collapse;
            } else {
                self.handle_key_player(rigth_ui, ctx);
            }
        });
    }

    fn select_file() -> Option<PathBuf> {
        let names = FfmpegKit::demuxers();
        // &["mp4", "mkv", "ogg", "webm", "wmv", "mov", "avi", "mp3", "flv"]
        rfd::FileDialog::new().add_filter("videos", &names).pick_file()
    }

    fn open_file(&mut self, ctx: &egui::Context, buf: PathBuf) -> bool {
        self.media_path = buf.to_string_lossy().to_string();
        if !self.media_path.is_empty() {
            //create a new texture, do not use the old one
            let texture_handle = Player::default_texture_handle(ctx);
            match Player::new(ctx, texture_handle, self.command_ui.clone(), &self.media_path) {
                Ok(mut new_player) => {
                    if let Some(old_player) = &self.player {
                        new_player.tab_seek_ms = old_player.tab_seek_ms;
                        new_player.audio_volume.set(old_player.audio_volume.get());
                    }
                    self.player = Some(new_player);
                    true
                }
                Err(e) => {
                    log::error!("{}", e);
                    false
                }
            }
        } else {
            false
        }
    }

    fn right_panel(&mut self, ctx: &egui::Context, frame: egui::Frame) {
        if !self.collapse {
            egui::SidePanel::right("right_panel")
                .frame(frame)
                .min_width(0.0)
                .resizable(true)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        if ui.button("Exit").clicked() {
                            self.command_ui.set(CommandUi::Close);
                        }
                    });
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
                    ui.horizontal(|ui| {
                        if ui.button("Pause").clicked() {
                            if let Some(p) = &mut self.player {
                                p.pause();
                            }
                        }
                    });
                    ui.horizontal(|ui| {
                        if ui.button("Start").clicked() {
                            if let Some(p) = &mut self.player {
                                p.start();
                            }
                        }
                    });
                    ui.checkbox(&mut self.no_scale, "no scale");

                    if !self.media_path.is_empty() {
                        ui.horizontal(|ui| {
                            if ui.button("Full Screen").clicked() {
                                self.command_ui.set(CommandUi::FullscreenToggle);
                            }
                        });
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

                        ui.horizontal(|ui| {
                            if ui.button("Tab seek: ").clicked() {
                                player.tab_seek();
                            }
                            let mut str_amount = format!("{}", player.tab_seek_ms);
                            if ui.add(egui::TextEdit::singleline(&mut str_amount)).changed() {
                                if let Ok(v) = str_amount.parse() {
                                    player.tab_seek_ms = v;
                                }
                            }
                        });

                        ui.horizontal(|ui| {
                            if ui.button(" + ").on_hover_text("+ Volume").clicked() {
                                let v = player::kits::Volume::plus_volume(player.audio_volume.get());
                                player.audio_volume.set(v);
                            }
                            let mut s = player::kits::Volume::int_volume(player.audio_volume.get());
                            if ui.add(egui::Slider::new(&mut s, 0..=1000).suffix("")).changed() {
                                let v = player::kits::Volume::f64_volume(s);
                                player.audio_volume.set(v);
                            };
                            if ui.button(" - ").on_hover_text("- Volume").clicked() {
                                let v = player::kits::Volume::minus_volume(player.audio_volume.get());
                                player.audio_volume.set(v);
                            }
                        });

                        ui.horizontal(|ui| {
                            ui.checkbox(&mut self.auto_play_next, "Auto Play Next");
                        });
                    }

                    ui.allocate_rect(ui.available_rect_before_wrap(), egui::Sense::hover());
                });
        }
    }
}

unsafe impl Send for AppUi {}

unsafe impl Sync for AppUi {}

impl AppUi {
    fn new(cc: &eframe::CreationContext<'_>, title: &str) -> Self {
        Self::set_font(&cc.egui_ctx);
        Self {
            collapse: true,
            player: None,
            media_path: String::default(),
            no_scale: false,
            auto_play_next: false,
            title: title.to_owned(),
            command_ui: Shared::new(CommandUi::None),
            command_go_ui: Shared::new(CommandGo::GoMs(5000)),
        }
    }
    fn collapse_str(&self) -> &'static str {
        match self.collapse {
            true => "<",
            false => ">",
        }
    }

    pub fn run_app() {
        let title = "Door Player";
        let ops = eframe::NativeOptions {
            centered: true,
            renderer: eframe::Renderer::Wgpu,
            // follow_system_theme: false,
            default_theme: Theme::Dark,
            viewport: egui::ViewportBuilder {
                title: Some(title.to_string()),
                // decorations: Some(true),
                resizable: Some(true),
                active: Some(true),
                // window_level: Some(egui::WindowLevel::AlwaysOnTop),
                ..Default::default()
            },
            ..Default::default()
        };

        let re = eframe::run_native(title, ops, Box::new(|cc| Ok(Box::new(AppUi::new(cc, title)))));
        if let Err(e) = re {
            log::error!("{:?}", e);
        }
    }

    fn compute_player_size(video_size: egui::Vec2, ui_size: egui::Vec2) -> egui::Vec2 {
        let mut re = egui::Vec2::splat(0.0);
        if ui_size.x > 0.0 && ui_size.y > 0.0 && video_size.x > 0.0 && video_size.y > 0.0 {
            let x_ = ui_size.x / video_size.x;
            let y_ = ui_size.y / video_size.y;
            if x_ > y_ {
                re.x = video_size.x * y_;
                re.y = ui_size.y;
            } else if x_ == y_ {
                re.x = ui_size.x;
                re.y = ui_size.y;
            } else {
                re.x = ui_size.x;
                re.y = video_size.y * x_;
            }
        }
        re
    }
    /// set the font to support chinese
    fn set_font(ctx: &egui::Context) {
        let ctx: egui::Context = ctx.clone();
        std::thread::spawn(move || {
            let font_file = {
                match kits::fonts::get_font() {
                    Err(e) => {
                        log::error!("{}", e);
                        return;
                    }
                    Ok(p) => p,
                }
            };
            let mut fonts = egui::FontDefinitions::default();
            let font_name = String::from(font_file.file_stem().expect("").to_string_lossy());
            let bs = match fs::read(font_file) {
                Err(e) => {
                    log::error!("{}", e);
                    return;
                }
                Ok(t) => t,
            };
            if !bs.is_empty() {
                fonts.font_data.insert(font_name.clone(), egui::FontData::from_owned(bs));
                fonts.families.get_mut(&egui::FontFamily::Proportional).expect("").insert(0, font_name.clone());
                fonts.families.get_mut(&egui::FontFamily::Monospace).expect("").push(font_name.clone());
                if let Err(e) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    ctx.set_fonts(fonts);
                    return 0;
                })) {
                    log::error!("{:?}", e);
                }
            }
        });
        // let mut fonts = egui::FontDefinitions::default();
        // let font_name = "文泉驿正黑".to_string();
        // // let bs = include_bytes!("../assets/fonts/文泉驿正黑.ttc");
        // let bs = fs::read("/home/peace/gopath/src/peacess/door_player/assets/fonts/文泉驿正黑.ttc").expect("");
        // if !bs.is_empty() {
        //     fonts.font_data.insert(font_name.clone(), egui::FontData::from_owned(bs.to_vec()));
        //     fonts.families.get_mut(&egui::FontFamily::Proportional).expect("").insert(0, font_name.clone());
        //     fonts.families.get_mut(&egui::FontFamily::Monospace).expect("").push(font_name.clone());
        //     ctx.set_fonts(fonts);
        // }
    }
}

impl eframe::App for AppUi {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_command_ui(ctx);
        let frame = egui::Frame::default();
        // self.title_bar(ctx, frame);
        self.main_frame(ctx, frame);
    }
}
