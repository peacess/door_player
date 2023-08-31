use bevy::prelude::*;
use bevy_egui::EguiPlugin;

use door_player::AppUi;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(EguiPlugin)
        .init_resource::<AppUi>()
        .add_systems(Update, AppUi::update)
        .run();
}
