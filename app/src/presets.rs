//! "My presets": named designs kept on this computer, one library for scroll
//! layouts and one for chip layouts, in %APPDATA%\ORNATR (in the web edition,
//! the browser's storage for the page).
use crate::platform;
use serde::{Deserialize, Serialize};

pub const LIMIT: usize = 40;

#[derive(Serialize, Deserialize, Clone)]
pub struct Preset { pub name: String, /// The layout, in its file format.
    pub data: serde_json::Value }

pub struct Library { file: &'static str, pub entries: Vec<Preset>, pub selected: Option<usize>, pub name: String, pub message: String, pub deleted: Option<Preset> }

impl Library {
    pub fn load(file: &'static str) -> Library {
        let entries = platform::store_read(file).and_then(|t| serde_json::from_str::<Vec<Preset>>(&t).ok()).unwrap_or_default();
        Library { file, entries: entries.into_iter().take(LIMIT).collect(), selected: None, name: String::new(), message: String::new(), deleted: None }
    }
    fn store(&mut self) -> bool {
        match platform::store_write(self.file, &serde_json::to_string_pretty(&self.entries).unwrap()) { Ok(()) => true, Err(e) => { self.message = format!("Could not save presets: {e}"); false } }
    }
    pub fn add(&mut self, data: serde_json::Value, fallback: &str) {
        if self.entries.len() >= LIMIT { self.message = format!("The library holds {LIMIT} presets. Remove one first."); return; }
        let name = if self.name.trim().is_empty() { format!("{fallback} {}", self.entries.len() + 1) } else { self.name.trim().chars().take(80).collect() };
        self.entries.push(Preset { name: name.clone(), data });
        if self.store() { self.selected = Some(self.entries.len() - 1); self.name = name; self.message = "Preset saved.".into(); } else { self.entries.pop(); }
    }
    pub fn rename(&mut self) { if let (Some(i), false) = (self.selected, self.name.trim().is_empty()) { self.entries[i].name = self.name.trim().chars().take(80).collect(); self.store(); self.message = "Renamed.".into(); } }
    pub fn duplicate(&mut self) { if let Some(i) = self.selected { let e = self.entries[i].clone(); self.name = format!("{} copy", e.name); self.add(e.data, "Preset"); } }
    pub fn delete(&mut self) { if let Some(i) = self.selected { let e = self.entries.remove(i); if self.store() { self.deleted = Some(e); self.selected = None; self.message = "Preset removed.".into(); } } }
    pub fn undo_delete(&mut self) { if let Some(e) = self.deleted.take() { if self.entries.len() < LIMIT { self.entries.push(e); self.store(); self.message = "Preset restored.".into(); } } }
}
