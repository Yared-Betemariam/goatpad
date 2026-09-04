use crate::{paths::AppPaths, persistence::atomic_write};
#[cfg(target_os = "windows")]
use egui::FontData;
use egui::{Color32, FontDefinitions, FontFamily, Visuals};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::sync::Arc;
use std::{fs, io, path::PathBuf};

pub const FONT_OPTIONS: &[&str] = &[
    "Segoe UI",
    "Sans",
    "Georgia",
    "Cambria",
    "Times New Roman",
    "Arial",
    "Consolas",
    "Monospace",
];

pub const BORDER_COLOR: Color32 = Color32::from_rgba_premultiplied(48, 48, 48, 65);

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
    pub fn title_bar_color(&self) -> Color32 {
        let background = self.background.0.gamma_multiply(0.92);
        let secondary = self.secondary.0;
        Color32::from_rgb(
            blend_channel(background.r(), secondary.r(), 0.02),
            blend_channel(background.g(), secondary.g(), 0.02),
            blend_channel(background.b(), secondary.b(), 0.02),
        )
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

fn blend_channel(background: u8, secondary: u8, amount: f32) -> u8 {
    (f32::from(background) + (f32::from(secondary) - f32::from(background)) * amount).round() as u8
}

/// Installs Windows' included writing fonts while retaining egui's embedded
/// fonts as fallbacks for characters unavailable in the selected face.
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();

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

    #[cfg(target_os = "windows")]
    for (name, file) in [
        ("Segoe UI", "segoeui.ttf"),
        ("Georgia", "georgia.ttf"),
        ("Cambria", "cambria.ttc"),
        ("Times New Roman", "times.ttf"),
        ("Arial", "arial.ttf"),
        ("Consolas", "consola.ttf"),
    ] {
        install_windows_font(&mut fonts, name, file);
    }

    ctx.set_fonts(fonts);
}

#[cfg(target_os = "windows")]
fn install_windows_font(fonts: &mut FontDefinitions, family_name: &str, file_name: &str) {
    let fonts_directory = std::env::var_os("WINDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"))
        .join("Fonts");
    let Ok(data) = fs::read(fonts_directory.join(file_name)) else {
        return;
    };

    let font_name = format!("goatpad-{}", family_name.to_ascii_lowercase());
    fonts
        .font_data
        .insert(font_name.clone(), Arc::new(FontData::from_owned(data)));

    let mut family = vec![font_name];
    if let Some(fallbacks) = fonts.families.get(&FontFamily::Proportional) {
        family.extend(fallbacks.iter().cloned());
    }
    if !family.contains(&"phosphor".to_owned()) {
        family.push("phosphor".to_owned());
    }
    fonts
        .families
        .insert(FontFamily::Name(family_name.into()), family);
}

pub fn apply_theme(ctx: &egui::Context, theme: &Theme) {
    let egui_theme = if theme.background.0.r() < 128 {
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
    visuals.selection.bg_fill = egui::Color32::TRANSPARENT;
    visuals.window_stroke.color = BORDER_COLOR;
    visuals.hyperlink_color = primary;
    visuals.widgets.inactive.bg_fill = secondary.gamma_multiply(0.20);
    visuals.widgets.hovered.bg_fill = secondary.gamma_multiply(0.42);
    visuals.widgets.active.bg_fill = primary.gamma_multiply(0.60);
    visuals.widgets.open.bg_fill = secondary.gamma_multiply(0.30);
    visuals.widgets.noninteractive.bg_stroke.color = BORDER_COLOR;
    visuals.widgets.inactive.bg_stroke.color = BORDER_COLOR;
    visuals.widgets.hovered.bg_stroke.color = BORDER_COLOR;
    visuals.widgets.active.bg_stroke.color = BORDER_COLOR;
    visuals.widgets.open.bg_stroke.color = BORDER_COLOR;
    visuals.selection.stroke.color = BORDER_COLOR;
    ctx.set_visuals(visuals);
    ctx.style_mut_of(egui_theme, |style| {
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
    use super::{Theme, ThemeColor, delete_theme, ensure_default_themes, load_themes, save_theme};
    use crate::paths::AppPaths;
    use std::path::PathBuf;
    use uuid::Uuid;

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
