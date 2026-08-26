use crate::{paths::AppPaths, persistence::atomic_write};
use egui::{Color32, FontDefinitions, FontFamily, Visuals};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{fs, io, path::PathBuf};

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Theme {
    pub name: String,
    pub primary: ThemeColor,
    pub secondary: ThemeColor,
    pub background: ThemeColor,
    pub font_family: String,
    pub font_size: f32,
}

impl Theme {
    pub fn default_dark() -> Self {
        Self {
            name: "default-dark".to_owned(),
            primary: ThemeColor::rgb(111, 168, 255),
            secondary: ThemeColor::rgb(132, 205, 150),
            background: ThemeColor::rgb(28, 30, 34),
            font_family: "Sans".to_owned(),
            font_size: 16.0,
        }
    }

    pub fn default_light() -> Self {
        Self {
            name: "default-light".to_owned(),
            primary: ThemeColor::rgb(50, 100, 190),
            secondary: ThemeColor::rgb(42, 125, 83),
            background: ThemeColor::rgb(248, 249, 251),
            font_family: "Sans".to_owned(),
            font_size: 16.0,
        }
    }

    pub fn font_family(&self) -> FontFamily {
        match self.font_family.as_str() {
            "Monospace" => FontFamily::Monospace,
            _ => FontFamily::Proportional,
        }
    }
}

/// Configure the two curated families supplied by egui itself. They are embedded
/// in the binary by egui, so Goatpad has no runtime font-file dependency.
pub fn install_fonts(ctx: &egui::Context) {
    ctx.set_fonts(FontDefinitions::default());
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
    visuals.selection.bg_fill = primary.gamma_multiply(0.55);
    visuals.selection.stroke.color = primary;
    visuals.hyperlink_color = primary;
    visuals.widgets.inactive.bg_fill = secondary.gamma_multiply(0.20);
    visuals.widgets.hovered.bg_fill = secondary.gamma_multiply(0.42);
    visuals.widgets.active.bg_fill = primary.gamma_multiply(0.60);
    visuals.widgets.open.bg_fill = secondary.gamma_multiply(0.30);
    ctx.set_visuals(visuals);
    ctx.style_mut_of(egui_theme, |style| {
        for font_id in style.text_styles.values_mut() {
            font_id.family = theme.font_family();
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
    use super::{Theme, ThemeColor, ensure_default_themes, load_themes, save_theme};
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
    fn saved_custom_theme_is_loaded_with_default_presets() {
        let directory = PathBuf::from(std::env::temp_dir())
            .join(format!("goatpad-theme-test-{}", Uuid::new_v4()));
        let paths = AppPaths::for_test(directory).unwrap();
        ensure_default_themes(&paths).unwrap();
        let mut custom = Theme::default_dark();
        custom.name = "My writing theme".to_owned();
        custom.primary = ThemeColor::rgb(10, 20, 30);
        save_theme(&paths, &custom).unwrap();

        let themes = load_themes(&paths).unwrap();
        assert!(themes.iter().any(|theme| theme == &custom));
        assert!(themes.iter().any(|theme| theme.name == "default-dark"));
        assert!(themes.iter().any(|theme| theme.name == "default-light"));
    }
}
