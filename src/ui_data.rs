use crate::{AudioDevice, init_audio_device_default, Player};

pub struct AppUi {
    collapse: bool,
    audio_device: Option<AudioDevice>,
    player: Option<Player>,

    media_path: String,
    // stream_size_scale: f32,
    // seek_frac: f32,
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
                        }
                    }
                    if ui.button("Close").clicked() {
                        self.player = None;
                    }
                    ui.allocate_rect(ui.available_rect_before_wrap(), egui::Sense::hover());
                });
        }
        if !self.media_path.is_empty() {
            let f = self.media_path.clone();
            self.media_path = "".to_owned();
            match Player::new(ctx, &f.replace("\"", "")).and_then(|p| p.with_audio(&mut self.audio_device.as_mut().unwrap())) {
                Ok(p) => {
                    self.player = Some(p);
                }
                Err(e) => {
                    log::error!("{}", e);
                }
            }
        }

        egui::CentralPanel::default().frame(frame)
            .show(ctx, |ui| {
                if let Some(player) = &mut self.player {
                    let p = AppUi::compute_player_size(egui::Vec2::new(player.width as f32, player.height as f32),
                                                       egui::Vec2::new(ui.min_rect().width(), ui.min_rect().height()));
                    ui.centered_and_justified(|ui| {
                        player.ui(ui, [p.x, p.y]);
                    });
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