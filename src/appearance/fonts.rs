use egui::{FontData, FontDefinitions, FontFamily};
use std::sync::Arc;

#[cfg(target_os = "windows")]
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

#[cfg(target_os = "windows")]
use windows_sys::Win32::Foundation::{ERROR_MORE_DATA, ERROR_NO_MORE_ITEMS};

#[cfg(target_os = "windows")]
use windows_sys::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, REG_EXPAND_SZ, REG_SZ, RegCloseKey,
    RegEnumValueW, RegOpenKeyExW,
};

pub const FONT_OPTIONS: &[&str] = &[
    "Segoe UI",
    "Sans",
    "Georgia",
    "Cambria",
    "Times New Roman",
    "Arial",
    "Consolas",
    "Monospace",
    "Calibri",
    "Candara",
    "Constantia",
    "Corbel",
    "Courier New",
    "Lucida Console",
    "Microsoft Sans Serif",
    "Palatino Linotype",
    "Tahoma",
    "Trebuchet MS",
    "Verdana",
    "Yu Gothic",
    "Comic Sans MS",
    "Franklin Gothic Medium",
    "Garamond",
    "Book Antiqua",
    "Century Gothic",
    "Century Schoolbook",
    "Impact",
    "Bahnschrift",
    "Cascadia Code",
    "Cascadia Mono",
    "Fira Code",
    "Fira Sans",
    "IBM Plex Mono",
    "IBM Plex Sans",
    "Inter",
    "JetBrains Mono",
    "Lato",
    "Open Sans",
    "Raleway",
    "Roboto",
    "Roboto Mono",
    "Source Code Pro",
    "Ubuntu",
    "Noto Sans",
    "Noto Sans Mono",
    "Monaco",
    "Merriweather",
    "PT Sans",
    "Rubik",
    "Work Sans",
];

const GENERIC_FONT_OPTIONS: &[&str] = &["Sans", "Monospace"];

// This font is bundled so Ethiopic text does not depend on the user's selected
// content font or on an optional Windows font installation.
const AMHARIC_FONT_NAME: &str = "goatpad-amharic";
const AMHARIC_FONT_DATA: &[u8] = include_bytes!("../../assets/AbyssinicaSIL-Regular.ttf");

#[cfg(target_os = "windows")]
const WINDOWS_AMHARIC_FONT_NAME: &str = "goatpad-amharic-windows";

#[cfg(target_os = "windows")]
const WINDOWS_AMHARIC_FONT_FILE: &str = "ebrima.ttf";

#[cfg(target_os = "windows")]
const WINDOWS_FONT_REGISTRY_PATH: &str = "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Fonts";

#[cfg(target_os = "windows")]
const WINDOWS_FONT_FILE_FALLBACKS: &[(&str, &str)] = &[
    ("Segoe UI", "segoeui.ttf"),
    ("Georgia", "georgia.ttf"),
    ("Cambria", "cambria.ttc"),
    ("Times New Roman", "times.ttf"),
    ("Arial", "arial.ttf"),
    ("Consolas", "consola.ttf"),
];

#[cfg(target_os = "windows")]
const FONT_STYLE_WORDS: &[&str] = &[
    "black",
    "bold",
    "book",
    "condensed",
    "demi",
    "demibold",
    "display",
    "extra",
    "extrabold",
    "extralight",
    "font",
    "heavy",
    "italic",
    "light",
    "medium",
    "normal",
    "nl",
    "narrow",
    "oblique",
    "propo",
    "regular",
    "roman",
    "semicondensed",
    "semilight",
    "semibold",
    "semi",
    "text",
    "thin",
    "ui",
    "variable",
];

#[cfg(target_os = "windows")]
const FONT_UNWANTED_STYLE_WORDS: &[&str] = &[
    "black",
    "bold",
    "demibold",
    "extrabold",
    "heavy",
    "italic",
    "oblique",
    "semibold",
];

#[cfg(target_os = "windows")]
#[derive(Debug)]
struct InstalledFont {
    family_name: &'static str,
    path: PathBuf,
    source_priority: u8,
    match_score: u8,
}

/// Installs the supported fonts that are present on this machine while
/// retaining egui's embedded fonts as fallbacks for missing characters.
///
/// The returned list is the same list that can safely be offered by the
/// settings UI. This keeps a font that is merely named in the catalog from
/// being selectable when its file is not installed.
pub fn install_fonts(ctx: &egui::Context) -> Vec<String> {
    let mut fonts = FontDefinitions::default();

    install_amharic_fallback(&mut fonts);

    fonts.font_data.insert(
        "phosphor".to_owned(),
        Arc::new(FontData::from_static(
            egui_phosphor::Variant::Regular.font_bytes(),
        )),
    );
    if let Some(family) = fonts.families.get_mut(&FontFamily::Proportional) {
        family.push("phosphor".to_owned());
    }
    if let Some(family) = fonts.families.get_mut(&FontFamily::Monospace) {
        family.push("phosphor".to_owned());
    }

    let mut available_fonts = Vec::new();

    #[cfg(target_os = "windows")]
    let installed_fonts = discover_windows_fonts();

    for &font_name in FONT_OPTIONS {
        if GENERIC_FONT_OPTIONS.contains(&font_name) {
            available_fonts.push(font_name.to_owned());
            continue;
        }

        #[cfg(target_os = "windows")]
        if let Some(font) = installed_fonts
            .iter()
            .find(|font| font.family_name == font_name)
            && install_windows_font(&mut fonts, font.family_name, &font.path)
        {
            available_fonts.push(font_name.to_owned());
        }
    }

    ctx.set_fonts(fonts);
    available_fonts
}

fn install_amharic_fallback(fonts: &mut FontDefinitions) {
    fonts.font_data.insert(
        AMHARIC_FONT_NAME.to_owned(),
        Arc::new(FontData::from_static(AMHARIC_FONT_DATA)),
    );

    let mut fallback_fonts = vec![AMHARIC_FONT_NAME.to_owned()];

    // Ebrima's regular face is a little lighter than the bundled fallback and
    // is included with Windows. Use it when available without redistributing
    // Microsoft's font file with Goatpad.
    #[cfg(target_os = "windows")]
    if let Some(path) = resolve_font_path(WINDOWS_AMHARIC_FONT_FILE) {
        if let Ok(data) = fs::read(path) {
            fonts.font_data.insert(
                WINDOWS_AMHARIC_FONT_NAME.to_owned(),
                Arc::new(FontData::from_owned(data)),
            );
            fallback_fonts.insert(0, WINDOWS_AMHARIC_FONT_NAME.to_owned());
        }
    }

    for family_name in [FontFamily::Proportional, FontFamily::Monospace] {
        if let Some(family) = fonts.families.get_mut(&family_name) {
            for fallback_font in &fallback_fonts {
                if !family.iter().any(|font| font == fallback_font) {
                    family.push(fallback_font.clone());
                }
            }
        }
    }
}

#[cfg(target_os = "windows")]
fn discover_windows_fonts() -> Vec<InstalledFont> {
    let mut discovered = HashMap::new();

    // Per-user registrations take precedence over machine-wide registrations.
    collect_registry_fonts(HKEY_CURRENT_USER, 0, &mut discovered);
    collect_registry_fonts(HKEY_LOCAL_MACHINE, 1, &mut discovered);

    // Keep the bundled Windows fonts discoverable even if a registry entry is
    // temporarily unavailable while Windows is refreshing its font cache.
    for &(family_name, file_name) in WINDOWS_FONT_FILE_FALLBACKS {
        if discovered.contains_key(&normalize_font_name(family_name)) {
            continue;
        }
        if let Some(path) = resolve_font_path(file_name) {
            discovered.insert(
                normalize_font_name(family_name),
                InstalledFont {
                    family_name,
                    path,
                    source_priority: 1,
                    match_score: 2,
                },
            );
        }
    }

    discovered.into_values().collect()
}

#[cfg(target_os = "windows")]
fn collect_registry_fonts(
    root: HKEY,
    source_priority: u8,
    discovered: &mut HashMap<String, InstalledFont>,
) {
    let registry_path = to_wide(WINDOWS_FONT_REGISTRY_PATH);
    let mut key = std::ptr::null_mut();
    let result = unsafe { RegOpenKeyExW(root, registry_path.as_ptr(), 0, KEY_READ, &mut key) };
    if result != 0 {
        return;
    }

    let mut index = 0;
    loop {
        let mut value_name = vec![0u16; 512];
        let mut value_name_length = (value_name.len() - 1) as u32;
        let mut value_data = vec![0u8; 32 * 1024];
        let mut value_data_length = value_data.len() as u32;
        let mut value_type = 0;
        let result = unsafe {
            RegEnumValueW(
                key,
                index,
                value_name.as_mut_ptr(),
                &mut value_name_length,
                std::ptr::null(),
                &mut value_type,
                value_data.as_mut_ptr(),
                &mut value_data_length,
            )
        };
        index += 1;

        if result == ERROR_NO_MORE_ITEMS {
            break;
        }
        if result == ERROR_MORE_DATA {
            continue;
        }
        if result != 0 {
            // Font registry entries are small, so an oversized or malformed
            // value can be skipped without preventing the other entries from
            // being discovered.
            continue;
        }

        let registry_name = String::from_utf16_lossy(&value_name[..value_name_length as usize]);
        let Some((family_name, match_score)) = supported_family(&registry_name) else {
            continue;
        };
        if value_type != REG_SZ && value_type != REG_EXPAND_SZ {
            continue;
        }
        let Some(file_name) = decode_registry_string(&value_data[..value_data_length as usize])
        else {
            continue;
        };
        let Some(path) = resolve_font_path(&file_name) else {
            continue;
        };

        let key_name = normalize_font_name(family_name);
        let replace = discovered.get(&key_name).is_none_or(|existing| {
            source_priority < existing.source_priority
                || (source_priority == existing.source_priority
                    && match_score > existing.match_score)
        });
        if replace {
            discovered.insert(
                key_name,
                InstalledFont {
                    family_name,
                    path,
                    source_priority,
                    match_score,
                },
            );
        }
    }

    unsafe {
        RegCloseKey(key);
    }
}

#[cfg(target_os = "windows")]
fn supported_family(registry_name: &str) -> Option<(&'static str, u8)> {
    let registered_name = normalize_font_name(strip_registry_suffix(registry_name));

    // Prefer a true family-name registry value over a style-specific alias.
    for &candidate in FONT_OPTIONS {
        if GENERIC_FONT_OPTIONS.contains(&candidate) {
            continue;
        }
        if registered_name == normalize_font_name(candidate) {
            return Some((candidate, 3));
        }
    }

    for &candidate in FONT_OPTIONS {
        if GENERIC_FONT_OPTIONS.contains(&candidate) {
            continue;
        }
        let candidate_name = normalize_font_name(candidate);
        let Some(style_suffix) = registered_name.strip_prefix(&format!("{candidate_name} ")) else {
            continue;
        };
        let style_words = style_suffix.split_whitespace().collect::<Vec<_>>();
        if style_words.is_empty()
            || !style_words
                .iter()
                .all(|word| FONT_STYLE_WORDS.contains(word))
            || style_words
                .iter()
                .any(|word| FONT_UNWANTED_STYLE_WORDS.contains(word))
        {
            continue;
        }

        let score = if style_words
            .iter()
            .any(|word| ["normal", "regular", "variable"].contains(word))
        {
            3
        } else if style_words
            .iter()
            .any(|word| ["book", "medium"].contains(word))
        {
            2
        } else {
            1
        };
        return Some((candidate, score));
    }

    None
}

#[cfg(target_os = "windows")]
fn strip_registry_suffix(value: &str) -> &str {
    let family = [" (TrueType)", " (OpenType)", " (Type 1)"]
        .iter()
        .find_map(|suffix| value.strip_suffix(suffix))
        .unwrap_or(value)
        .trim()
        .split(" & ")
        .next()
        .unwrap_or(value)
        .trim();
    family
}

#[cfg(target_os = "windows")]
fn normalize_font_name(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

#[cfg(target_os = "windows")]
fn decode_registry_string(data: &[u8]) -> Option<String> {
    let utf16 = data
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .take_while(|character| *character != 0)
        .collect::<Vec<_>>();
    let value = String::from_utf16(&utf16).ok()?.trim().to_owned();
    (!value.is_empty()).then_some(value)
}

#[cfg(target_os = "windows")]
fn resolve_font_path(value: &str) -> Option<PathBuf> {
    let expanded = expand_windows_variables(value);
    let path = PathBuf::from(&expanded);
    if path.is_absolute() && path.is_file() {
        return Some(path);
    }

    windows_font_directories()
        .into_iter()
        .map(|directory| directory.join(&path))
        .find(|candidate| candidate.is_file())
}

#[cfg(target_os = "windows")]
fn windows_font_directories() -> Vec<PathBuf> {
    let mut directories = Vec::new();
    if let Some(windir) = std::env::var_os("WINDIR") {
        directories.push(PathBuf::from(windir).join("Fonts"));
    } else {
        directories.push(PathBuf::from(r"C:\Windows\Fonts"));
    }
    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        directories.push(
            PathBuf::from(local_app_data)
                .join("Microsoft")
                .join("Windows")
                .join("Fonts"),
        );
    }
    directories
}

#[cfg(target_os = "windows")]
fn expand_windows_variables(value: &str) -> String {
    let mut expanded = value.to_owned();
    for variable in ["WINDIR", "LOCALAPPDATA", "USERPROFILE"] {
        let Some(replacement) = std::env::var(variable).ok() else {
            continue;
        };
        let token = format!("%{variable}%");
        expanded = expanded.replace(&token, &replacement);
        expanded = expanded.replace(&token.to_ascii_lowercase(), &replacement);
    }
    expanded
}

#[cfg(target_os = "windows")]
fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(target_os = "windows")]
fn install_windows_font(fonts: &mut FontDefinitions, family_name: &str, path: &Path) -> bool {
    let Ok(data) = fs::read(path) else {
        return false;
    };

    let font_name = format!(
        "goatpad-font-{}",
        normalize_font_name(family_name).replace(' ', "-")
    );
    fonts
        .font_data
        .insert(font_name.clone(), Arc::new(FontData::from_owned(data)));

    let mut family = vec![font_name];
    if let Some(fallbacks) = fonts.families.get(&FontFamily::Proportional).cloned() {
        family.extend(fallbacks);
    }
    if !family.contains(&"phosphor".to_owned()) {
        family.push("phosphor".to_owned());
    }
    fonts
        .families
        .insert(FontFamily::Name(family_name.into()), family);

    true
}

#[cfg(test)]
mod tests {
    use super::{AMHARIC_FONT_NAME, FONT_OPTIONS, install_amharic_fallback};
    use egui::{FontDefinitions, FontFamily};

    #[test]
    fn font_catalog_has_at_least_twenty_choices() {
        assert!(FONT_OPTIONS.len() >= 20);
    }

    #[test]
    fn amharic_font_is_added_to_builtin_fallback_chains() {
        let mut fonts = FontDefinitions::default();
        install_amharic_fallback(&mut fonts);

        assert!(fonts.font_data.contains_key(AMHARIC_FONT_NAME));
        for family_name in [FontFamily::Proportional, FontFamily::Monospace] {
            assert!(
                fonts
                    .families
                    .get(&family_name)
                    .is_some_and(|family| family.iter().any(|font| font == AMHARIC_FONT_NAME))
            );
        }
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn registry_font_names_match_supported_families() {
        assert_eq!(
            super::supported_family("Inter Regular (TrueType)"),
            Some(("Inter", 3))
        );
        assert!(super::supported_family("Inter Italic (TrueType)").is_none());
        assert!(super::supported_family("Inter Bold (TrueType)").is_none());
        assert_eq!(
            super::supported_family("JetBrains Mono (OpenType)"),
            Some(("JetBrains Mono", 3))
        );
        assert_eq!(
            super::supported_family("JetBrains Mono Medium (OpenType)"),
            Some(("JetBrains Mono", 2))
        );
        assert_eq!(
            super::supported_family(
                "Yu Gothic Regular & Yu Gothic UI Semilight & Yu Gothic UI Bold (TrueType)"
            ),
            Some(("Yu Gothic", 3))
        );
        assert!(super::supported_family("Not A Supported Font (TrueType)").is_none());
    }
}
