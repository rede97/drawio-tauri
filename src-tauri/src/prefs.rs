//! User preferences persisted as JSON at `<data_local_dir>/drawio/prefs.json`.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Prefs {
    #[serde(rename = "googleFonts")]
    pub is_google_fonts_enabled: bool,
    #[serde(rename = "spellCheck")]
    pub enable_spell_check: bool,
    #[serde(rename = "storeBkp")]
    pub store_bkp: bool,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            is_google_fonts_enabled: false,
            enable_spell_check: false,
            // Matches drawio-desktop: backup files are stored by default
            store_bkp: true,
        }
    }
}

/// Shared, managed preference state (`app.manage(...)`).
pub struct PrefsState {
    pub prefs: Mutex<Prefs>,
}

impl PrefsState {
    pub fn new(prefs: Prefs) -> Self {
        Self { prefs: Mutex::new(prefs) }
    }

    /// Toggle a preference field and persist it.
    pub fn toggle(&self, field: PrefField) {
        let mut prefs = self.prefs.lock().unwrap();
        match field {
            PrefField::GoogleFonts => {
                prefs.is_google_fonts_enabled = !prefs.is_google_fonts_enabled
            }
            PrefField::SpellCheck => prefs.enable_spell_check = !prefs.enable_spell_check,
            PrefField::StoreBkp => prefs.store_bkp = !prefs.store_bkp,
        }
        save_prefs(&prefs);
    }
}

pub enum PrefField {
    GoogleFonts,
    SpellCheck,
    StoreBkp,
}

fn prefs_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| Path::new(".").to_path_buf())
        .join("drawio")
        .join("prefs.json")
}

pub fn load_prefs() -> Prefs {
    let path = prefs_path();
    fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_prefs(prefs: &Prefs) {
    let path = prefs_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).ok();
    }
    if let Ok(json) = serde_json::to_string_pretty(prefs) {
        fs::write(&path, json).ok();
    }
}
