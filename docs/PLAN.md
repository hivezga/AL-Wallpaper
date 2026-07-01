# Azur Lane Live2D Wallpaper Picker — Detailed Plan

A DE-independent (COSMIC + Hyprland first) desktop GUI to browse the 250 extracted
Azur Lane Live2D skins **by faction**, preview them, and apply any as an animated
live wallpaper. Faction-themed, two-level navigation. Precursor to a future
full Azur Lane Hyprland rice.

---

## 1. Goals & non-goals

**Goals**
- Two-level nav: **faction-emblem grid → faction gallery** (thumbnail + character name + skin name).
- **Whole-UI reskin per faction** (palette, accent, emblem watermark, header banner).
- Click a skin → preview (thumbnail / looping mp4) → **Apply** (sets live wallpaper) + **Set default**.
- Works independently of the desktop environment; first-class on **COSMIC** and **Hyprland** (both `wlr-layer-shell`).
- Reuse the proven pipeline already built in `~/azurlane/wallpaper/` (render.js → mp4 → mpvpaper).

**Non-goals (explicit traps avoided)**
- No in-app Live2D/Cubism engine (binding Cubism Native SDK = multi-week rabbit hole). Preview = thumbnail, optionally the mp4 via a video frame; the *real* live preview is applying it.
- No support for non-layer-shell compositors (GNOME/KDE Wayland) — out of scope per target list.
- No decryption of the game's encrypted `sharecfgdata` (confirmed encrypted; use community JSON instead).

---

## 2. Target environments & portability

| Concern | Approach |
|---|---|
| GUI runtime | `eframe`/`egui` over `winit` → runs on any Wayland/X11, DE-agnostic |
| Monitor detection | Wayland `xdg-output` protocol in-app; CLI falls back `wlr-randr`→`cosmic-randr`→`hyprctl` |
| Apply wallpaper | `mpvpaper` per output (layer-shell). Works on COSMIC `cosmic-comp` + Hyprland |
| Theming | Fully custom egui `Visuals` per faction — NOT tied to DE theme |
| Config | `~/.config/al-wallpaper/config.toml` (XDG) |

---

## 3. Architecture

```
                ┌─────────────────────────────────────────┐
                │  al-wallpaper (Rust / egui)               │
                │                                           │
  catalog.json ─┤  Model: factions[], skins[]              │
  factions.json─┤  View:  FactionGrid ⇄ FactionGallery     │
  assets/thumbs ┤  Theme: per-faction Visuals + emblem     │
  assets/emblems┤  Action: apply(skin) / set_default(skin) │
                └───────────────┬───────────────────────────┘
                                │ spawns
                                ▼
        scripts/apply.sh  (portable wrapper around render + mpvpaper)
                                │
                  render.js (reused) ── mp4 cache ── mpvpaper per output
```

**Data flow at startup:** load `catalog.json` + `factions.json` → group skins by faction →
render faction grid. Click faction → filter skins, apply faction theme → gallery.
Click skin → detail panel → Apply spawns `apply.sh <skin>` (renders per-monitor if not cached, launches mpvpaper, writes default).

---

## 4. Repository layout (`~/azurlane/al-wallpaper/`)

```
al-wallpaper/
├─ Cargo.toml                  # eframe, egui_extras, image, serde, serde_json, toml, anyhow, rfd?
├─ docs/PLAN.md                # this file
├─ README.md
├─ data/
│  ├─ raw/                     # pulled sharecfgdata bundles (encrypted; kept for reference)
│  ├─ sources/                 # downloaded community JSON (ship_skin_template, ship_data_*)
│  ├─ catalog.json             # GENERATED — the app's primary input
│  └─ factions.json            # GENERATED/curated — faction order, names, palettes, emblem file
├─ assets/
│  ├─ thumbs/<skin>.png        # GENERATED (250) — 480×640 card thumbnails
│  └─ emblems/<faction>.png    # faction emblems (extracted or stylized)
├─ scripts/
│  ├─ build_catalog.js         # joins community JSON → catalog.json (+ updates aliases)
│  ├─ thumb.js                 # (symlink/reuse from ../wallpaper) batch thumbnailer
│  └─ apply.sh                 # portable apply (monitor detect + render + mpvpaper + default)
└─ src/
   ├─ main.rs                  # eframe bootstrap
   ├─ model.rs                 # Catalog, Faction, Skin structs + loader
   ├─ theme.rs                 # faction palettes → egui Visuals; emblem tinting
   ├─ ui/
   │  ├─ faction_grid.rs       # level 1
   │  ├─ gallery.rs            # level 2 (filtered + themed)
   │  └─ detail.rs             # preview + Apply / Set default
   └─ apply.rs                 # spawn apply.sh, track status, current/default state
```

---

## 5. Data model

### catalog.json (generated)
```json
{
  "generated": "2026-06-29",
  "skins": [
    {
      "codename": "qiye_9",          // == Live2D folder name == apply key
      "ship": "Enterprise",          // English ship name
      "ship_id_group": 10800,
      "faction": "eagle_union",      // canonical faction key
      "rarity": "UR",                // SSR/UR/... (if available)
      "skin_name": "Sundered Blue",  // EN skin display name (from ship_skin_words)
      "is_oath": false,              // _hx variant
      "thumb": "assets/thumbs/qiye_9.png",
      "has_l2d": true
    }
  ]
}
```
Only skins with an extracted L2D folder are included (the 250). `skin_name`/`rarity`
are best-effort; fall back to humanizing the codename if missing.

### factions.json (curated from research, ordered)
```json
{
  "factions": [
    {
      "key": "eagle_union", "name": "Eagle Union", "short": "USS",
      "order": 1, "emblem": "assets/emblems/eagle_union.png",
      "palette": { "bg":"#0d1b2a", "panel":"#13283f", "accent":"#c9a227",
                   "accent2":"#e8e8e8", "text":"#eef2f6", "muted":"#9fb2c4" }
    }
  ]
}
```
Faction set + nationality-int mapping + palettes come from the research agent's report.

---

## 6. Phase 1 — Data foundation  *(in progress)*

1. **Thumbnails** — `thumb.js` over all 250 → `assets/thumbs/` (480×640). *(running in background)*
2. **Metadata** — fetch community JSON (`ship_skin_template`, `ship_data_statistics`/`template`, `ship_skin_words`) for EN server → `data/sources/`. *(awaiting research agent for exact URLs + nationality enum)*
3. **build_catalog.js** — join:
   - `ship_skin_template`: `painting` (codename) → `ship_group`, `skin_name` ref.
   - `ship_data_statistics`/`template`: group → `name`, `nationality`, `rarity`.
   - `ship_skin_words`: skin display names.
   - Map `nationality:int` → faction key via curated enum.
   - Filter to codenames that exist as folders in `out_all/Live2DOutput`.
   - Emit `catalog.json`; also regenerate full `aliases.tsv` (English→codename) for `wallpaper.sh`.
4. **Faction emblems** — best-effort extract from a UI bundle (`squareicon`/`herohrzicon`/etc.) via AssetStudio; if not cleanly found, ship clean stylized emblems. Palettes from research.
5. **Validation** — assert every one of the 250 codenames resolves to a faction (log any "unknown"; bucket them under an "Other/Collab" faction rather than dropping).

**Exit criteria:** `catalog.json` has 250 entries each with a faction; `factions.json` complete; 250 thumbs present; emblems for each non-empty faction.

---

## 7. Phase 2 — egui application

**Crate deps:** `eframe`, `egui`, `egui_extras` (image loader + tables), `image`, `serde`, `serde_json`, `toml`, `anyhow`, `dirs`.

**App state machine**
```
enum Screen { Factions, Gallery { faction: String }, }
struct App {
  catalog: Catalog, factions: Vec<Faction>,
  screen: Screen, search: String,
  current: Option<String>,    // applied skin
  default: Option<String>,    // boot default
  apply_rx: Receiver<ApplyEvent>,  // async apply status
  thumb_cache: egui texture handles (lazy),
}
```

**Level 1 — Faction grid (`faction_grid.rs`)**
- Neutral base theme; responsive grid of faction **emblem cards** (emblem + name + skin count).
- Hover = faction accent glow. Click → `Screen::Gallery`, apply that faction's `Visuals`.

**Level 2 — Faction gallery (`gallery.rs`)**
- Entire window reskinned to the faction palette + faint emblem watermark + header banner ("Iron Blood — 23 skins").
- Back button → return to Level 1 (restore neutral theme).
- Search box filters within faction by ship/skin name.
- Responsive thumbnail grid (egui_extras image loading, lazy texture upload, clip-cull offscreen).
- Card: thumb + ship name + skin name (+ oath ⚭ / rarity badge). Click → detail.

**Detail (`detail.rs`)**
- Larger preview (thumb now; optional mp4 playback later via `egui_video`/frames — deferred).
- Buttons: **Apply** (live), **Set as default** (writes config + default.txt), shows current/applied state.
- Apply runs async (channel) so UI stays responsive; toast on completion/failure.

**Theming (`theme.rs`)**
- `fn faction_visuals(p: &Palette) -> egui::Visuals` — map palette → window/panel/widget colors, selection, hyperlink, rounding.
- Emblem watermark drawn via `Painter` (low-alpha, corner/back).
- Smooth-ish transition acceptable as instant swap v1.

---

## 8. Phase 3 — Apply pipeline (portable)

`scripts/apply.sh` = generalized `wallpaper.sh`:
- Monitor detect: try `wlr-randr --json` → `cosmic-randr list` → `hyprctl -j monitors`; produce `(name WxH)` list.
- For each output: render `out/<skin>_<W>x<H>.mp4` if absent (reuse `render.js`), then `mpvpaper <name> <mp4>`.
- Write chosen skin to `~/.config/al-wallpaper/default` + keep `~/azurlane/wallpaper/default.txt` in sync for the existing autostart.
- App calls `apply.sh <skin>`; parses stdout lines for progress events.

Autostart (existing `al-live2d-wallpaper.desktop`) keeps working; update its wrapper to read the XDG config default.

---

## 9. Phase 4 — Config, polish, packaging

- `config.toml`: `default_skin`, `thumb_dir`, `models_dir`, `last_faction`, render knobs (fill/oy/fps), window size.
- Graceful empty/error states (missing thumb → placeholder; unknown faction → Other).
- `cargo build --release` → single binary `al-wallpaper`; `.desktop` launcher (optional, NoDisplay=false) so it appears in app menus.
- README with usage + how the data was built.

---

## 10. Future hooks (Hyprland AL rice — later, not now)

- App is a thin layer over `apply.sh` + `catalog.json`; the rice can call the same CLI core.
- Expose `al-wallpaper --set <skin>` / `--random [faction]` headless subcommands for keybinds.
- factions.json palettes can drive the wider rice (pywal-like): export active faction palette to a file other rice components read.

---

## 11. Risks & decisions

| Risk | Mitigation |
|---|---|
| Community metadata stale / missing some new skins | Fall back to humanized codename + "Other" faction; never drop a skin |
| Faction emblems hard to extract cleanly | Ship stylized emblems; extraction is best-effort polish |
| egui texture memory for 250 thumbs | Lazy-load + free offscreen; thumbs are small (480×640 PNG) |
| Per-monitor mp4 render latency on first apply | Pre-rendered cache; show progress; Enterprise default already cached |
| Hyprland vs COSMIC output APIs differ | In-app xdg-output + CLI multi-tool fallback |

**Decisions locked:** egui (not iced/libcosmic); two-level nav; metadata from community JSON (not bundle decryption); preview = thumbnail (no in-app Cubism); apply = mpvpaper per output.
