//! Draft / backup file conventions, mirroring drawio-desktop:
//!
//! | Convention                    | Meaning                                            |
//! |-------------------------------|----------------------------------------------------|
//! | `.$name.dtmp`, `.$name_N.dtmp`| Autosave drafts beside the original file (hidden)  |
//! | `.$name.bkp`                  | Backup of the last good save, written pre-overwrite|
//! | `~$name.dtmp` / `~$name.bkp`  | Legacy prefixes; ported/cleaned up automatically   |

use std::fs;
use std::path::{Path, PathBuf};

pub const DRAFT_PREFIX: &str = ".$";
pub const OLD_DRAFT_PREFIX: &str = "~$";
pub const DRAFT_EXT: &str = ".dtmp";
pub const BKP_PREFIX: &str = ".$";
pub const OLD_BKP_PREFIX: &str = "~$";

/// Sibling backup path: `<dir>/<prefix><basename>.bkp`
pub fn backup_path(file_path: &str, prefix: &str) -> PathBuf {
    let p = Path::new(file_path);
    let base = p.file_name().unwrap_or_default().to_string_lossy();
    p.with_file_name(format!("{}{}.bkp", prefix, base))
}

/// Write a `.$name.bkp` backup of the current on-disk content before an
/// overwrite. No-op when the target file does not exist yet (new file).
pub fn create_backup(file_path: &str) {
    if let Ok(old_content) = fs::read(file_path) {
        fs::write(backup_path(file_path, BKP_PREFIX), &old_content).ok();
    }
}

/// Remove the legacy `~$name.bkp` backup after a successful save.
pub fn cleanup_legacy_backup(file_path: &str) {
    let old_bkp = backup_path(file_path, OLD_BKP_PREFIX);
    if old_bkp.exists() {
        fs::remove_file(&old_bkp).ok();
    }
}

/// Read the best available backup for `file_path`: new prefix first, then the
/// legacy one. Returns `(data, created_ms, modified_ms, path)`.
pub fn read_backup(file_path: &str) -> Option<(String, u64, u64, PathBuf)> {
    for prefix in [BKP_PREFIX, OLD_BKP_PREFIX] {
        let bkp = backup_path(file_path, prefix);
        if let Ok(meta) = fs::metadata(&bkp) {
            if meta.is_file() {
                if let Ok(data) = fs::read_to_string(&bkp) {
                    let to_ms = |t: std::io::Result<std::time::SystemTime>| {
                        t.ok()
                            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                            .map(|d| d.as_millis() as u64)
                            .unwrap_or(0)
                    };
                    return Some((data, to_ms(meta.created()), to_ms(meta.modified()), bkp));
                }
            }
        }
    }
    None
}

/// First free sibling draft path: `<dir>/.$<basename>.dtmp`, then `_1`, `_2`, …
pub fn next_draft_path(file_path: &str) -> PathBuf {
    let p = Path::new(file_path);
    let base = p.file_name().unwrap_or_default().to_string_lossy();
    let mut counter: u32 = 0;
    loop {
        let unique = if counter == 0 { String::new() } else { format!("_{}", counter) };
        let candidate = p.with_file_name(format!("{}{}{}{}", DRAFT_PREFIX, base, unique, DRAFT_EXT));
        if !candidate.exists() {
            return candidate;
        }
        counter += 1;
    }
}

/// Write a draft for `file_path`, reusing the webapp-provided draft name when
/// it is a sibling of the original file. Returns the draft path used.
pub fn write_draft(file_path: &str, preferred: Option<&str>, data: &str) -> std::io::Result<PathBuf> {
    let draft_path = preferred
        .map(PathBuf::from)
        .filter(|p| p.parent() == Path::new(file_path).parent())
        .unwrap_or_else(|| next_draft_path(file_path));

    fs::write(&draft_path, data)?;

    // Mark the draft hidden on Windows (same as drawio-desktop)
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("attrib")
            .args(["+h", &draft_path.to_string_lossy()])
            .spawn()
            .ok();
    }

    Ok(draft_path)
}

/// Port legacy `~$`-prefixed drafts to the new `.$` prefix, then return all
/// sibling draft paths for `file_path`, sorted.
pub fn collect_drafts(file_path: &str) -> Vec<PathBuf> {
    for legacy in collect_prefixed_files(file_path, OLD_DRAFT_PREFIX, DRAFT_EXT) {
        let file_name = legacy.file_name().unwrap_or_default().to_string_lossy();
        let target = legacy.with_file_name(format!(".{}", file_name));
        if !target.exists() {
            fs::rename(&legacy, &target).ok();
        }
    }
    collect_prefixed_files(file_path, DRAFT_PREFIX, DRAFT_EXT)
}

/// All sibling files of `file_path` named `<prefix><basename>*<ext>`, sorted.
fn collect_prefixed_files(file_path: &str, prefix: &str, ext: &str) -> Vec<PathBuf> {
    let p = Path::new(file_path);
    let base = p.file_name().unwrap_or_default().to_string_lossy().to_string();
    let dir = p.parent().map(|d| d.to_path_buf()).unwrap_or_default();
    let mut out = Vec::new();
    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(&format!("{}{}", prefix, base)) && name.ends_with(ext) {
                out.push(entry.path());
            }
        }
    }
    out.sort();
    out
}
