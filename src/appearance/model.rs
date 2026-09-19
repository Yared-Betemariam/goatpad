use egui::{Color32, FontFamily};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub const DARK_THEME_ID: &str = "default-dark";
pub const LIGHT_THEME_ID: &str = "default-light";
pub const DEFAULT_THEME_ID: &str = DARK_THEME_ID;
pub const DEFAULT_FONT_FAMILY: &str = "Segoe UI";
pub const DEFAULT_FONT_SIZE: f32 = 16.0;
pub const MIN_FONT_SIZE: f32 = 12.0;
pub const MAX_FONT_SIZE: f32 = 24.0;

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
            DEFAULT_FONT_SIZE
        }

        let raw = ThemeRaw::deserialize(deserializer)?;
        let fallback_font = raw
            .font_family
            .or(raw.font)
            .unwrap_or_else(|| DEFAULT_FONT_FAMILY.to_owned());
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
            name: DARK_THEME_ID.to_owned(),
            primary: ThemeColor::rgb(111, 168, 255),
            secondary: ThemeColor::rgb(132, 205, 150),
            background: ThemeColor::rgb(28, 30, 34),
            system_font: DEFAULT_FONT_FAMILY.to_owned(),
            content_font: DEFAULT_FONT_FAMILY.to_owned(),
            font_size: DEFAULT_FONT_SIZE,
        }
    }

    pub fn default_light() -> Self {
        Self {
            name: LIGHT_THEME_ID.to_owned(),
            primary: ThemeColor::rgb(50, 100, 190),
            secondary: ThemeColor::rgb(42, 125, 83),
            background: ThemeColor::rgb(248, 249, 251),
            system_font: DEFAULT_FONT_FAMILY.to_owned(),
            content_font: DEFAULT_FONT_FAMILY.to_owned(),
            font_size: DEFAULT_FONT_SIZE,
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
        self.name == DARK_THEME_ID || self.name == LIGHT_THEME_ID
    }

    pub fn display_name(&self) -> &str {
        match self.name.as_str() {
            DARK_THEME_ID => "Dark (Default)",
            LIGHT_THEME_ID => "Light (Default)",
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

#[cfg(test)]
mod tests {
    use super::{Theme, ThemeColor};

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
}
