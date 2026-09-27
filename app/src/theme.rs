//! Interface themes. Each theme sets the panels, the controls and the canvas
//! (desk, page, ink and on-canvas marks). Exported SVGs are always black on
//! white; themes only change what is shown on screen.
use eframe::egui::{self, Color32, Rounding, Stroke, Vec2};
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ThemeId { Graphite, Midnight, Slate, Studio, Paper, Sage }

impl ThemeId {
    pub const ALL: [ThemeId; 6] = [ThemeId::Graphite, ThemeId::Midnight, ThemeId::Slate, ThemeId::Studio, ThemeId::Paper, ThemeId::Sage];
    pub fn key(self) -> &'static str { match self { ThemeId::Graphite => "graphite", ThemeId::Midnight => "midnight", ThemeId::Slate => "slate", ThemeId::Studio => "studio", ThemeId::Paper => "paper", ThemeId::Sage => "sage" } }
    pub fn from_key(k: &str) -> Option<ThemeId> { ThemeId::ALL.into_iter().find(|t| t.key() == k) }
    pub fn theme(self) -> &'static Theme { match self { ThemeId::Graphite => &GRAPHITE, ThemeId::Midnight => &MIDNIGHT, ThemeId::Slate => &SLATE, ThemeId::Studio => &STUDIO, ThemeId::Paper => &PAPER, ThemeId::Sage => &SAGE } }
}

pub struct Theme {
    pub name: &'static str,
    pub blurb: &'static str,
    pub dark: bool,
    // interface
    pub bg: Color32,       // panels
    pub surface: Color32,  // buttons, cards, inputs
    pub hover: Color32,
    pub active: Color32,   // pressed controls, slider rails, checkbox boxes
    pub border: Color32,
    pub text: Color32,
    pub dim: Color32,      // secondary text
    pub accent: Color32,   // selected tabs, tools and slider fill
    // canvas
    pub desk: Color32,
    pub paper: Color32,
    pub ink: Color32,
    pub guide: Color32,    // backbone guide line
    pub mark: Color32,     // handles, selection and the transform box
    pub grid: Color32,
    pub ridge: Color32,
    pub crease: Color32,
}

const fn hex(v: u32) -> Color32 { Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8) }

pub static GRAPHITE: Theme = Theme { name: "Graphite", blurb: "Soft dark grey. Easy on the eyes for long sessions.", dark: true,
    bg: hex(0x1f2023), surface: hex(0x2a2b2f), hover: hex(0x34363b), active: hex(0x44464d), border: hex(0x323338), text: hex(0xe4e4e7), dim: hex(0x9a9ba2), accent: hex(0x4a4c54),
    desk: hex(0x161719), paper: hex(0x27282c), ink: hex(0xe6e6e8), guide: hex(0x686a72), mark: hex(0xf2f2f3), grid: hex(0x303136), ridge: hex(0x7fb0e0), crease: hex(0xe08a72) };
pub static MIDNIGHT: Theme = Theme { name: "Midnight", blurb: "Near-black with bright ink. Highest contrast.", dark: true,
    bg: hex(0x0d0d0e), surface: hex(0x19191b), hover: hex(0x232326), active: hex(0x34343a), border: hex(0x1f1f22), text: hex(0xededee), dim: hex(0x8b8b92), accent: hex(0x3a3a40),
    desk: hex(0x000000), paper: hex(0x121214), ink: hex(0xf2f2f2), guide: hex(0x5c5c63), mark: hex(0xffffff), grid: hex(0x1f1f23), ridge: hex(0x86b7e8), crease: hex(0xe8917a) };
pub static SLATE: Theme = Theme { name: "Slate", blurb: "Dark interface with a white page, like the printed pattern.", dark: true,
    bg: hex(0x25272b), surface: hex(0x303237), hover: hex(0x3a3d43), active: hex(0x4a4d55), border: hex(0x36383d), text: hex(0xe6e6e8), dim: hex(0xa0a2a8), accent: hex(0x50535b),
    desk: hex(0x1b1c1f), paper: hex(0xf6f6f4), ink: hex(0x19191a), guide: hex(0x9a9ea5), mark: hex(0x2a2c31), grid: hex(0xe3e4e6), ridge: hex(0x2f6fb0), crease: hex(0xb4533a) };
pub static STUDIO: Theme = Theme { name: "Studio", blurb: "Clean light grey with black ink.", dark: false,
    bg: hex(0xf3f3f4), surface: hex(0xe5e5e8), hover: hex(0xdadade), active: hex(0xc4c4ca), border: hex(0xdcdce0), text: hex(0x18181b), dim: hex(0x6b6b73), accent: hex(0xcfcfd6),
    desk: hex(0xdcdce0), paper: hex(0xffffff), ink: hex(0x141414), guide: hex(0xa1a1aa), mark: hex(0x27272a), grid: hex(0xe8e8eb), ridge: hex(0x2f6fb0), crease: hex(0xb4533a) };
pub static PAPER: Theme = Theme { name: "Paper", blurb: "Warm off-white, like a drawing on cartridge paper.", dark: false,
    bg: hex(0xf5f3ee), surface: hex(0xe9e5dd), hover: hex(0xdfdad0), active: hex(0xcac3b6), border: hex(0xdfd9ce), text: hex(0x2a2723), dim: hex(0x7a746b), accent: hex(0xd6cfc2),
    desk: hex(0xe2ddd3), paper: hex(0xfffdf8), ink: hex(0x2a2723), guide: hex(0xb3aa9a), mark: hex(0x3a352f), grid: hex(0xeee9df), ridge: hex(0x3a6f9e), crease: hex(0xa9553d) };
pub static SAGE: Theme = Theme { name: "Sage", blurb: "The original light theme with green accents.", dark: false,
    bg: hex(0xeef0ec), surface: hex(0xdfe4de), hover: hex(0xd2ddd5), active: hex(0xb9cbbf), border: hex(0xd3dad3), text: hex(0x1c211d), dim: hex(0x66706a), accent: hex(0xbcdcce),
    desk: hex(0xcfd4cd), paper: hex(0xffffff), ink: hex(0x151a16), guide: hex(0x9bb6ac), mark: hex(0x286b55), grid: hex(0xdfe8e2), ridge: hex(0x246a9b), crease: hex(0xa24434) };

/// Canvas colours in use, after the "white page" option.
#[derive(Clone, Copy)]
pub struct Canvas { pub desk: Color32, pub paper: Color32, pub ink: Color32, pub guide: Color32, pub mark: Color32, pub grid: Color32, pub ridge: Color32, pub crease: Color32 }

impl Theme {
    pub fn canvas(&self, white_page: bool) -> Canvas {
        if white_page && self.paper.r() < 128 {
            Canvas { desk: self.desk, paper: hex(0xffffff), ink: hex(0x141414), guide: hex(0xa1a1aa), mark: hex(0x27272a), grid: hex(0xe8e8eb), ridge: hex(0x2f6fb0), crease: hex(0xb4533a) }
        } else {
            Canvas { desk: self.desk, paper: self.paper, ink: self.ink, guide: self.guide, mark: self.mark, grid: self.grid, ridge: self.ridge, crease: self.crease }
        }
    }

    pub fn visuals(&self) -> egui::Visuals {
        let mut v = if self.dark { egui::Visuals::dark() } else { egui::Visuals::light() };
        let r = Rounding::same(6.0);
        v.override_text_color = None;
        v.panel_fill = self.bg;
        v.window_fill = self.bg;
        v.window_stroke = Stroke::new(1.0, self.border);
        v.window_rounding = Rounding::same(10.0);
        v.menu_rounding = Rounding::same(8.0);
        v.faint_bg_color = self.surface;
        v.extreme_bg_color = self.surface;
        v.code_bg_color = self.surface;
        v.hyperlink_color = self.text;
        v.window_shadow = egui::epaint::Shadow { offset: Vec2::new(0.0, 8.0), blur: 24.0, spread: 0.0, color: Color32::from_black_alpha(if self.dark { 110 } else { 40 }) };
        v.popup_shadow = egui::epaint::Shadow { offset: Vec2::new(0.0, 4.0), blur: 14.0, spread: 0.0, color: Color32::from_black_alpha(if self.dark { 90 } else { 30 }) };
        v.selection.bg_fill = self.accent;
        v.selection.stroke = Stroke::new(1.0, self.text);
        v.slider_trailing_fill = true;
        v.text_cursor.stroke = Stroke::new(2.0, self.text);
        v.striped = false;

        let w = &mut v.widgets;
        w.noninteractive.bg_fill = self.bg;
        w.noninteractive.weak_bg_fill = self.bg;
        w.noninteractive.bg_stroke = Stroke::new(1.0, self.border); // separators
        w.noninteractive.fg_stroke = Stroke::new(1.0, self.text);
        w.noninteractive.rounding = r;

        w.inactive.bg_fill = self.active;        // slider rails, checkbox boxes
        w.inactive.weak_bg_fill = self.surface;  // buttons
        w.inactive.bg_stroke = Stroke::NONE;
        w.inactive.fg_stroke = Stroke::new(1.2, self.text);
        w.inactive.rounding = r;
        w.inactive.expansion = 0.0;

        w.hovered.bg_fill = self.hover;
        w.hovered.weak_bg_fill = self.hover;
        w.hovered.bg_stroke = Stroke::new(1.0, self.active);
        w.hovered.fg_stroke = Stroke::new(1.5, self.text);
        w.hovered.rounding = r;
        w.hovered.expansion = 0.0;

        w.active.bg_fill = self.active;
        w.active.weak_bg_fill = self.active;
        w.active.bg_stroke = Stroke::new(1.0, self.dim);
        w.active.fg_stroke = Stroke::new(1.5, self.text);
        w.active.rounding = r;
        w.active.expansion = 0.0;

        w.open.bg_fill = self.hover;
        w.open.weak_bg_fill = self.hover;
        w.open.bg_stroke = Stroke::new(1.0, self.active);
        w.open.fg_stroke = Stroke::new(1.2, self.text);
        w.open.rounding = r;
        v
    }

    /// Install this theme in both of egui's light and dark slots, so a change
    /// of the Windows light/dark setting cannot swap it out.
    pub fn apply(&self, ctx: &egui::Context) {
        let v = self.visuals();
        ctx.set_theme(if self.dark { egui::Theme::Dark } else { egui::Theme::Light });
        for slot in [egui::Theme::Light, egui::Theme::Dark] {
            ctx.set_visuals_of(slot, v.clone());
            ctx.style_mut_of(slot, |s| {
                use egui::{FontFamily, FontId, TextStyle};
                s.text_styles.insert(TextStyle::Body, FontId::proportional(14.5));
                s.text_styles.insert(TextStyle::Button, FontId::proportional(14.5));
                s.text_styles.insert(TextStyle::Small, FontId::proportional(12.5));
                s.text_styles.insert(TextStyle::Heading, FontId::new(17.0, FontFamily::Name("semibold".into())));
                s.text_styles.insert(TextStyle::Monospace, FontId::monospace(13.0));
                s.spacing.item_spacing = Vec2::new(8.0, 7.0);
                s.spacing.button_padding = Vec2::new(10.0, 5.0);
                s.spacing.slider_width = 150.0;
                s.spacing.interact_size = Vec2::new(40.0, 26.0);
                s.spacing.icon_width = 16.0;
                s.spacing.icon_width_inner = 9.0;
                s.spacing.combo_width = 150.0;
                s.spacing.menu_margin = egui::Margin::same(8.0);
                s.spacing.window_margin = egui::Margin::same(16.0);
            });
        }
    }
}

/// Use Windows' own interface font when it is installed; egui's bundled font
/// stays as the fallback for anything Segoe lacks.
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let dir = std::env::var_os("WINDIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("C:\\Windows")).join("Fonts");
    let regular = std::fs::read(dir.join("segoeui.ttf")).ok();
    let semibold = std::fs::read(dir.join("seguisb.ttf")).ok();
    let fallback = fonts.families.get(&egui::FontFamily::Proportional).cloned().unwrap_or_default();
    if let Some(b) = regular {
        fonts.font_data.insert("segoe".into(), egui::FontData::from_owned(b));
        fonts.families.entry(egui::FontFamily::Proportional).or_default().insert(0, "segoe".into());
    }
    let mut strong = vec![];
    if let Some(b) = semibold { fonts.font_data.insert("segoe-semibold".into(), egui::FontData::from_owned(b)); strong.push("segoe-semibold".to_string()); }
    strong.extend(fonts.families.get(&egui::FontFamily::Proportional).cloned().unwrap_or(fallback));
    fonts.families.insert(egui::FontFamily::Name("semibold".into()), strong);
    ctx.set_fonts(fonts);
}

/// Preferences kept between sessions, in %APPDATA%\ORNATR\settings.txt.
#[derive(Clone, Copy, PartialEq)]
pub struct Prefs { pub theme: ThemeId, pub white_page: bool, pub ui_scale: f32, /// Last workspace was Chip.
    pub chip: bool,
    /// How roots are drawn: "classic", "smooth" or "exact".
    pub joins: Joins,
    /// Fillet radius in mm for exact joins.
    pub fillet: f32 }

/// The root-join drawing engine chosen in the Canvas settings.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Joins { Classic, Smooth, Exact }
impl Joins {
    pub fn key(self) -> &'static str { match self { Joins::Classic => "classic", Joins::Smooth => "smooth", Joins::Exact => "exact" } }
    pub fn from_key(k: &str) -> Option<Joins> { match k { "classic" => Some(Joins::Classic), "smooth" => Some(Joins::Smooth), "exact" => Some(Joins::Exact), _ => None } }
}

impl Default for Prefs { fn default() -> Self { Prefs { theme: ThemeId::Graphite, white_page: false, ui_scale: 1.0, chip: false, joins: Joins::Exact, fillet: 0.8 } } }

impl Prefs {
    /// The core join style these preferences ask for.
    pub fn join_style(&self) -> scroll_core::model::JoinStyle {
        use scroll_core::model::JoinStyle;
        match self.joins { Joins::Classic => JoinStyle::Classic, Joins::Smooth => JoinStyle::Smooth, Joins::Exact => JoinStyle::Exact(self.fillet as f64) }
    }
}

fn prefs_path() -> Option<PathBuf> { crate::presets::app_dir().map(|d| d.join("settings.txt")) }

impl Prefs {
    pub fn load() -> Prefs {
        let mut p = Prefs::default();
        let Some(text) = prefs_path().and_then(|f| std::fs::read_to_string(f).ok()) else { return p };
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            match k.trim() {
                "theme" => if let Some(t) = ThemeId::from_key(v.trim()) { p.theme = t; },
                "white_page" => p.white_page = v.trim() == "true",
                "workspace" => p.chip = v.trim() == "chip",
                "joins" => if let Some(j) = Joins::from_key(v.trim()) { p.joins = j; },
                "fillet" => if let Ok(r) = v.trim().parse::<f32>() { p.fillet = r.clamp(0.3, 1.5); },
                "ui_scale" => if let Ok(s) = v.trim().parse::<f32>() { p.ui_scale = s.clamp(0.8, 1.5); },
                _ => {}
            }
        }
        p
    }
    pub fn save(&self) {
        let Some(f) = prefs_path() else { return };
        if let Some(dir) = f.parent() { let _ = std::fs::create_dir_all(dir); }
        let _ = std::fs::write(f, format!("theme={}\nwhite_page={}\nui_scale={:.2}\nworkspace={}\njoins={}\nfillet={:.2}\n", self.theme.key(), self.white_page, self.ui_scale, if self.chip { "chip" } else { "scroll" }, self.joins.key(), self.fillet));
    }
}
