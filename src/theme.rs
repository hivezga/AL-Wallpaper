use crate::model::{hex, scale, Palette};
use eframe::egui::{Color32, Stroke, Visuals};

pub fn neutral_visuals() -> Visuals {
    let mut v = Visuals::dark();
    let bg = hex("#0f1115");
    let panel = hex("#191c22");
    v.panel_fill = bg;
    v.window_fill = bg;
    v.extreme_bg_color = hex("#0b0d11");
    v.faint_bg_color = panel;
    v.override_text_color = Some(hex("#e6e8ec"));
    v.widgets.inactive.bg_fill = panel;
    v.widgets.inactive.weak_bg_fill = panel;
    v.widgets.hovered.bg_fill = hex("#2a2f38");
    v.widgets.active.bg_fill = hex("#343b46");
    v
}

pub fn faction_visuals(p: &Palette) -> Visuals {
    let bg = hex(&p.bg);
    let panel = hex(&p.panel);
    let accent = hex(&p.accent);
    let text = hex(&p.text);
    let mut v = Visuals::dark();
    v.panel_fill = bg;
    v.window_fill = bg;
    v.extreme_bg_color = scale(bg, 0.7);
    v.faint_bg_color = panel;
    v.override_text_color = Some(text);
    v.selection.bg_fill = scale(accent, 0.5);
    v.selection.stroke = Stroke::new(1.0, accent);
    v.hyperlink_color = accent;
    v.widgets.inactive.bg_fill = panel;
    v.widgets.inactive.weak_bg_fill = panel;
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, text);
    v.widgets.hovered.bg_fill = scale(accent, 0.45);
    v.widgets.hovered.weak_bg_fill = scale(accent, 0.45);
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, Color32::WHITE);
    v.widgets.active.bg_fill = scale(accent, 0.65);
    v.widgets.active.weak_bg_fill = scale(accent, 0.65);
    v.widgets.noninteractive.bg_fill = panel;
    v
}
