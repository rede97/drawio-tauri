//! System font enumeration for the webapp's local-fonts autocomplete.

/// Enumerate installed system font family names.
///
/// - Windows: registry query (`HKLM\...\Fonts`)
/// - Linux: `fc-list`
/// - macOS: `fc-list`, falling back to scanning the font directories
pub fn get_local_fonts() -> Vec<String> {
    #[cfg(target_os = "windows")]
    {
        if let Ok(out) = std::process::Command::new("reg")
            .args(["query", r"HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Fonts"])
            .output()
        {
            let text = String::from_utf8_lossy(&out.stdout);
            let mut fonts: Vec<String> = text.lines()
                .filter_map(|line| {
                    if !line.contains("REG_SZ") { return None; }
                    let name = line.split("REG_SZ").next()?.trim().to_string();
                    if name.is_empty() || name.starts_with("HKEY_") { return None; }
                    Some(name
                        .replace(" (TrueType)", "")
                        .replace(" (OpenType)", "")
                        .trim()
                        .to_string())
                })
                .filter(|s| !s.is_empty())
                .collect();
            fonts.sort();
            fonts.dedup();
            return fonts;
        }
        Vec::new()
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(out) = std::process::Command::new("fc-list")
            .args([":", "family"])
            .output()
        {
            if out.status.success() {
                let text = String::from_utf8_lossy(&out.stdout);
                let mut fonts: Vec<String> = text.lines()
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                fonts.sort();
                fonts.dedup();
                return fonts;
            }
        }

        #[cfg(target_os = "macos")]
        {
            let mut fonts = Vec::new();
            for dir in &["/System/Library/Fonts", "/Library/Fonts"] {
                if let Ok(entries) = std::fs::read_dir(dir) {
                    for entry in entries.flatten() {
                        let name = entry.file_name().to_string_lossy().to_string();
                        if let Some(dot) = name.rfind('.') {
                            fonts.push(name[..dot].to_string());
                        }
                    }
                }
            }
            fonts.sort();
            fonts.dedup();
            fonts
        }

        #[cfg(not(target_os = "macos"))]
        {
            Vec::new()
        }
    }
}
