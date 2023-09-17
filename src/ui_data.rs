use bevy::app::PluginGroupBuilder;
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPlugin};

use crate::{AudioDevice, init_audio_device_default, Player};

#[derive(Resource)]
pub struct AppUi {
    collapse: bool,
    audio_device: Option<AudioDevice>,
    player: Option<Player>,

    media_path: String,
    stream_size_scale: f32,
    seek_frac: f32,
}

unsafe impl Send for AppUi {}

unsafe impl Sync for AppUi {}

impl Default for AppUi {
    fn default() -> Self {
        Self {
            collapse: true,
            audio_device: None,
            player: None,
            media_path: String::default(),
            stream_size_scale: 1.0,
            seek_frac: 0.0,
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
        let mut app = App::new();
        app
            .add_plugins(AppUi::plugins())
            .add_plugins(EguiPlugin)
            .init_resource::<AppUi>()
            .add_systems(Startup, AppUi::startup)
            .add_systems(Update, AppUi::update);
        app.run();
    }

    pub fn startup(mut app_ui: ResMut<AppUi>, mut _contexts: EguiContexts) {
        app_ui.audio_device = Some(init_audio_device_default().unwrap());
        // primary_window.single_mut().title = "Door Player".to_owned();
    }

    pub fn plugins() -> PluginGroupBuilder {
        DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Door Player".to_string(),
                ..default()
            }),
            ..default()
        })
    }
    pub fn update(
        mut app_ui: ResMut<AppUi>,
        mut contexts: EguiContexts,
    ) {
        let ctx = contexts.ctx_mut();
        ctx.request_repaint();
        // if cfg!(debug_assertions) {
        //     ctx.set_debug_on_hover(true);
        // } else {
        //     ctx.set_debug_on_hover(false);
        // }
        let frame = egui::Frame::default();
        if !app_ui.collapse {
            egui::SidePanel::right("right_panel").frame(frame.clone())
                .min_width(0.0)
                .resizable(true)
                .show(ctx, |ui| {
                    if ui.button("Open").clicked() {
                        if let Some(buf) = rfd::FileDialog::new().add_filter("videos", &["mp4"]).pick_file() {
                            let f = buf.as_path().to_string_lossy().to_string();
                            app_ui.media_path = f;
                        }
                    }
                    if ui.button("Close").clicked() {
                        app_ui.player = None;
                    }
                    ui.allocate_rect(ui.available_rect_before_wrap(), egui::Sense::hover());
                });
        }
        if !app_ui.media_path.is_empty() {
            let f = app_ui.media_path.clone();
            app_ui.media_path = "".to_owned();
            match Player::new(ctx, &f.replace("\"", "")).and_then(|p| p.with_audio(&mut app_ui.audio_device.as_mut().unwrap())) {
                Ok(p) => {
                    app_ui.player = Some(p);
                }
                Err(e) => {
                    log::error!("{}", e);
                }
            }
        }

        egui::CentralPanel::default().frame(frame)
            .show(ctx, |ui| {
                let stream_size_scale = app_ui.stream_size_scale;
                let seek_frac = app_ui.seek_frac;
                if let Some(player) = &mut app_ui.player {
                    player.ui(
                        ui,
                        [
                            player.width as f32 * stream_size_scale,
                            player.height as f32 * stream_size_scale,
                        ],
                    );
                }


                let rect = {
                    const WIDTH: f32 = 30.0;
                    let right_center = egui::Pos2 {
                        x: ui.min_rect().right() - WIDTH / 2.0,
                        y: ui.min_rect().center().y,
                    };
                    egui::Rect::from_center_size(right_center, egui::Vec2::splat(WIDTH))
                };

                let button = ui.put(rect, egui::Button::new(app_ui.collapse_str()).small());
                if button.clicked() {
                    app_ui.collapse = !app_ui.collapse;
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