use crate::apply::{ApplyEvent, Applier};
use crate::model::{hex, load_favorites, load_state, save_favorites, scale, Data, Fit, Monitor, PowerCfg, RotationCfg};
use crate::theme::{
    col, faction_visuals, install_fonts, mono, neutral_visuals, oswald, oswald_b, plex, plex_it,
    stencil,
};
use eframe::egui::{
    self, Align2, Color32, ColorImage, Context, FontId, Pos2, Rect, Rounding, Sense, Shape, Stroke,
    TextureHandle, TextureOptions, Vec2,
};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;

enum Screen {
    Factions,
    Gallery(String),
}

enum Phase {
    Cached,
    Rendering { done: u32, total: u32 },
    Applying,
}

struct Prog {
    skin: String,
    total: usize,
    idx: usize,
    name: String,
    w: u32,
    h: u32,
    phase: Phase,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Sort {
    Ship,
    SkinName,
}
impl Sort {
    fn label(self) -> &'static str {
        match self {
            Sort::Ship => "Ship A–Z",
            Sort::SkinName => "Skin A–Z",
        }
    }
}

/// A decoded looping GIF preview: one texture per frame + per-frame delay (seconds).
struct Anim {
    frames: Vec<TextureHandle>,
    delays: Vec<f32>,
    total: f32,
}
impl Anim {
    /// The frame to show at wall-clock `t` seconds (loops over `total`).
    fn frame_at(&self, t: f32) -> &TextureHandle {
        if self.frames.len() <= 1 || self.total <= 0.0 {
            return &self.frames[0];
        }
        let mut x = t % self.total;
        for (i, d) in self.delays.iter().enumerate() {
            if x < *d {
                return &self.frames[i];
            }
            x -= *d;
        }
        self.frames.last().unwrap()
    }
}

/// Decode `assets/preview_anim/<codename>.gif` into per-frame textures.
fn load_anim(ctx: &Context, root: &Path, codename: &str) -> Option<Anim> {
    use image::AnimationDecoder;
    let file = std::fs::File::open(root.join(format!("assets/preview_anim/{codename}.gif"))).ok()?;
    let dec = image::codecs::gif::GifDecoder::new(std::io::BufReader::new(file)).ok()?;
    let frames = dec.into_frames().collect_frames().ok()?;
    if frames.is_empty() {
        return None;
    }
    let mut texs = Vec::with_capacity(frames.len());
    let mut delays = Vec::with_capacity(frames.len());
    let mut total = 0.0f32;
    for (i, fr) in frames.iter().enumerate() {
        let (num, den) = fr.delay().numer_denom_ms();
        let mut d = if den == 0 { 0.0 } else { num as f32 / den as f32 / 1000.0 };
        if d <= 0.0 {
            d = 1.0 / 12.0;
        }
        let buf = fr.buffer();
        let size = [buf.width() as usize, buf.height() as usize];
        let ci = ColorImage::from_rgba_unmultiplied(size, buf.as_raw());
        texs.push(ctx.load_texture(format!("anim:{codename}:{i}"), ci, TextureOptions::LINEAR));
        delays.push(d);
        total += d;
    }
    Some(Anim { frames: texs, delays, total })
}

const INTERVALS: &[(&str, &str)] = &[
    ("5m", "Every 5 minutes"),
    ("15m", "Every 15 minutes"),
    ("30m", "Every 30 minutes"),
    ("1h", "Hourly"),
    ("6h", "Every 6 hours"),
    ("daily", "Daily"),
    ("weekly", "Weekly"),
    ("monthly", "Monthly"),
];

pub struct AppState {
    data: Data,
    cfg_dir: PathBuf,
    screen: Screen,
    search: String,
    selected: Option<usize>,
    textures: HashMap<String, TextureHandle>,
    applier: Applier,
    status: String,
    monitors: Vec<Monitor>,
    per_output: HashMap<String, String>,
    fit: HashMap<String, String>,
    preview_monitor: usize,
    preview_jobs: HashSet<String>,
    prog: Option<Prog>,
    rotation: RotationCfg,
    show_rotation: bool,
    pending: Option<String>,
    favorites: HashSet<String>,
    // gallery filters & sort
    fav_only: bool,
    oath_only: bool,
    sort: Sort,
    // keyboard navigation
    cursor: usize,
    grid_cols: usize,
    scroll_to_cursor: bool,
    focus_search: bool,
    search_focused: bool,
    // power settings
    power: PowerCfg,
    show_power: bool,
    // animated previews (decoded gif frames, lazily loaded)
    anim: HashMap<String, Anim>,
    anim_jobs: HashSet<String>,
    // headless capture (AL_SHOT=out.png): render a few frames, grab the framebuffer, exit
    shot_path: Option<PathBuf>,
    frame: u64,
}

fn root_dir() -> PathBuf {
    std::env::var("AL_WALLPAPER_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")))
}

fn config_dir() -> PathBuf {
    dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("al-wallpaper")
}

fn detect_monitors(root: &Path) -> Vec<Monitor> {
    let out = Command::new("bash")
        .arg(root.join("scripts/apply.sh"))
        .arg("--outputs")
        .output();
    let mut v = vec![];
    if let Ok(o) = out {
        for line in String::from_utf8_lossy(&o.stdout).lines() {
            let p: Vec<&str> = line.split_whitespace().collect();
            if p.len() == 2 {
                if let Some((w, h)) = p[1].split_once('x') {
                    if let (Ok(w), Ok(h)) = (w.parse(), h.parse()) {
                        v.push(Monitor { name: p[0].to_string(), w, h });
                    }
                }
            }
        }
    }
    v
}

fn load_tex(
    cache: &mut HashMap<String, TextureHandle>,
    ctx: &Context,
    root: &Path,
    rel: &str,
) -> Option<TextureHandle> {
    if let Some(t) = cache.get(rel) {
        return Some(t.clone());
    }
    let img = image::open(root.join(rel)).ok()?.to_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    let ci = ColorImage::from_rgba_unmultiplied(size, img.as_raw());
    let t = ctx.load_texture(rel, ci, TextureOptions::LINEAR);
    cache.insert(rel.to_string(), t.clone());
    Some(t)
}

fn trunc(s: &str, n: usize) -> String {
    if s.chars().count() > n {
        format!("{}…", s.chars().take(n.saturating_sub(1)).collect::<String>())
    } else {
        s.to_string()
    }
}

impl AppState {
    pub fn new(cc: &eframe::CreationContext) -> Self {
        let root = root_dir();
        let data = Data::load(root.clone()).expect("failed to load catalog/factions");
        let cfg_dir = config_dir();
        let st = load_state(&cfg_dir);
        install_fonts(&cc.egui_ctx);
        let mut style = (*cc.egui_ctx.style()).clone();
        style.spacing.item_spacing = Vec2::new(10.0, 10.0);
        style.spacing.button_padding = Vec2::new(10.0, 5.0);
        use egui::TextStyle as T;
        style.text_styles = [
            (T::Heading, oswald_b(22.0)),
            (T::Body, plex(14.5)),
            (T::Button, plex(13.5)),
            (T::Monospace, mono(12.5)),
            (T::Small, plex(11.0)),
        ]
        .into();
        cc.egui_ctx.set_style(style);

        let start = std::env::var("AL_START_FACTION").ok().filter(|k| data.faction(k).is_some() && data.count(k) > 0);
        let screen = match &start {
            Some(k) => {
                if let Some(f) = data.faction(k) {
                    cc.egui_ctx.set_visuals(faction_visuals(&f.palette));
                }
                Screen::Gallery(k.clone())
            }
            None => {
                cc.egui_ctx.set_visuals(neutral_visuals());
                Screen::Factions
            }
        };
        let selected = std::env::var("AL_SELECT").ok().and_then(|c| data.by_code.get(&c).copied());
        let show_rotation = std::env::var("AL_SHOW_ROTATION").is_ok();
        let show_power = std::env::var("AL_SHOW_POWER").is_ok();
        if st.power.battery == "pause" {
            start_power_daemon(&root);
        }
        Self {
            data,
            cfg_dir,
            screen,
            search: String::new(),
            selected,
            textures: HashMap::new(),
            applier: Applier::new(),
            status: "ready".into(),
            monitors: detect_monitors(&root),
            per_output: st.outputs,
            fit: st.fit,
            preview_monitor: 0,
            preview_jobs: HashSet::new(),
            prog: None,
            rotation: st.rotation,
            show_rotation,
            pending: std::env::var("AL_AUTOAPPLY").ok().filter(|s| !s.is_empty()),
            favorites: load_favorites(&root),
            fav_only: false,
            oath_only: false,
            sort: Sort::Ship,
            cursor: 0,
            grid_cols: 5,
            scroll_to_cursor: false,
            focus_search: false,
            search_focused: false,
            power: st.power,
            show_power,
            anim: HashMap::new(),
            anim_jobs: HashSet::new(),
            shot_path: std::env::var("AL_SHOT").ok().filter(|s| !s.is_empty()).map(PathBuf::from),
            frame: 0,
        }
    }
}

impl eframe::App for AppState {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        // headless capture harness: drive frames, grab the framebuffer, save, quit
        if self.shot_path.is_some() {
            self.frame += 1;
            if self.frame == 1 {
                eprintln!("[shot] first frame; will capture at frame 40");
            }
            // timer-based wake advances frames even when the window is on a hidden
            // workspace (compositor frame callbacks are throttled there).
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
            if self.frame == 40 {
                eprintln!("[shot] frame {} — requesting screenshot", self.frame);
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot);
            }
            let grabbed = ctx.input(|i| {
                i.events.iter().find_map(|e| match e {
                    egui::Event::Screenshot { image, .. } => Some(image.clone()),
                    _ => None,
                })
            });
            if let (Some(img), Some(path)) = (grabbed, self.shot_path.clone()) {
                let (w, h) = (img.size[0] as u32, img.size[1] as u32);
                let mut buf = Vec::with_capacity((w * h * 4) as usize);
                for px in &img.pixels {
                    buf.extend_from_slice(&px.to_array());
                }
                match image::RgbaImage::from_raw(w, h, buf).map(|im| im.save(&path)) {
                    Some(Ok(())) => eprintln!("[shot] saved {}x{} -> {}", w, h, path.display()),
                    other => eprintln!("[shot] save failed: {:?}", other),
                }
                std::process::exit(0);
            }
            // failsafe so a foreground run always terminates
            if self.frame > 240 {
                eprintln!("[shot] failsafe exit — no framebuffer after 240 frames");
                std::process::exit(2);
            }
        }
        // optional launch-time apply (AL_AUTOAPPLY=<skin>)
        if let Some(sk) = self.pending.take() {
            let root = self.data.root.clone();
            self.applier.apply(&root, &sk, None);
        }
        // ---- drain apply events ----
        for ev in self.applier.poll() {
            match ev {
                ApplyEvent::Outputs(n) => {
                    self.prog = Some(Prog { skin: String::new(), total: n, idx: 0, name: String::new(), w: 0, h: 0, phase: Phase::Applying });
                }
                ApplyEvent::Target { name, w, h, i, n } => {
                    if let Some(p) = &mut self.prog {
                        p.name = name; p.w = w; p.h = h; p.idx = i; p.total = n; p.phase = Phase::Applying;
                    }
                }
                ApplyEvent::Render { w, h, .. } => {
                    if let Some(p) = &mut self.prog { p.w = w; p.h = h; p.phase = Phase::Rendering { done: 0, total: 1 }; }
                    self.status = "rendering…".into();
                }
                ApplyEvent::Cached(_) => {
                    if let Some(p) = &mut self.prog { p.phase = Phase::Cached; }
                }
                ApplyEvent::Progress { done, total } => {
                    if let Some(p) = &mut self.prog { p.phase = Phase::Rendering { done, total }; }
                }
                ApplyEvent::Applied { name, skin } => {
                    self.per_output.insert(name, skin);
                }
                ApplyEvent::Done { skin, .. } => {
                    self.status = format!("live: {skin}");
                }
                ApplyEvent::Err(e) => {
                    self.status = format!("error: {e}");
                }
                ApplyEvent::Exit(_) => {
                    self.prog = None;
                }
            }
        }
        if self.applier.running {
            ctx.request_repaint();
        }

        let AppState {
            data, screen, search, selected, textures, applier, status,
            monitors, per_output, fit, preview_monitor, preview_jobs, prog, rotation, show_rotation, favorites,
            fav_only, oath_only, sort, cursor, grid_cols, scroll_to_cursor, focus_search, search_focused, power, show_power,
            anim, anim_jobs, ..
        } = self;
        let root = data.root.clone();
        let mut goto: Option<Screen> = None;
        let busy = applier.running;
        let now = ctx.input(|i| i.time) as f32;

        // ---- visible list for the current gallery (filter + sort), reused by render & keyboard nav ----
        let mut visible: Vec<usize> = Vec::new();
        if let Screen::Gallery(key) = &*screen {
            let q = search.to_lowercase();
            let empty = vec![];
            for &i in data.by_faction.get(key).unwrap_or(&empty) {
                let s = &data.skins[i];
                if *fav_only && !favorites.contains(&s.codename) { continue; }
                if *oath_only && !s.is_oath { continue; }
                if !q.is_empty()
                    && !s.ship.to_lowercase().contains(&q)
                    && !s.skin_name.as_deref().unwrap_or("").to_lowercase().contains(&q)
                    && !s.codename.contains(&q)
                { continue; }
                visible.push(i);
            }
            if *sort == Sort::SkinName {
                visible.sort_by(|&a, &b| {
                    let sa = data.skins[a].skin_name.as_deref().unwrap_or(&data.skins[a].codename).to_lowercase();
                    let sb = data.skins[b].skin_name.as_deref().unwrap_or(&data.skins[b].codename).to_lowercase();
                    sa.cmp(&sb).then(data.skins[a].codename.cmp(&data.skins[b].codename))
                });
            }
            if *cursor >= visible.len() { *cursor = visible.len().saturating_sub(1); }
        }

        // ---- keyboard navigation ---- (only the search box should swallow keys; a focused
        // card must NOT block arrow nav, so we gate on the search field specifically)
        if !*search_focused {
            ctx.input_mut(|inp| {
                use egui::Key;
                let slash = inp.consume_key(egui::Modifiers::NONE, Key::Slash);
                let esc = inp.consume_key(egui::Modifiers::NONE, Key::Escape);
                let enter = inp.consume_key(egui::Modifiers::NONE, Key::Enter);
                let left = inp.consume_key(egui::Modifiers::NONE, Key::ArrowLeft);
                let right = inp.consume_key(egui::Modifiers::NONE, Key::ArrowRight);
                let up = inp.consume_key(egui::Modifiers::NONE, Key::ArrowUp);
                let down = inp.consume_key(egui::Modifiers::NONE, Key::ArrowDown);
                if slash {
                    if let Screen::Gallery(_) = &*screen { *focus_search = true; }
                }
                if esc {
                    if selected.is_some() {
                        *selected = None;
                    } else if let Screen::Gallery(_) = &*screen {
                        goto = Some(Screen::Factions);
                    }
                }
                if let Screen::Gallery(_) = &*screen {
                    let cols = (*grid_cols).max(1);
                    let n = visible.len();
                    if n > 0 {
                        let mut c = (*cursor).min(n - 1);
                        let mut moved = false;
                        if right { c = (c + 1).min(n - 1); moved = true; }
                        if left { c = c.saturating_sub(1); moved = true; }
                        if down { c = (c + cols).min(n - 1); moved = true; }
                        if up { c = c.saturating_sub(cols); moved = true; }
                        if moved { *cursor = c; *scroll_to_cursor = true; }
                        if enter { *selected = Some(visible[c]); }
                    }
                }
            });
        }

        // ---- header ----
        let head_accent = match &*screen {
            Screen::Factions => col::NEUTRAL_ACCENT,
            Screen::Gallery(key) => data.faction(key).map(|f| hex(&f.palette.accent)).unwrap_or(col::NEUTRAL_ACCENT),
        };
        egui::TopBottomPanel::top("header").exact_height(80.0).show(ctx, |ui| {
            // accent waterline pinned to the bottom of the header
            let hr = ui.max_rect();
            ui.painter().line_segment([hr.left_bottom(), hr.right_bottom()], Stroke::new(2.0, head_accent));
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.add_space(12.0);
                match &*screen {
                    Screen::Factions => {
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 2.0;
                            ui.label(egui::RichText::new("AZUR LANE").font(oswald_b(28.0)).color(col::CHALK));
                            ui.label(egui::RichText::new(track("Fleet Registry")).font(mono(11.0)).color(col::HAZE));
                        });
                    }
                    Screen::Gallery(key) => {
                        if ui.button("◂ FLEETS").clicked() { goto = Some(Screen::Factions); }
                        ui.add_space(10.0);
                        if let Some(f) = data.faction(key) {
                            pennant_chip(ui, &f.short, head_accent, 15.0);
                            ui.add_space(6.0);
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.y = 2.0;
                                ui.label(egui::RichText::new(f.name.to_uppercase()).font(oswald_b(26.0)).color(col::CHALK));
                                ui.label(egui::RichText::new(track(&format!("{} ships on register", data.count(key)))).font(mono(10.5)).color(col::HAZE));
                            });
                        }
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(12.0);
                    if ui.button("⟳ ROTATION").clicked() { *show_rotation = !*show_rotation; }
                    if ui.button("⚡ POWER").on_hover_text("Pause the wallpaper when hidden; cap fps").clicked() { *show_power = !*show_power; }
                    *search_focused = false;
                    if let Screen::Gallery(_) = &*screen {
                        let te = ui.add(egui::TextEdit::singleline(search).hint_text("search register  (/)").desired_width(180.0));
                        if *focus_search { te.request_focus(); *focus_search = false; }
                        *search_focused = te.has_focus();
                    }
                });
            });
        });

        // ---- status bar: per-monitor assignment ----
        egui::TopBottomPanel::bottom("status").exact_height(30.0).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.add_space(10.0);
                if busy { ui.spinner(); }
                if monitors.is_empty() {
                    ui.label(egui::RichText::new(&*status).font(mono(11.5)).color(col::HAZE));
                } else {
                    for (i, m) in monitors.iter().enumerate() {
                        if i > 0 { ui.label(egui::RichText::new("│").font(mono(11.0)).color(col::RIVET)); }
                        let ship = per_output.get(&m.name).map(|c| data.ship_of(c)).unwrap_or_else(|| "—".into());
                        ui.label(egui::RichText::new(format!("{} ▸", m.name)).font(mono(11.0)).color(col::HAZE_DIM));
                        ui.label(egui::RichText::new(ship).font(mono(11.0)).color(col::CHALK));
                    }
                }
                // Re-apply the saved wallpapers to recreate their surfaces. On multi-monitor
                // setups a wallpaper can occasionally show up on the wrong screen — a compositor
                // quirk, not a bad assignment — and re-applying clears it.
                if monitors.len() > 1 && !per_output.is_empty() {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(8.0);
                        let btn = ui.add_enabled(!busy, egui::Button::new(egui::RichText::new("⟳ FIX SCREENS").font(mono(11.0))));
                        if btn.on_hover_text(
                            "Wrong wallpaper on a monitor? On some multi-monitor Wayland setups a\n\
                             wallpaper can flicker onto the wrong screen — a known compositor quirk,\n\
                             not your assignment. Click to re-apply and clear it.",
                        ).clicked() {
                            applier.refresh(&root);
                            *status = "re-applying wallpapers…".into();
                        }
                    });
                }
            });
        });

        // ---- ship dossier side panel ----
        if let Some(idx) = *selected {
            let skin = data.skins[idx].clone();
            let fac = data.faction(&skin.faction).cloned();
            let accent = fac.as_ref().map(|f| hex(&f.palette.accent)).unwrap_or(col::NEUTRAL_ACCENT);
            egui::SidePanel::right("detail").resizable(false).exact_width(338.0).show(ctx, |ui| {
              // accent seam down the panel's left edge
              let pr = ui.max_rect();
              ui.painter().line_segment([pr.left_top(), pr.left_bottom()], Stroke::new(2.0, accent));
              egui::ScrollArea::vertical().show(ui, |ui| {
                ui.add_space(12.0);
                ui.horizontal(|ui| { ui.add_space(8.0); ui.label(egui::RichText::new(track("Ship dossier")).font(mono(10.5)).color(col::HAZE)); });
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new(skin.ship.to_uppercase()).font(oswald_b(26.0)).color(col::CHALK));
                    if skin.is_oath {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add_space(8.0);
                            ui.label(egui::RichText::new("⚭").size(18.0).color(col::GOLD)).on_hover_text("Oath skin");
                        });
                    }
                });
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    if let Some(f) = &fac { pennant_chip(ui, &f.short, accent, 12.0); ui.add_space(4.0); }
                    if let Some(sn) = &skin.skin_name {
                        ui.label(egui::RichText::new(sn).font(plex_it(13.0)).color(col::HAZE));
                    }
                });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    let fav = favorites.contains(&skin.codename);
                    let (txt, fc) = if fav { ("★ FAVORITED", col::GOLD) } else { ("☆ ADD TO FAVORITES", col::HAZE) };
                    if ui.button(egui::RichText::new(txt).color(fc)).clicked() {
                        if fav {
                            favorites.remove(&skin.codename);
                        } else {
                            favorites.insert(skin.codename.clone());
                            // pre-render at all resolutions so rotation stays instant
                            let _ = Command::new("setsid").args(["-f", "bash"])
                                .arg(root.join("scripts/prerender.sh")).arg(&skin.codename).spawn();
                        }
                        save_favorites(&root, favorites);
                    }
                });
                ui.add_space(10.0);
                // ---- on-screen preview for the selected monitor + fit mode ----
                let n_mon = monitors.len();
                if *preview_monitor >= n_mon { *preview_monitor = 0; }
                let mon = monitors.get(*preview_monitor).cloned();
                let aspect = mon.as_ref().map(|m| m.w as f32 / m.h.max(1) as f32).unwrap_or(16.0 / 9.0);
                let cur_fit = mon
                    .as_ref()
                    .map(|m| Fit::from_key(fit.get(&m.name).map(|s| s.as_str()).unwrap_or("fit")))
                    .unwrap_or(Fit::Fit);

                if n_mon > 1 {
                    ui.horizontal(|ui| {
                        ui.add_space(8.0);
                        ui.label(egui::RichText::new(track("screen")).font(mono(10.5)).color(col::HAZE_DIM));
                        let sel = mon.as_ref().map(|m| format!("{} · {}×{}", m.name, m.w, m.h)).unwrap_or_default();
                        egui::ComboBox::from_id_source("prev_mon").selected_text(sel).width(224.0).show_ui(ui, |ui| {
                            for (i, m) in monitors.iter().enumerate() {
                                if ui.selectable_label(*preview_monitor == i, format!("{} ({}×{})", m.name, m.w, m.h)).clicked() {
                                    *preview_monitor = i;
                                }
                            }
                        });
                    });
                    ui.add_space(4.0);
                }

                let avail = ui.available_width() - 16.0;
                let ph = (avail / aspect).clamp(80.0, 240.0);
                // accurate preview uses a native-aspect render of the painting; fall back to the
                // 3:4 gallery thumbnail and generate the real one in the background on first view.
                let prev = load_tex(textures, ctx, &root, &format!("assets/preview/{}.png", skin.codename));
                if prev.is_none() {
                    if preview_jobs.insert(skin.codename.clone()) {
                        let _ = Command::new("setsid").args(["-f", "node"])
                            .arg(root.join("scripts/preview.js")).arg(&skin.codename).spawn();
                    }
                    ctx.request_repaint_after(std::time::Duration::from_millis(800));
                }
                // animated preview: loop the cached gif if present, else generate it in the background
                // and keep showing the static frame meanwhile.
                let anim_tex: Option<TextureHandle> = if let Some(a) = anim.get(&skin.codename) {
                    ctx.request_repaint_after(std::time::Duration::from_millis(70));
                    Some(a.frame_at(now).clone())
                } else if root.join(format!("assets/preview_anim/{}.gif", skin.codename)).exists() {
                    if let Some(a) = load_anim(ctx, &root, &skin.codename) {
                        let t = a.frame_at(now).clone();
                        anim.insert(skin.codename.clone(), a);
                        ctx.request_repaint_after(std::time::Duration::from_millis(70));
                        Some(t)
                    } else {
                        None
                    }
                } else {
                    if anim_jobs.insert(skin.codename.clone()) {
                        let _ = Command::new("setsid").args(["-f", "node"])
                            .arg(root.join("scripts/preview_anim.js")).arg(&skin.codename).spawn();
                    }
                    ctx.request_repaint_after(std::time::Duration::from_millis(1500));
                    None
                };
                let content = anim_tex.clone().or_else(|| prev.clone()).or_else(|| load_tex(textures, ctx, &root, &skin.thumb));
                let content_aspect = content.as_ref().map(|t| { let s = t.size(); s[0] as f32 / s[1].max(1) as f32 }).unwrap_or(0.75);
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(avail, ph), Sense::hover());
                    let emblem = if content.is_none() {
                        fac.as_ref().and_then(|f| load_tex(textures, ctx, &root, &format!("assets/emblems/{}.png", f.key)))
                    } else { None };
                    monitor_preview(ui.painter(), rect, aspect, cur_fit, content.as_ref(), emblem.as_ref(), accent, content_aspect);
                });
                let note = if anim_tex.is_some() {
                    Some(("▶ ANIMATED PREVIEW", col::SIGNAL))
                } else if prev.is_none() {
                    Some(("GENERATING PREVIEW…", col::HAZE))
                } else {
                    Some(("RENDERING ANIMATED PREVIEW…", col::HAZE))
                };
                if let Some((txt, c)) = note {
                    ui.horizontal(|ui| {
                        ui.add_space(8.0);
                        ui.label(egui::RichText::new(track(txt)).font(mono(9.5)).color(c));
                    });
                }

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new(track("Fit on screen")).font(mono(10.5)).color(col::HAZE));
                });
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    for f in Fit::ALL {
                        if ui.add_enabled(mon.is_some(), egui::SelectableLabel::new(f == cur_fit, f.label())).clicked() {
                            if let Some(m) = &mon {
                                fit.insert(m.name.clone(), f.key().to_string());
                                save_fit(&root, &m.name, f.key());
                            }
                        }
                    }
                });
                ui.add_space(3.0);
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    let hint = match cur_fit {
                        Fit::Fit => "Whole character; gradient fills the sides.",
                        Fit::Stretch => "Stretched to fill — may distort.",
                        Fit::Crop => "Zoomed to fill the screen — edges cropped.",
                    };
                    ui.label(egui::RichText::new(hint).font(plex_it(11.0)).color(col::HAZE_DIM));
                });
                ui.add_space(12.0);
                // ---- data rows ----
                let mut row = |ui: &mut egui::Ui, k: &str, v: egui::RichText| {
                    ui.horizontal(|ui| {
                        ui.add_space(8.0);
                        ui.label(egui::RichText::new(track(k)).font(mono(10.5)).color(col::HAZE_DIM));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { ui.add_space(2.0); ui.label(v); });
                    });
                };
                if let Some(f) = &fac { row(ui, "fleet", egui::RichText::new(f.name.to_uppercase()).font(oswald(14.0)).color(accent)); }
                row(ui, "codename", egui::RichText::new(&skin.codename).font(mono(11.5)).color(col::CHALK));
                if let Some(m) = &mon { row(ui, "target", egui::RichText::new(format!("{}×{}", m.w, m.h)).font(mono(11.5)).color(col::CHALK)); }

                ui.add_space(12.0);
                let seam = ui.max_rect();
                ui.painter().line_segment([Pos2::new(seam.left() + 8.0, ui.cursor().top()), Pos2::new(seam.right() - 8.0, ui.cursor().top())], Stroke::new(1.0, col::RIVET));
                ui.add_space(10.0);
                ui.horizontal(|ui| { ui.add_space(8.0); ui.label(egui::RichText::new(track("Commission to")).font(mono(10.5)).color(col::HAZE)); });
                ui.add_space(5.0);
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    let b = egui::Button::new(egui::RichText::new("▣ ALL SCREENS").font(oswald(16.0)).color(col::INK))
                        .fill(accent)
                        .min_size(Vec2::new(ui.available_width() - 8.0, 36.0));
                    if ui.add_enabled(!busy, b).clicked() { applier.apply(&root, &skin.codename, None); *status = format!("commissioning {}…", skin.codename); }
                });
                for m in monitors.iter() {
                    ui.add_space(5.0);
                    ui.horizontal(|ui| {
                        ui.add_space(8.0);
                        let on = per_output.get(&m.name).map(|c| c == &skin.codename).unwrap_or(false);
                        let label = format!("{}  {} · {}×{}", if on { "✓" } else { "→" }, m.name, m.w, m.h);
                        let mut b = egui::Button::new(egui::RichText::new(label).font(mono(12.0)).color(if on { accent } else { col::CHALK }))
                            .min_size(Vec2::new(ui.available_width() - 8.0, 28.0));
                        if on { b = b.stroke(Stroke::new(1.0, accent)); }
                        if ui.add_enabled(!busy, b).clicked() { applier.apply(&root, &skin.codename, Some(&m.name)); *status = format!("commissioning {} → {}…", skin.codename, m.name); }
                    });
                }
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new("First use renders once per screen, resolution & fit, then it's cached. Change the fit above, then re-commission to update a live screen.").font(plex(11.0)).color(col::HAZE_DIM));
                });
                if monitors.len() > 1 {
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.add_space(8.0);
                        ui.label(egui::RichText::new("Multi-screen note: a wallpaper occasionally showing on the wrong screen is a known Wayland compositor quirk, not a bad assignment. Use ⟳ Fix screens (bottom bar) to re-apply and clear it.").font(plex(11.0)).color(col::HAZE_DIM));
                    });
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| { ui.add_space(8.0); if ui.button("CLOSE DOSSIER").clicked() { *selected = None; } });
                ui.add_space(10.0);
              });
            });
        }

        // ---- central ----
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.add_space(8.0);
                match &*screen {
                    Screen::Factions => {
                        ui.horizontal(|ui| {
                            ui.add_space(2.0);
                            ui.label(egui::RichText::new(track("Select fleet")).font(mono(11.5)).color(col::HAZE));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.add_space(2.0);
                                let n = data.factions.iter().filter(|f| data.count(&f.key) > 0).count();
                                ui.label(egui::RichText::new(track(&format!("{n} fleets"))).font(mono(11.0)).color(col::HAZE_DIM));
                            });
                        });
                        ui.add_space(8.0);
                        ui.horizontal_wrapped(|ui| {
                            for f in data.factions.iter() {
                                let n = data.count(&f.key);
                                if n == 0 { continue; }
                                if faction_tile(ui, ctx, textures, &root, &f.key, &f.name, &f.short, n, &f.palette) {
                                    goto = Some(Screen::Gallery(f.key.clone()));
                                }
                            }
                        });
                    }
                    Screen::Gallery(key) => {
                        let accent = data.faction(key).map(|f| hex(&f.palette.accent)).unwrap_or(Color32::GRAY);
                        let fkey = key.clone();
                        // ---- manifest toolbar ----
                        ui.horizontal(|ui| {
                            ui.add_space(2.0);
                            ui.toggle_value(fav_only, "★ FAVORITES");
                            ui.toggle_value(oath_only, "⚭ OATH");
                            ui.separator();
                            ui.label(egui::RichText::new(track("sort")).font(mono(11.0)).color(col::HAZE));
                            egui::ComboBox::from_id_source("sort").selected_text(sort.label()).width(104.0).show_ui(ui, |ui| {
                                ui.selectable_value(sort, Sort::Ship, Sort::Ship.label());
                                ui.selectable_value(sort, Sort::SkinName, Sort::SkinName.label());
                            });
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.add_space(2.0);
                                ui.label(egui::RichText::new(track(&format!("{} shown", visible.len()))).font(mono(11.0)).color(col::HAZE_DIM));
                            });
                        });
                        ui.add_space(6.0);
                        // column count for keyboard up/down (card 176 + 10 spacing)
                        *grid_cols = (((ui.available_width() + 10.0) / 186.0).floor() as usize).max(1);
                        ui.horizontal_wrapped(|ui| {
                            for (pos, &i) in visible.iter().enumerate() {
                                let s = &data.skins[i];
                                let thumb = s.thumb.clone();
                                let ship = s.ship.clone();
                                let sub = s.skin_name.clone().unwrap_or_else(|| s.codename.clone());
                                let oath = s.is_oath;
                                let live = per_output.values().any(|c| c == &s.codename);
                                let fav = favorites.contains(&s.codename);
                                let is_cursor = pos == *cursor;
                                let resp = skin_card(ui, ctx, textures, &root, &thumb, &ship, &sub, oath, live, fav, &fkey, accent, is_cursor);
                                if resp.clicked() { *selected = Some(i); *cursor = pos; }
                                if is_cursor && *scroll_to_cursor { resp.scroll_to_me(Some(egui::Align::Center)); }
                            }
                        });
                        *scroll_to_cursor = false;
                    }
                }
                ui.add_space(12.0);
            });
        });

        // ---- rotation settings window ----
        if *show_rotation {
            let mut open = true;
            let mut changed = false;
            egui::Window::new("⟳  ROTATION")
                .open(&mut open)
                .resizable(false)
                .collapsible(false)
                .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.set_width(330.0);
                    changed |= ui.checkbox(&mut rotation.enabled, "Enable automatic rotation").changed();
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.label("Every:");
                        let cur = INTERVALS.iter().find(|(k, _)| *k == rotation.interval).map(|(_, l)| *l).unwrap_or("Every 30 minutes");
                        egui::ComboBox::from_id_source("iv").selected_text(cur).show_ui(ui, |ui| {
                            for (k, l) in INTERVALS {
                                if ui.selectable_label(rotation.interval == *k, *l).clicked() { rotation.interval = k.to_string(); changed = true; }
                            }
                        });
                    });
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label("Pool:");
                        let cur = if rotation.scope == "all" { "All skins".to_string() }
                            else if rotation.scope == "favorites" { format!("★ Favorites ({})", favorites.len()) }
                            else { data.faction(rotation.scope.strip_prefix("faction:").unwrap_or("")).map(|f| f.name.clone()).unwrap_or_else(|| "All skins".into()) };
                        egui::ComboBox::from_id_source("sc").selected_text(cur).show_ui(ui, |ui| {
                            if ui.selectable_label(rotation.scope == "favorites", format!("★ Favorites ({})", favorites.len())).clicked() { rotation.scope = "favorites".into(); changed = true; }
                            if ui.selectable_label(rotation.scope == "all", "All skins").clicked() { rotation.scope = "all".into(); changed = true; }
                            for f in data.factions.iter() {
                                if data.count(&f.key) == 0 { continue; }
                                let key = format!("faction:{}", f.key);
                                if ui.selectable_label(rotation.scope == key, &f.name).clicked() { rotation.scope = key; changed = true; }
                            }
                        });
                    });
                    ui.add_space(4.0);
                    changed |= ui.checkbox(&mut rotation.per_monitor, "Different skin on each monitor").changed();
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if ui.add_enabled(!busy, egui::Button::new("⟳ Rotate now")).clicked() {
                            if let Some(code) = pick_random(data, &rotation.scope) {
                                applier.apply(&root, &code, None);
                                *status = format!("rotating → {code}…");
                            }
                        }
                        ui.label(egui::RichText::new(if rotation.enabled { "rotation on" } else { "rotation off" }).size(11.0).color(hex("#6f8090")));
                    });
                });
            if changed { self_save_rotation(&root, rotation); }
            if !open { *show_rotation = false; }
        }

        // ---- power settings window ----
        if *show_power {
            let mut open = true;
            let mut changed = false;
            const FPS_CAPS: &[(u32, &str)] = &[(0, "Uncapped"), (15, "15 fps"), (24, "24 fps"), (30, "30 fps")];
            egui::Window::new("⚡  POWER")
                .open(&mut open)
                .resizable(false)
                .collapsible(false)
                .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.set_width(330.0);
                    ui.label(egui::RichText::new("When the wallpaper is hidden (e.g. a fullscreen game):").size(12.5).color(hex("#9fb0c0")));
                    ui.add_space(6.0);
                    for (mode, label, hint) in [
                        ("off", "Keep playing", "Always animate (uses the most power)."),
                        ("pause", "Pause video", "Stop decoding while covered — resumes instantly."),
                        ("stop", "Stop video", "Fully stop mpv while covered — max saving, brief reload."),
                    ] {
                        if ui.radio(power.mode == mode, label).on_hover_text(hint).clicked() {
                            power.mode = mode.to_string();
                            changed = true;
                        }
                    }
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        ui.label("Frame rate cap:");
                        let cur = FPS_CAPS.iter().find(|(c, _)| *c == power.fps_cap).map(|(_, l)| *l).unwrap_or("Uncapped");
                        egui::ComboBox::from_id_source("fpscap").selected_text(cur).show_ui(ui, |ui| {
                            for (c, l) in FPS_CAPS {
                                if ui.selectable_label(power.fps_cap == *c, *l).clicked() { power.fps_cap = *c; changed = true; }
                            }
                        });
                    });
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new("Applies to wallpapers set from now on — re-apply to update what's already live.").size(11.0).italics().color(hex("#6f8090")));
                    ui.add_space(10.0);
                    ui.separator();
                    ui.add_space(6.0);
                    let mut bat = power.battery == "pause";
                    if ui.checkbox(&mut bat, "Freeze the wallpaper while on battery")
                        .on_hover_text("Stops the animation whenever the laptop is unplugged; resumes on AC. No effect on a desktop.")
                        .changed()
                    {
                        power.battery = if bat { "pause".into() } else { "off".into() };
                        changed = true;
                    }
                    ui.label(egui::RichText::new("A tiny daemon watches the power source even when this app is closed.").size(11.0).italics().color(hex("#6f8090")));
                });
            if changed { save_power(&root, power); }
            if !open { *show_power = false; }
        }

        // ---- progress modal ----
        if busy {
            if let Some(p) = &*prog {
                egui::Area::new(egui::Id::new("scrim")).fixed_pos(Pos2::ZERO).show(ctx, |ui| {
                    let sr = ctx.screen_rect();
                    ui.painter().rect_filled(sr, Rounding::ZERO, Color32::from_black_alpha(150));
                });
                egui::Window::new("applying").title_bar(false).resizable(false).collapsible(false)
                    .anchor(Align2::CENTER_CENTER, Vec2::ZERO).show(ctx, |ui| {
                        ui.set_width(320.0);
                        ui.add_space(10.0);
                        ui.vertical_centered(|ui| {
                            ui.label(egui::RichText::new("COMMISSIONING").font(oswald_b(21.0)).color(col::CHALK));
                            ui.label(egui::RichText::new(track("rendering to your screen")).font(mono(10.0)).color(col::HAZE));
                        });
                        ui.add_space(14.0);
                        let frac = overall_fraction(p);
                        let (line, sub) = match &p.phase {
                            Phase::Rendering { done, total } => (
                                format!("{} · {}×{}", p.name, p.w, p.h),
                                format!("frame {done}/{total}  ·  screen {}/{}", p.idx.max(1), p.total.max(1)),
                            ),
                            Phase::Cached => (format!("{} · {}×{}", p.name, p.w, p.h), "cached — applying instantly".into()),
                            Phase::Applying => ("Applying…".to_string(), format!("screen {}/{}", p.idx.max(1), p.total.max(1))),
                        };
                        // radar scope with the percentage at its centre
                        ui.vertical_centered(|ui| {
                            let (r, _) = ui.allocate_exact_size(Vec2::splat(158.0), Sense::hover());
                            radar(ui.painter(), r.center(), 68.0, frac, head_accent, now);
                            ui.painter().text(r.center(), Align2::CENTER_CENTER, format!("{:.0}%", frac * 100.0), oswald_b(30.0), col::CHALK);
                        });
                        ui.add_space(14.0);
                        ui.vertical_centered(|ui| {
                            ui.label(egui::RichText::new(line).font(oswald(15.0)).color(col::CHALK));
                            ui.label(egui::RichText::new(track(&sub)).font(mono(10.0)).color(col::HAZE));
                        });
                        ui.add_space(8.0);
                        ui.vertical_centered(|ui| {
                            ui.label(egui::RichText::new("Rendered once per skin, resolution & fit, then cached.").font(plex(11.0)).color(col::HAZE_DIM));
                        });
                        ui.add_space(6.0);
                    });
            }
        }

        // ---- apply navigation ----
        if let Some(s) = goto {
            match &s {
                Screen::Factions => ctx.set_visuals(neutral_visuals()),
                Screen::Gallery(k) => { if let Some(f) = data.faction(k) { ctx.set_visuals(faction_visuals(&f.palette)); } }
            }
            *screen = s;
            *selected = None;
            *search = String::new();
        }
    }
}

fn overall_fraction(p: &Prog) -> f32 {
    let n = p.total.max(1) as f32;
    let completed = (p.idx.saturating_sub(1)) as f32;
    let cur = match &p.phase {
        Phase::Rendering { done, total } => *done as f32 / (*total).max(1) as f32,
        Phase::Cached => 0.95,
        Phase::Applying => 0.5,
    };
    ((completed + cur) / n).clamp(0.0, 1.0)
}

fn pick_random(data: &Data, scope: &str) -> Option<String> {
    let pool: Vec<&str> = if let Some(f) = scope.strip_prefix("faction:") {
        data.by_faction.get(f).map(|v| v.iter().map(|&i| data.skins[i].codename.as_str()).collect()).unwrap_or_default()
    } else {
        data.skins.iter().map(|s| s.codename.as_str()).collect()
    };
    if pool.is_empty() { return None; }
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
    Some(pool[(nanos as usize) % pool.len()].to_string())
}

// free fn so it doesn't borrow all of self in the window closure
fn self_save_rotation(root: &Path, r: &RotationCfg) {
    let _ = Command::new("node")
        .arg(root.join("scripts/state.js"))
        .args(["set-rotation", if r.enabled { "true" } else { "false" }, &r.interval, &r.scope, if r.per_monitor { "true" } else { "false" }])
        .status();
    if r.enabled {
        let _ = Command::new("setsid").args(["-f", "bash"]).arg(root.join("scripts/rotate-daemon.sh")).spawn();
    }
}

fn save_fit(root: &Path, name: &str, mode: &str) {
    let _ = Command::new("node")
        .arg(root.join("scripts/state.js"))
        .args(["set-fit", name, mode])
        .status();
}

fn save_power(root: &Path, p: &PowerCfg) {
    let _ = Command::new("node")
        .arg(root.join("scripts/state.js"))
        .args(["set-power", &p.mode, &p.fps_cap.to_string(), &p.battery])
        .status();
    // ensure the battery daemon is running once the feature is enabled (self-locks against dupes)
    if p.battery == "pause" {
        start_power_daemon(root);
    }
}

fn start_power_daemon(root: &Path) {
    let _ = Command::new("setsid").args(["-f", "bash"]).arg(root.join("scripts/power-daemon.sh")).spawn();
}

/// Largest rect of the given aspect (w/h) centered inside `outer`.
fn fit_rect(outer: Rect, aspect: f32) -> Rect {
    let (ow, oh) = (outer.width(), outer.height());
    let (w, h) = if ow / oh > aspect { (oh * aspect, oh) } else { (ow, ow / aspect) };
    Rect::from_center_size(outer.center(), Vec2::new(w, h))
}

/// Vertical two-color gradient fill (mimics the wallpaper's background gradient).
fn gradient_rect(painter: &egui::Painter, rect: Rect, top: Color32, bot: Color32) {
    let uv = egui::epaint::WHITE_UV;
    let mut mesh = egui::epaint::Mesh::default();
    mesh.vertices.push(egui::epaint::Vertex { pos: rect.left_top(), uv, color: top });
    mesh.vertices.push(egui::epaint::Vertex { pos: rect.right_top(), uv, color: top });
    mesh.vertices.push(egui::epaint::Vertex { pos: rect.right_bottom(), uv, color: bot });
    mesh.vertices.push(egui::epaint::Vertex { pos: rect.left_bottom(), uv, color: bot });
    mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
    painter.add(mesh);
}

fn alpha(c: Color32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

/// Thin-spaced uppercase for eyebrow labels — a technical-readout tracking.
fn track(s: &str) -> String {
    s.to_uppercase()
        .chars()
        .map(|c| c.to_string())
        .collect::<Vec<_>>()
        .join("\u{2009}")
}

/// Four rivet dots inset into the corners of a plate.
fn rivets(p: &egui::Painter, rect: Rect, c: Color32) {
    let d = 4.5;
    for pos in [
        rect.left_top() + Vec2::splat(d),
        rect.right_top() + Vec2::new(-d, d),
        rect.left_bottom() + Vec2::new(d, -d),
        rect.right_bottom() + Vec2::splat(-d),
    ] {
        p.circle_filled(pos, 1.15, c);
    }
}

/// Paint a stencilled pennant plate (a navy's real hull prefix — USS/HMS/IJN),
/// left edge at `left`, centred on `cy`. Returns the width painted.
fn paint_pennant(p: &egui::Painter, left: f32, cy: f32, code: &str, accent: Color32, size: f32) -> f32 {
    let galley = p.layout_no_wrap(code.to_uppercase(), stencil(size), accent);
    let (padx, pady) = (size * 0.5, size * 0.26);
    let w = galley.size().x + padx * 2.0;
    let h = galley.size().y + pady * 2.0;
    let rect = Rect::from_min_size(Pos2::new(left, cy - h / 2.0), Vec2::new(w, h));
    p.rect_filled(rect, Rounding::same(2.0), col::INK);
    p.rect_stroke(rect, Rounding::same(2.0), Stroke::new(1.0, alpha(accent, 200)));
    p.circle_filled(rect.left_top() + Vec2::splat(3.0), 1.0, scale(accent, 0.55));
    p.circle_filled(rect.right_bottom() + Vec2::splat(-3.0), 1.0, scale(accent, 0.55));
    p.galley(rect.min + Vec2::new(padx, pady), galley, accent);
    w
}

/// Allocate + paint a pennant plate inline in a ui layout.
fn pennant_chip(ui: &mut egui::Ui, code: &str, accent: Color32, size: f32) {
    let g = ui.painter().layout_no_wrap(code.to_uppercase(), stencil(size), accent);
    let want = Vec2::new(g.size().x + size, g.size().y + size * 0.52);
    let (r, _) = ui.allocate_exact_size(want, Sense::hover());
    paint_pennant(ui.painter(), r.min.x, r.center().y, code, accent, size);
}

/// A small filled tag (e.g. "ON STATION") with ink text.
fn paint_tag(p: &egui::Painter, min: Pos2, text: &str, fill: Color32, size: f32) -> Rect {
    let galley = p.layout_no_wrap(text.to_uppercase(), mono(size), col::INK);
    let (padx, pady) = (6.0, 3.0);
    let rect = Rect::from_min_size(min, galley.size() + Vec2::new(padx * 2.0, pady * 2.0));
    p.rect_filled(rect, Rounding::same(2.0), fill);
    p.galley(rect.min + Vec2::new(padx, pady), galley, col::INK);
    rect
}

/// Radar scope: range rings, a rotating fading sweep, and a progress arc.
fn radar(p: &egui::Painter, center: Pos2, radius: f32, frac: f32, accent: Color32, t: f32) {
    use std::f32::consts::{FRAC_PI_2, TAU};
    p.circle_filled(center, radius, col::INK);
    for k in 1..=3 {
        p.circle_stroke(center, radius * k as f32 / 3.0, Stroke::new(1.0, col::RIVET));
    }
    p.line_segment([center - Vec2::new(radius, 0.0), center + Vec2::new(radius, 0.0)], Stroke::new(1.0, col::RIVET));
    p.line_segment([center - Vec2::new(0.0, radius), center + Vec2::new(0.0, radius)], Stroke::new(1.0, col::RIVET));

    // sweep: a fading fan trailing the leading edge
    let lead = t * 2.4;
    let span = 1.0;
    let steps = 26;
    let mut mesh = egui::epaint::Mesh::default();
    mesh.colored_vertex(center, alpha(accent, 55));
    for i in 0..=steps {
        let f = i as f32 / steps as f32;
        let ang = lead - span * f;
        let a = (55.0 * (1.0 - f)) as u8;
        mesh.colored_vertex(center + Vec2::angled(ang) * radius, alpha(accent, a));
    }
    for i in 0..steps {
        mesh.add_triangle(0, i as u32 + 1, i as u32 + 2);
    }
    p.add(mesh);
    p.line_segment([center, center + Vec2::angled(lead) * radius], Stroke::new(1.5, accent));

    // progress arc, from 12 o'clock clockwise
    let n = 64;
    let pts: Vec<Pos2> = (0..=n)
        .map(|i| center + Vec2::angled(-FRAC_PI_2 + (i as f32 / n as f32) * frac * TAU) * radius)
        .collect();
    p.add(Shape::line(pts, Stroke::new(2.5, accent)));
}

/// Paint a monitor-shaped preview of how a skin's thumbnail would sit on screen under `fit`.
#[allow(clippy::too_many_arguments)]
fn monitor_preview(
    painter: &egui::Painter, area: Rect, aspect: f32, fit: Fit,
    thumb: Option<&TextureHandle>, emblem: Option<&TextureHandle>, tint: Color32, content_aspect: f32,
) {
    let ta = content_aspect.max(0.01); // native aspect of the painting being previewed
    let mr = fit_rect(area, aspect);
    painter.rect_filled(mr.expand(4.0), Rounding::same(3.0), col::INK);
    gradient_rect(painter, mr, hex("#1a1630"), hex("#2c1c3e"));
    let full = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
    if let Some(tex) = thumb {
        match fit {
            Fit::Stretch => { painter.image(tex.id(), mr, full, Color32::WHITE); }
            Fit::Fit => { painter.image(tex.id(), fit_rect(mr, ta), full, Color32::WHITE); }
            Fit::Crop => {
                let fa = mr.width() / mr.height().max(1.0);
                let uv = if ta < fa {
                    let v = ta / fa; // visible vertical fraction of the painting
                    Rect::from_min_max(Pos2::new(0.0, 0.5 - v / 2.0), Pos2::new(1.0, 0.5 + v / 2.0))
                } else {
                    let h = fa / ta;
                    Rect::from_min_max(Pos2::new(0.5 - h / 2.0, 0.0), Pos2::new(0.5 + h / 2.0, 1.0))
                };
                painter.image(tex.id(), mr, uv, Color32::WHITE);
            }
        }
    } else {
        if let Some(em) = emblem {
            let s = (mr.height() * 0.55).min(110.0);
            let er = Rect::from_center_size(mr.center(), Vec2::splat(s));
            painter.image(em.id(), er, full, Color32::from_rgba_unmultiplied(tint.r(), tint.g(), tint.b(), 150));
        }
        painter.text(mr.center_bottom() - Vec2::new(0.0, 16.0), Align2::CENTER_CENTER, track("no L2D on file"), mono(9.5), col::HAZE);
    }
    painter.rect_stroke(mr, Rounding::same(1.0), Stroke::new(1.0, alpha(tint, 200)));
    // scope corner ticks
    let t = 8.0;
    for (c, dx, dy) in [
        (mr.left_top(), 1.0, 1.0), (mr.right_top(), -1.0, 1.0),
        (mr.left_bottom(), 1.0, -1.0), (mr.right_bottom(), -1.0, -1.0),
    ] {
        painter.line_segment([c, c + Vec2::new(dx * t, 0.0)], Stroke::new(1.5, tint));
        painter.line_segment([c, c + Vec2::new(0.0, dy * t)], Stroke::new(1.5, tint));
    }
}

#[allow(clippy::too_many_arguments)]
fn faction_tile(
    ui: &mut egui::Ui, ctx: &Context, textures: &mut HashMap<String, TextureHandle>, root: &Path,
    key: &str, name: &str, short: &str, count: usize, pal: &crate::model::Palette,
) -> bool {
    let size = Vec2::new(252.0, 150.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    if ui.is_rect_visible(rect) {
        let emblem = load_tex(textures, ctx, root, &format!("assets/emblems/{key}.png"));
        let p = ui.painter_at(rect);
        let accent = hex(&pal.accent);
        let hovered = resp.hovered();
        let round = Rounding::same(4.0);
        p.rect_filled(rect, round, if hovered { scale(col::STEEL_HI, 1.28) } else { col::STEEL_HI });
        // faction ensign, tinted, anchored to the right edge
        if let Some(tex) = &emblem {
            let em = 122.0;
            let er = Rect::from_min_size(Pos2::new(rect.max.x - em + 10.0, rect.center().y - em / 2.0), Vec2::splat(em));
            let a = if hovered { 66 } else { 42 };
            p.image(tex.id(), er, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), alpha(accent, a));
        }
        // hull stripe
        p.rect_filled(Rect::from_min_max(rect.min, Pos2::new(rect.min.x + 5.0, rect.max.y)), round, accent);
        rivets(&p, rect.shrink(3.0), col::RIVET);
        paint_pennant(&p, rect.min.x + 18.0, rect.min.y + 27.0, short, accent, 13.0);
        p.text(Pos2::new(rect.min.x + 18.0, rect.min.y + 48.0), Align2::LEFT_TOP, name.to_uppercase(), oswald_b(21.0), col::CHALK);
        let wy = rect.max.y - 32.0;
        p.line_segment([Pos2::new(rect.min.x + 18.0, wy), Pos2::new(rect.max.x - 16.0, wy)], Stroke::new(1.0, col::RIVET));
        let unit = if count == 1 { "ship" } else { "ships" };
        p.text(Pos2::new(rect.min.x + 18.0, rect.max.y - 24.0), Align2::LEFT_TOP, track(&format!("{count} {unit}")), mono(11.0), col::HAZE);
        p.rect_stroke(rect, round, Stroke::new(if hovered { 1.5 } else { 1.0 }, if hovered { accent } else { col::RIVET }));
    }
    resp.clicked()
}

#[allow(clippy::too_many_arguments)]
fn skin_card(
    ui: &mut egui::Ui, ctx: &Context, textures: &mut HashMap<String, TextureHandle>, root: &Path,
    thumb: &str, ship: &str, sub: &str, oath: bool, live: bool, fav: bool, fkey: &str, accent: Color32, cursor: bool,
) -> egui::Response {
    let cw = 176.0;
    let img_h = cw / 0.75;
    let plate_h = 58.0;
    let ch = img_h + plate_h;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(cw, ch), Sense::click());
    if ui.is_rect_visible(rect) {
        let round = Rounding::same(4.0);
        let img_rect = Rect::from_min_size(rect.min, Vec2::new(cw, img_h));
        let tex = load_tex(textures, ctx, root, thumb);
        let emblem = if tex.is_none() { load_tex(textures, ctx, root, &format!("assets/emblems/{fkey}.png")) } else { None };
        let p = ui.painter_at(rect);
        p.rect_filled(rect, round, col::STEEL_HI);
        if let Some(tex) = &tex {
            p.image(tex.id(), img_rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
        } else {
            p.rect_filled(img_rect, round, col::INK);
            if let Some(tex) = &emblem {
                let er = Rect::from_center_size(img_rect.center() - Vec2::new(0.0, 12.0), Vec2::splat(96.0));
                p.image(tex.id(), er, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), alpha(accent, 130));
            }
            p.text(img_rect.center_bottom() - Vec2::new(0.0, 16.0), Align2::CENTER_CENTER, track("no L2D on file"), mono(9.5), col::HAZE_DIM);
        }
        // waterline: the boundary between artwork and nameplate
        p.line_segment([Pos2::new(rect.min.x, img_rect.max.y), Pos2::new(rect.max.x, img_rect.max.y)], Stroke::new(2.0, accent));
        // engraved nameplate
        p.text(Pos2::new(rect.min.x + 9.0, img_rect.max.y + 8.0), Align2::LEFT_TOP, trunc(&ship.to_uppercase(), 17), oswald(15.5), col::CHALK);
        p.text(Pos2::new(rect.min.x + 9.0, img_rect.max.y + 30.0), Align2::LEFT_TOP, trunc(sub, 26), plex_it(11.0), col::HAZE);

        if live {
            paint_tag(&p, rect.min + Vec2::splat(7.0), "on station", col::SIGNAL, 9.5);
        }
        // status glyphs on a small ink chip so they read over any artwork
        if fav || oath {
            let n = fav as i32 + oath as i32;
            let chip = Rect::from_min_size(Pos2::new(rect.max.x - 7.0 - 20.0 * n as f32, rect.min.y + 7.0), Vec2::new(20.0 * n as f32, 20.0));
            p.rect_filled(chip, Rounding::same(3.0), alpha(col::INK, 170));
            let mut rx = rect.max.x - 12.0;
            if fav { p.text(Pos2::new(rx, rect.min.y + 8.0), Align2::RIGHT_TOP, "★", FontId::proportional(14.0), col::GOLD); rx -= 20.0; }
            if oath { p.text(Pos2::new(rx, rect.min.y + 8.0), Align2::RIGHT_TOP, "⚭", FontId::proportional(15.0), col::GOLD); }
        }

        if cursor {
            p.rect_stroke(rect, round, Stroke::new(2.0, accent));
            // corner ticks — a targeting reticle for the keyboard focus
            let t = 9.0;
            for (c, dx, dy) in [
                (rect.left_top(), 1.0, 1.0), (rect.right_top(), -1.0, 1.0),
                (rect.left_bottom(), 1.0, -1.0), (rect.right_bottom(), -1.0, -1.0),
            ] {
                p.line_segment([c, c + Vec2::new(dx * t, 0.0)], Stroke::new(2.0, col::CHALK));
                p.line_segment([c, c + Vec2::new(0.0, dy * t)], Stroke::new(2.0, col::CHALK));
            }
        } else if resp.hovered() {
            p.rect_stroke(rect, round, Stroke::new(1.0, accent));
        }
    }
    resp
}
