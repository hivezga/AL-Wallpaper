mod model;
mod theme;
mod apply;
mod app;

use eframe::egui;

fn main() -> eframe::Result<()> {
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1180.0, 800.0])
        .with_min_inner_size([840.0, 560.0])
        .with_app_id("al-wallpaper")
        .with_title("Azur Lane — Live2D Wallpaper");
    if let Ok(icon) = eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png")) {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    eframe::run_native(
        "al-wallpaper",
        options,
        Box::new(|cc| Ok(Box::new(app::AppState::new(cc)))),
    )
}
