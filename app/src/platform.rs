//! What differs between the desktop program and the web edition: opening and
//! saving files, the store kept between sessions (settings, presets, page sizes,
//! the current chip pattern), the window title and the unsaved-work warning.
//! The desktop side is the code the program always had; the web side is only
//! compiled into the web edition (`target_arch = "wasm32"`).
use eframe::egui;
use std::path::{Path, PathBuf};

/// Who asked for a file to be opened (in the web edition the file arrives a few frames later).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OpenFor { Scroll, Chip, Rococo, Cartouche, Palmette }

/// A file that was picked and read: its path (the file name alone on the web) and text.
pub type Opened = Result<(PathBuf, String), String>;

/// How a written file is named in messages.
pub fn shown(path: &Path) -> String { path.display().to_string() }

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use super::*;

    /// %APPDATA%\ORNATR. The first time it's missing, the files kept under the
    /// program's former name (%APPDATA%\ScrollWorks) are copied across, so
    /// settings, presets and the current chip pattern carry over.
    pub fn app_dir() -> Option<PathBuf> {
        let base = std::env::var_os("APPDATA").or_else(|| std::env::var_os("HOME")).map(PathBuf::from)?;
        let dir = base.join("ORNATR");
        let old = base.join("ScrollWorks");
        if !dir.exists() && old.is_dir() && std::fs::create_dir_all(&dir).is_ok() {
            for entry in std::fs::read_dir(&old).into_iter().flatten().flatten() {
                if entry.path().is_file() { let _ = std::fs::copy(entry.path(), dir.join(entry.file_name())); }
            }
        }
        Some(dir)
    }

    /// A file kept in %APPDATA%\ORNATR.
    pub fn store_read(file: &str) -> Option<String> { app_dir().and_then(|d| std::fs::read_to_string(d.join(file)).ok()) }
    pub fn store_write(file: &str, text: &str) -> Result<(), String> {
        let Some(dir) = app_dir() else { return Err("no settings folder available".into()) };
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(dir.join(file), text).map_err(|e| e.to_string())
    }

    /// Ask where to save; None if cancelled.
    pub fn choose_save(filter: &str, exts: &[&str], name: &str) -> Option<PathBuf> {
        rfd::FileDialog::new().add_filter(filter, exts).set_file_name(name).save_file()
    }
    pub fn write_file(path: &Path, text: &str) -> Result<(), String> { std::fs::write(path, text).map_err(|e| e.to_string()) }

    /// Ask for a file and read it. None if cancelled.
    pub fn pick_text(_ctx: &egui::Context, _who: OpenFor, filter: &str, exts: &[&str]) -> Option<Opened> {
        let path = rfd::FileDialog::new().add_filter(filter, exts).pick_file()?;
        Some(std::fs::read_to_string(&path).map(|t| (path, t)).map_err(|e| e.to_string()))
    }
    /// Files picked earlier that have now been read (the desktop reads them at once).
    pub fn take_opened() -> Option<(OpenFor, Opened)> { None }

    pub fn set_title(ctx: &egui::Context, title: &str) { ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.to_string())); }
    /// The desktop asks through its own Unsaved changes window when it is closed.
    pub fn set_unsaved(_unsaved: bool) {}
    /// The web edition's About and licences link (the desktop ships its licences beside the exe).
    pub fn about_link(_ui: &mut egui::Ui, _color: egui::Color32) {}
}

/// The web edition's side is in web.rs, which only the web build compiles; it is
/// kept with the website build and is not part of the public source.
#[cfg(target_arch = "wasm32")]
#[path = "web.rs"]
mod imp;

pub use imp::*;
