//! Fleet Registry visual system: bundled type, a gunmetal-steel chrome, and
//! per-faction accent visuals. The faction accent is the only saturated colour
//! on screen — everything else is steel, so the fleet reads at a glance.

use crate::model::{hex, scale, Palette};
use eframe::egui::{
    self, Color32, FontData, FontDefinitions, FontFamily, FontId, Rounding, Stroke, Visuals,
};

/// Steel chrome palette — shared by every screen regardless of faction.
pub mod col {
    use eframe::egui::Color32 as C;
    pub const INK: C = C::from_rgb(0x0b, 0x0e, 0x12); // deepest ground / behind panels
    pub const STEEL: C = C::from_rgb(0x13, 0x18, 0x20); // panel fill
    pub const STEEL_HI: C = C::from_rgb(0x1b, 0x22, 0x2d); // raised plate
    pub const RIVET: C = C::from_rgb(0x2c, 0x37, 0x45); // hairlines / rules
    pub const CHALK: C = C::from_rgb(0xe8, 0xed, 0xf3); // primary text
    pub const HAZE: C = C::from_rgb(0x8b, 0x99, 0xa8); // muted text
    pub const HAZE_DIM: C = C::from_rgb(0x56, 0x62, 0x70); // faintest labels
    pub const GOLD: C = C::from_rgb(0xe7, 0xc5, 0x58); // oath
    pub const SIGNAL: C = C::from_rgb(0x53, 0xb8, 0x74); // "on station" green
    pub const NEUTRAL_ACCENT: C = C::from_rgb(0x6f, 0x9b, 0xb8); // fleet-select accent
}

// ---- type roles ----------------------------------------------------------

fn name(s: &str) -> FontFamily {
    FontFamily::Name(s.into())
}
/// Condensed display — ship names, big titles (painted-hull lettering).
pub fn oswald(size: f32) -> FontId {
    FontId::new(size, name("oswald"))
}
pub fn oswald_b(size: f32) -> FontId {
    FontId::new(size, name("oswald_b"))
}
/// Stencil — pennant/hull codes only. The one place stencil is meaningful.
pub fn stencil(size: f32) -> FontId {
    FontId::new(size, name("stencil"))
}
/// Body sans.
pub fn plex(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}
pub fn plex_it(size: f32) -> FontId {
    FontId::new(size, name("plex_it"))
}
/// Technical readout — data, counts, eyebrows.
pub fn mono(size: f32) -> FontId {
    FontId::new(size, FontFamily::Monospace)
}

/// Register the bundled faces, keeping egui's emoji/symbol fallbacks so glyphs
/// like ⚭ ★ ⟳ still render.
pub fn install_fonts(ctx: &egui::Context) {
    let mut f = FontDefinitions::default();
    macro_rules! load {
        ($k:expr, $file:expr) => {
            f.font_data.insert(
                $k.to_owned(),
                FontData::from_static(include_bytes!(concat!("../assets/fonts/", $file))),
            );
        };
    }
    load!("plex", "IBMPlexSans-Regular.ttf");
    load!("plex_it", "IBMPlexSans-Italic.ttf");
    load!("plex_mono", "IBMPlexMono-Medium.ttf");
    load!("oswald", "Oswald-Medium.ttf");
    load!("oswald_b", "Oswald-Bold.ttf");
    load!("stencil", "StardosStencil-Bold.ttf");
    // DejaVu Sans: broad UI-symbol coverage (◂ ⚭ ⚡ ✓ …) the display faces lack
    load!("dejavu", "DejaVuSans.ttf");

    // original defaults carry the emoji fallbacks; splice DejaVu in for symbol glyphs
    let mut fb = f.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    fb.insert(0, "dejavu".to_owned());
    let mut mono_fb = f.families.get(&FontFamily::Monospace).cloned().unwrap_or_default();
    mono_fb.insert(0, "dejavu".to_owned());
    let with = |first: &str| -> Vec<String> {
        std::iter::once(first.to_owned()).chain(fb.iter().cloned()).collect()
    };

    f.families.insert(FontFamily::Proportional, with("plex"));
    f.families.insert(
        FontFamily::Monospace,
        std::iter::once("plex_mono".to_owned()).chain(mono_fb).collect(),
    );
    f.families.insert(name("oswald"), with("oswald"));
    f.families.insert(name("oswald_b"), with("oswald_b"));
    f.families.insert(name("stencil"), with("stencil"));
    f.families.insert(name("plex_it"), with("plex_it"));
    ctx.set_fonts(f);
}

// ---- visuals --------------------------------------------------------------

/// Steel chrome with a single accent driving selection / hover / focus.
fn steel_visuals(accent: Color32) -> Visuals {
    let mut v = Visuals::dark();
    v.panel_fill = col::STEEL;
    v.window_fill = col::STEEL;
    v.window_stroke = Stroke::new(1.0, col::RIVET);
    v.window_rounding = Rounding::same(4.0);
    v.extreme_bg_color = col::INK;
    v.faint_bg_color = col::STEEL_HI;
    v.override_text_color = Some(col::CHALK);
    v.hyperlink_color = accent;
    v.selection.bg_fill = scale(accent, 0.4);
    v.selection.stroke = Stroke::new(1.0, accent);

    let r = Rounding::same(3.0);
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = col::STEEL;
    w.noninteractive.weak_bg_fill = col::STEEL;
    w.noninteractive.bg_stroke = Stroke::new(1.0, col::RIVET);
    w.noninteractive.fg_stroke = Stroke::new(1.0, col::HAZE);

    w.inactive.bg_fill = col::STEEL_HI;
    w.inactive.weak_bg_fill = col::STEEL_HI;
    w.inactive.bg_stroke = Stroke::new(1.0, col::RIVET);
    w.inactive.fg_stroke = Stroke::new(1.0, col::CHALK);
    w.inactive.rounding = r;

    w.hovered.bg_fill = scale(accent, 0.34);
    w.hovered.weak_bg_fill = scale(accent, 0.34);
    w.hovered.bg_stroke = Stroke::new(1.0, accent);
    w.hovered.fg_stroke = Stroke::new(1.0, Color32::WHITE);
    w.hovered.rounding = r;

    w.active.bg_fill = scale(accent, 0.5);
    w.active.weak_bg_fill = scale(accent, 0.5);
    w.active.bg_stroke = Stroke::new(1.0, accent);
    w.active.fg_stroke = Stroke::new(1.0, Color32::WHITE);
    w.active.rounding = r;

    w.open.bg_fill = col::STEEL_HI;
    w.open.weak_bg_fill = col::STEEL_HI;
    w.open.bg_stroke = Stroke::new(1.0, accent);
    w.open.rounding = r;
    v
}

/// Fleet-select (no faction chosen): a cool steel-signal accent.
pub fn neutral_visuals() -> Visuals {
    steel_visuals(col::NEUTRAL_ACCENT)
}

/// Inside a fleet: chrome stays steel, the faction accent drives the highlights.
pub fn faction_visuals(p: &Palette) -> Visuals {
    steel_visuals(hex(&p.accent))
}
