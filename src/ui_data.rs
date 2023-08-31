use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

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

        if !app_ui.collapse {
            egui::SidePanel::right("left_panel")
                .min_width(0.0)
                .resizable(true)
                .show(ctx, |ui| {
                    ui.allocate_rect(ui.available_rect_before_wrap(), egui::Sense::hover());
                })
                .response
                .rect
                .width();
        }
        egui::CentralPanel::default()
            .show(ctx, |ui| {
                let re = {
                    let mut left_center = ui.min_rect().center();
                    left_center.x = ui.min_rect().right() - 15.0;
                    egui::Rect::from_center_size(left_center, egui::Vec2::splat(30.0))
                };
                let button = ui.put(re, egui::Button::new(app_ui.collapse_str()).small());
                if button.clicked() {
                    app_ui.collapse = !app_ui.collapse;
                }
            })
            .response
            .rect
            .width();
    }
}