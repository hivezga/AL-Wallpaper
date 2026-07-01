use eframe::egui::Color32;
use serde::Deserialize;
use std::{collections::HashMap, fs, path::PathBuf};

#[derive(Deserialize, Clone)]
pub struct Skin {
    pub codename: String,
    pub ship: String,
    pub faction: String,
    #[serde(default)]
    pub rarity: Option<String>,
    #[serde(default)]
    pub skin_name: Option<String>,
    #[serde(default)]
    pub is_oath: bool,
    pub thumb: String,
}

#[derive(Deserialize)]
struct Catalog {
    skins: Vec<Skin>,
}

#[derive(Deserialize, Clone)]
pub struct Palette {
    pub bg: String,
    pub panel: String,
    pub accent: String,
    pub accent2: String,
    pub text: String,
    pub muted: String,
}

#[derive(Deserialize, Clone)]
pub struct Faction {
    pub key: String,
    pub name: String,
    pub short: String,
    pub order: i32,
    pub palette: Palette,
}

#[derive(Deserialize)]
struct FactionsFile {
    factions: Vec<Faction>,
}

pub struct Data {
    pub root: PathBuf,
    pub factions: Vec<Faction>,
    pub skins: Vec<Skin>,
    pub by_faction: HashMap<String, Vec<usize>>,
    pub by_code: HashMap<String, usize>,
}

#[derive(Clone)]
pub struct Monitor {
    pub name: String,
    pub w: u32,
    pub h: u32,
}

#[derive(Deserialize, Clone)]
pub struct RotationCfg {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "def_interval")]
    pub interval: String,
    #[serde(default = "def_scope")]
    pub scope: String,
    #[serde(default)]
    pub per_monitor: bool,
    #[serde(default)]
    pub last_run: i64,
}
impl Default for RotationCfg {
    fn default() -> Self {
        Self { enabled: false, interval: def_interval(), scope: def_scope(), per_monitor: false, last_run: 0 }
    }
}
fn def_interval() -> String { "30m".into() }
fn def_scope() -> String { "all".into() }

#[derive(Deserialize, Clone)]
pub struct PowerCfg {
    /// "off" | "pause" | "stop" — pause/stop the video when the wallpaper is hidden (e.g. fullscreen game).
    #[serde(default = "def_power_mode")]
    pub mode: String,
    /// 0 = uncapped; otherwise cap playback to N fps to save power.
    #[serde(default)]
    pub fps_cap: u32,
    /// "off" | "pause" — freeze the wallpaper while running on battery (handled by power-daemon.sh).
    #[serde(default = "def_power_battery")]
    pub battery: String,
}
impl Default for PowerCfg {
    fn default() -> Self {
        Self { mode: def_power_mode(), fps_cap: 0, battery: def_power_battery() }
    }
}
fn def_power_mode() -> String { "pause".into() }
fn def_power_battery() -> String { "off".into() }

#[derive(Deserialize, Default)]
pub struct State {
    #[serde(default)]
    pub outputs: HashMap<String, String>,
    #[serde(default)]
    pub fit: HashMap<String, String>,
    #[serde(default)]
    pub rotation: RotationCfg,
    #[serde(default)]
    pub power: PowerCfg,
}

/// How a wallpaper is scaled to a monitor.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Fit {
    Fit,
    Stretch,
    Crop,
}

impl Fit {
    pub fn key(self) -> &'static str {
        match self {
            Fit::Fit => "fit",
            Fit::Stretch => "stretch",
            Fit::Crop => "crop",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Fit::Fit => "Fit",
            Fit::Stretch => "Stretch",
            Fit::Crop => "Crop",
        }
    }
    pub fn from_key(s: &str) -> Self {
        match s {
            "stretch" => Fit::Stretch,
            "crop" => Fit::Crop,
            _ => Fit::Fit,
        }
    }
    pub const ALL: [Fit; 3] = [Fit::Fit, Fit::Stretch, Fit::Crop];
}

pub fn load_state(cfg_dir: &std::path::Path) -> State {
    fs::read_to_string(cfg_dir.join("state.json"))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

#[derive(Deserialize, Default, serde::Serialize)]
pub struct Favorites {
    #[serde(default)]
    pub favorites: Vec<String>,
}

pub fn load_favorites(root: &std::path::Path) -> std::collections::HashSet<String> {
    fs::read_to_string(root.join("data/favorites.json"))
        .ok()
        .and_then(|s| serde_json::from_str::<Favorites>(&s).ok())
        .map(|f| f.favorites.into_iter().collect())
        .unwrap_or_default()
}

pub fn save_favorites(root: &std::path::Path, favs: &std::collections::HashSet<String>) {
    let mut v: Vec<String> = favs.iter().cloned().collect();
    v.sort();
    let _ = fs::write(
        root.join("data/favorites.json"),
        serde_json::to_string_pretty(&Favorites { favorites: v }).unwrap_or_default(),
    );
}

impl Data {
    pub fn load(root: PathBuf) -> anyhow::Result<Self> {
        let cat: Catalog = serde_json::from_str(&fs::read_to_string(root.join("data/catalog.json"))?)?;
        let ff: FactionsFile =
            serde_json::from_str(&fs::read_to_string(root.join("data/factions.json"))?)?;
        let mut factions = ff.factions;
        factions.sort_by_key(|f| f.order);
        let mut by_faction: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, s) in cat.skins.iter().enumerate() {
            by_faction.entry(s.faction.clone()).or_default().push(i);
        }
        // sort each faction's skins by ship then codename for stable display
        for v in by_faction.values_mut() {
            v.sort_by(|&a, &b| {
                cat.skins[a]
                    .ship
                    .to_lowercase()
                    .cmp(&cat.skins[b].ship.to_lowercase())
                    .then(cat.skins[a].codename.cmp(&cat.skins[b].codename))
            });
        }
        let by_code = cat
            .skins
            .iter()
            .enumerate()
            .map(|(i, s)| (s.codename.clone(), i))
            .collect();
        Ok(Self {
            root,
            factions,
            skins: cat.skins,
            by_faction,
            by_code,
        })
    }

    pub fn ship_of(&self, code: &str) -> String {
        self.by_code
            .get(code)
            .map(|&i| self.skins[i].ship.clone())
            .unwrap_or_else(|| code.to_string())
    }

    pub fn faction(&self, key: &str) -> Option<&Faction> {
        self.factions.iter().find(|f| f.key == key)
    }

    pub fn count(&self, key: &str) -> usize {
        self.by_faction.get(key).map(|v| v.len()).unwrap_or(0)
    }
}

pub fn hex(s: &str) -> Color32 {
    let s = s.trim_start_matches('#');
    if s.len() < 6 {
        return Color32::GRAY;
    }
    let r = u8::from_str_radix(&s[0..2], 16).unwrap_or(200);
    let g = u8::from_str_radix(&s[2..4], 16).unwrap_or(200);
    let b = u8::from_str_radix(&s[4..6], 16).unwrap_or(200);
    Color32::from_rgb(r, g, b)
}

/// Scale an opaque color toward black/white by factor (0..2), keeping alpha opaque.
pub fn scale(c: Color32, f: f32) -> Color32 {
    let s = |v: u8| (v as f32 * f).clamp(0.0, 255.0) as u8;
    Color32::from_rgb(s(c.r()), s(c.g()), s(c.b()))
}
