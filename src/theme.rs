use crate::{paths::AppPaths, persistence::atomic_write};
use egui::FontData;
use egui::{Color32, FontDefinitions, FontFamily, Visuals};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::sync::Arc;
use std::{
    fs, io,
    path::{Path, PathBuf},
};

#[cfg(target_os = "windows")]
use std::collections::HashMap;

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
const AMHARIC_FONT_DATA: &[u8] = include_bytes!("../assets/AbyssinicaSIL-Regular.ttf");

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeColor(pub Color32);

impl ThemeColor {
    pub const fn rgb(red: u8, green: u8, blue: u8) -> Self {
        Self(Color32::from_rgb(red, green, blue))
    }
}

impl Serialize for ThemeColor {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&format!(
            "#{:02X}{:02X}{:02X}",
            self.0.r(),
            self.0.g(),
            self.0.b()
        ))
    }
}

impl<'de> Deserialize<'de> for ThemeColor {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        let hex = value.strip_prefix('#').unwrap_or(&value);
        if hex.len() != 6 {
            return Err(serde::de::Error::custom(
                "colour must be a six-digit hex value",
            ));
        }
        let component =
            |range| u8::from_str_radix(&hex[range], 16).map_err(serde::de::Error::custom);
        Ok(Self(Color32::from_rgb(
            component(0..2)?,
            component(2..4)?,
            component(4..6)?,
        )))
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Theme {
    pub name: String,
    pub primary: ThemeColor,
    pub secondary: ThemeColor,
    pub background: ThemeColor,
    pub system_font: String,
    pub content_font: String,
    pub font_size: f32,
}

impl<'de> Deserialize<'de> for Theme {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct ThemeRaw {
            name: String,
            primary: ThemeColor,
            secondary: ThemeColor,
            background: ThemeColor,
            #[serde(default)]
            system_font: Option<String>,
            #[serde(default)]
            content_font: Option<String>,
            #[serde(default)]
            font_family: Option<String>,
            #[serde(default)]
            font: Option<String>,
            #[serde(default = "default_font_size")]
            font_size: f32,
        }

        fn default_font_size() -> f32 {
            16.0
        }

        let raw = ThemeRaw::deserialize(deserializer)?;
        let fallback_font = raw
            .font_family
            .or(raw.font)
            .unwrap_or_else(|| "Segoe UI".to_owned());
        let system_font = raw.system_font.unwrap_or_else(|| fallback_font.clone());
        let content_font = raw.content_font.unwrap_or(fallback_font);

        Ok(Self {
            name: raw.name,
            primary: raw.primary,
            secondary: raw.secondary,
            background: raw.background,
            system_font,
            content_font,
            font_size: raw.font_size,
        })
    }
}

// Color shaded title bar
impl Theme {
    pub fn is_dark(&self) -> bool {
        self.background.0.r() < 128
    }

    pub fn title_bar_color(&self) -> Color32 {
        let background =
            shade_toward_contrast(self.background.0, if self.is_dark() { 0.04 } else { 0.06 });
        let secondary = self.secondary.0;
        Color32::from_rgb(
            blend_channel(background.r(), secondary.r(), 0.04),
            blend_channel(background.g(), secondary.g(), 0.04),
            blend_channel(background.b(), secondary.b(), 0.04),
        )
    }

    pub fn footer_color(&self) -> Color32 {
        shade_toward_contrast(self.background.0, if self.is_dark() { 0.04 } else { 0.02 })
    }

    pub fn border_color(&self) -> Color32 {
        if self.is_dark() {
            Color32::from_rgba_premultiplied(24, 24, 24, 100)
        } else {
            Color32::from_rgba_premultiplied(32, 32, 32, 65)
        }
    }

    pub fn default_dark() -> Self {
        Self {
            name: "default-dark".to_owned(),
            primary: ThemeColor::rgb(111, 168, 255),
            secondary: ThemeColor::rgb(132, 205, 150),
            background: ThemeColor::rgb(28, 30, 34),
            system_font: "Segoe UI".to_owned(),
            content_font: "Segoe UI".to_owned(),
            font_size: 16.0,
        }
    }

    pub fn default_light() -> Self {
        Self {
            name: "default-light".to_owned(),
            primary: ThemeColor::rgb(50, 100, 190),
            secondary: ThemeColor::rgb(42, 125, 83),
            background: ThemeColor::rgb(248, 249, 251),
            system_font: "Segoe UI".to_owned(),
            content_font: "Segoe UI".to_owned(),
            font_size: 16.0,
        }
    }

    pub fn system_font_family(&self) -> FontFamily {
        Self::resolve_font_family(&self.system_font)
    }

    pub fn content_font_family(&self) -> FontFamily {
        Self::resolve_font_family(&self.content_font)
    }

    pub fn resolve_font_family(font_name: &str) -> FontFamily {
        match font_name {
            "Monospace" => FontFamily::Monospace,
            "Sans" => FontFamily::Proportional,
            name => FontFamily::Name(name.into()),
        }
    }

    pub fn is_builtin(&self) -> bool {
        self.name == "default-dark" || self.name == "default-light"
    }

    pub fn display_name(&self) -> &str {
        match self.name.as_str() {
            "default-dark" => "Dark (Default)",
            "default-light" => "Light (Default)",
            custom => custom,
        }
    }
}

fn blend_channel(background: u8, target: u8, amount: f32) -> u8 {
    (f32::from(background) + (f32::from(target) - f32::from(background)) * amount).round() as u8
}

fn shade_toward_contrast(background: Color32, amount: f32) -> Color32 {
    let target = if background.r() < 128 { 255 } else { 0 };
    Color32::from_rgb(
        blend_channel(background.r(), target, amount),
        blend_channel(background.g(), target, amount),
        blend_channel(background.b(), target, amount),
    )
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

pub fn apply_theme(ctx: &egui::Context, theme: &Theme) {
    let egui_theme = if theme.is_dark() {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    };
    ctx.set_theme(egui_theme);
    let mut visuals = if egui_theme == egui::Theme::Dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };
    let primary = theme.primary.0;
    let secondary = theme.secondary.0;
    let background = theme.background.0;
    visuals.panel_fill = background;
    visuals.window_fill = background;
    visuals.extreme_bg_color = background;
    visuals.faint_bg_color = secondary.gamma_multiply(0.13);
    visuals.code_bg_color = secondary.gamma_multiply(0.20);
    // visuals.selection.bg_fill = primary.gamma_multiply(0.50);
    visuals.window_stroke.color = theme.border_color();
    visuals.hyperlink_color = primary;
    visuals.widgets.inactive.bg_fill = secondary.gamma_multiply(0.20);
    visuals.widgets.hovered.bg_fill = secondary.gamma_multiply(0.42);
    visuals.widgets.active.bg_fill = primary.gamma_multiply(0.60);
    visuals.widgets.open.bg_fill = secondary.gamma_multiply(0.30);
    visuals.widgets.noninteractive.bg_stroke.color = theme.border_color();
    visuals.widgets.inactive.bg_stroke.color = theme.border_color();
    visuals.widgets.hovered.bg_stroke.color = theme.border_color();
    visuals.widgets.active.bg_stroke.color = theme.border_color();
    visuals.widgets.open.bg_stroke.color = theme.border_color();
    ctx.set_visuals(visuals);
    ctx.style_mut_of(egui_theme, |style| {
        style.spacing.scroll.dormant_handle_opacity = 0.2;
        style.spacing.scroll.active_handle_opacity = 0.2;
        style.spacing.scroll.interact_handle_opacity = 0.2;
        for font_id in style.text_styles.values_mut() {
            font_id.family = theme.system_font_family();
            font_id.size = theme.font_size;
        }
    });
}

pub fn ensure_default_themes(paths: &AppPaths) -> io::Result<()> {
    for theme in [Theme::default_dark(), Theme::default_light()] {
        let path = theme_path(paths, &theme.name);
        if !path.exists() {
            save_theme(paths, &theme)?;
        }
    }
    Ok(())
}

pub fn load_themes(paths: &AppPaths) -> io::Result<Vec<Theme>> {
    let mut themes = fs::read_dir(paths.themes_dir())?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .filter_map(|path| fs::read(path).ok())
        .filter_map(|contents| serde_json::from_slice::<Theme>(&contents).ok())
        .collect::<Vec<_>>();
    themes.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(themes)
}

pub fn save_theme(paths: &AppPaths, theme: &Theme) -> io::Result<()> {
    let data = serde_json::to_vec_pretty(theme)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    atomic_write(&theme_path(paths, &theme.name), &data)
}

pub fn delete_theme(paths: &AppPaths, name: &str) -> io::Result<bool> {
    if name == "default-dark" || name == "default-light" {
        return Ok(false);
    }
    let path = theme_path(paths, name);
    if path.exists() {
        fs::remove_file(path)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

fn theme_path(paths: &AppPaths, name: &str) -> PathBuf {
    let slug = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_owned();
    paths.themes_dir().join(format!(
        "{}.json",
        if slug.is_empty() { "custom" } else { &slug }
    ))
}

#[cfg(test)]
mod tests {
    use super::{
        AMHARIC_FONT_NAME, FONT_OPTIONS, Theme, ThemeColor, delete_theme, ensure_default_themes,
        install_amharic_fallback, load_themes, save_theme,
    };
    use crate::paths::AppPaths;
    use egui::{FontDefinitions, FontFamily};
    use std::path::PathBuf;
    use uuid::Uuid;

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

    #[test]
    fn theme_colour_round_trips_as_hex() {
        let colour = ThemeColor::rgb(1, 171, 255);
        assert_eq!(serde_json::to_string(&colour).unwrap(), "\"#01ABFF\"");
        assert_eq!(
            serde_json::from_str::<ThemeColor>("\"#01ABFF\"").unwrap(),
            colour
        );
    }

    #[test]
    fn legacy_theme_deserialization_migrates_font_family_to_both_fonts() {
        let legacy_json = r##"{
            "name": "Legacy Theme",
            "primary": "#112233",
            "secondary": "#445566",
            "background": "#778899",
            "font_family": "Georgia",
            "font_size": 18.0
        }"##;
        let theme: Theme = serde_json::from_str(legacy_json).unwrap();
        assert_eq!(theme.system_font, "Georgia");
        assert_eq!(theme.content_font, "Georgia");
        assert_eq!(theme.font_size, 18.0);
    }

    #[test]
    fn saved_custom_theme_is_loaded_with_default_presets() {
        let directory = PathBuf::from(std::env::temp_dir())
            .join(format!("goatpad-theme-test-{}", Uuid::new_v4()));
        let paths = AppPaths::for_test(directory).unwrap();
        ensure_default_themes(&paths).unwrap();
        let mut custom = Theme::default_dark();
        custom.name = "My writing theme".to_owned();
        custom.primary = ThemeColor::rgb(10, 20, 30);
        custom.system_font = "Segoe UI".to_owned();
        custom.content_font = "Georgia".to_owned();
        save_theme(&paths, &custom).unwrap();

        let themes = load_themes(&paths).unwrap();
        assert!(themes.iter().any(|theme| theme == &custom));
        assert!(themes.iter().any(|theme| theme.name == "default-dark"));
        assert!(themes.iter().any(|theme| theme.name == "default-light"));

        // Test delete_theme
        assert!(delete_theme(&paths, "My writing theme").unwrap());
        let themes_after = load_themes(&paths).unwrap();
        assert!(
            !themes_after
                .iter()
                .any(|theme| theme.name == "My writing theme")
        );

        // Default themes cannot be deleted
        assert!(!delete_theme(&paths, "default-dark").unwrap());
    }
}
