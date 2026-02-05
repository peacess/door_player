use eframe::Frame;
use egui::{Context, Visuals};

fn main() {
    env_logger::init();
    let opt = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_decorations(false)
            .with_resizable(true)
            .with_transparent(false),
        ..Default::default()
    };
    if let Err(e) = eframe::run_native("Hide titlebar", opt, Box::new(|_cc| Ok(Box::new(App {})))) {
        log::error!("{}", e);
    }
}

struct App {}
impl eframe::App for App {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        let w = {
            let mut w = egui::Window::new("Hello");
            w = w.title_bar(true);
            w = w.resizable(true);
            w = w.movable(true);
            w
        };
        w.show(ctx, |ui| {
            ui.set_width(ui.available_width());
            ui.set_height(ui.available_height());
            ui.label("hide titlebar");
        });
    }

    fn clear_color(&self, _visuals: &Visuals) -> [f32; 4] {
        egui::Rgba::TRANSPARENT.to_array()
    }
}
