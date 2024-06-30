pub struct Eguis {}

impl Eguis {
    pub fn front_painter(ctx: &egui::Context) -> egui::Painter {
        ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("Foreground")))
    }

    pub fn ui_from_painter(painter: &egui::Painter) -> egui::Ui {
        let ctx = painter.ctx();
        let available_rect = ctx.available_rect();
        let layer_id = egui::LayerId::background();
        let clip_rect = ctx.screen_rect();
        egui::Ui::new(ctx.clone(), layer_id, layer_id.id, available_rect, clip_rect)
    }
}
