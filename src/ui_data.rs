use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

#[derive(Default, Resource)]
pub struct AppUi {
    left_id: f32,
    right_id: f32,
}

impl AppUi {
    pub fn update(
        mut app_ui: ResMut<AppUi>,
        mut contexts: EguiContexts,
    ) {
        let ctx = contexts.ctx_mut();
        app_ui.left_id = egui::SidePanel::left("left_panel")
            .resizable(true)
            .show(ctx, |ui| {
                ui.label("Left");
                ui.allocate_rect(ui.available_rect_before_wrap(), egui::Sense::hover());
            })
            .response
            .rect
            .width();
        app_ui.right_id = egui::CentralPanel::default()
            .show(ctx, |ui| {
                ui.label("Right");
                ui.allocate_rect(ui.available_rect_before_wrap(), egui::Sense::hover());
            })
            .response
            .rect
            .width();
    }
}