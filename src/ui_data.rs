use bevy::app::PluginGroupBuilder;
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPlugin};

#[derive(Resource)]
pub struct AppUi {
    collapse: bool,
}

impl Default for AppUi {
    fn default() -> Self {
        Self {
            collapse: true,
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

    pub fn make_app() -> App {
        let mut app = App::new();
        app
            .add_plugins(AppUi::plugins())
            .add_plugins(EguiPlugin)
            .init_resource::<AppUi>()
            .add_systems(Startup, AppUi::startup)
            .add_systems(Update, AppUi::update);
        app
    }

    pub fn startup(mut _app_ui: ResMut<AppUi>, mut _contexts: EguiContexts) {
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
        if cfg!(debug_assertions) {
            ctx.set_debug_on_hover(true);
        } else {
            ctx.set_debug_on_hover(false);
        }
        let frame = egui::Frame::default();
        if !app_ui.collapse {
            egui::SidePanel::right("left_panel").frame(frame.clone())
                .min_width(0.0)
                .resizable(true)
                .show(ctx, |ui| {
                    ui.allocate_rect(ui.available_rect_before_wrap(), egui::Sense::hover());
                });
        }

        egui::CentralPanel::default().frame(frame)
            .show(ctx, |ui| {
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