use egui::{Key, Modifiers};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{collections::HashMap, fmt, str::FromStr};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Action {
    ToggleBold,
    ToggleItalic,
    ToggleUnderline,
    ToggleBulletList,
    NewTab,
    DeleteTab,
    OpenSettings,
}

impl Action {
    pub const ALL: [Self; 7] = [
        Self::ToggleBold,
        Self::ToggleItalic,
        Self::ToggleUnderline,
        Self::ToggleBulletList,
        Self::NewTab,
        Self::DeleteTab,
        Self::OpenSettings,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::ToggleBold => "Bold",
            Self::ToggleItalic => "Italic",
            Self::ToggleUnderline => "Underline",
            Self::ToggleBulletList => "Bullet list",
            Self::NewTab => "New tab",
            Self::DeleteTab => "Delete tab",
            Self::OpenSettings => "Settings",
        }
    }

    pub const fn is_formatting(self) -> bool {
        matches!(
            self,
            Self::ToggleBold | Self::ToggleItalic | Self::ToggleUnderline | Self::ToggleBulletList
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Keybinding {
    pub key: Key,
    pub modifiers: Modifiers,
}

impl Keybinding {
    pub const fn new(key: Key, modifiers: Modifiers) -> Self {
        Self { key, modifiers }
    }
}

impl fmt::Display for Keybinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts = Vec::new();
        if self.modifiers.ctrl {
            parts.push("Ctrl");
        }
        if self.modifiers.shift {
            parts.push("Shift");
        }
        if self.modifiers.alt {
            parts.push("Alt");
        }
        if self.modifiers.mac_cmd || self.modifiers.command {
            parts.push("Cmd");
        }
        parts.push(key_name(self.key));
        f.write_str(&parts.join("+"))
    }
}

impl FromStr for Keybinding {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let mut modifiers = Modifiers::NONE;
        let mut key = None;
        for part in value.split('+') {
            match part.trim().to_ascii_lowercase().as_str() {
                "ctrl" | "control" => modifiers.ctrl = true,
                "shift" => modifiers.shift = true,
                "alt" => modifiers.alt = true,
                "cmd" | "command" => modifiers.command = true,
                name => key = Some(parse_key(name).ok_or_else(|| format!("unknown key: {part}"))?),
            }
        }
        key.map(|key| Self { key, modifiers })
            .ok_or_else(|| "a key is required".to_owned())
    }
}

impl Serialize for Keybinding {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Keybinding {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

pub fn default_bindings() -> HashMap<Action, Keybinding> {
    [
        (Action::ToggleBold, Keybinding::new(Key::B, Modifiers::CTRL)),
        (
            Action::ToggleItalic,
            Keybinding::new(Key::I, Modifiers::CTRL),
        ),
        (
            Action::ToggleUnderline,
            Keybinding::new(Key::U, Modifiers::CTRL),
        ),
        (
            Action::ToggleBulletList,
            Keybinding::new(
                Key::Num8,
                Modifiers {
                    ctrl: true,
                    shift: true,
                    ..Modifiers::NONE
                },
            ),
        ),
        (Action::NewTab, Keybinding::new(Key::T, Modifiers::CTRL)),
        (
            Action::DeleteTab,
            Keybinding::new(
                Key::W,
                Modifiers {
                    ctrl: true,
                    shift: true,
                    ..Modifiers::NONE
                },
            ),
        ),
        (
            Action::OpenSettings,
            Keybinding::new(Key::Comma, Modifiers::CTRL),
        ),
    ]
    .into_iter()
    .collect()
}

pub fn keybinding_from_event(event: &egui::Event) -> Option<Keybinding> {
    match event {
        egui::Event::Key {
            key,
            pressed: true,
            repeat: false,
            modifiers,
            ..
        } => Some(Keybinding::new(*key, *modifiers)),
        _ => None,
    }
}

fn key_name(key: Key) -> &'static str {
    match key {
        Key::Num0 => "0",
        Key::Num1 => "1",
        Key::Num2 => "2",
        Key::Num3 => "3",
        Key::Num4 => "4",
        Key::Num5 => "5",
        Key::Num6 => "6",
        Key::Num7 => "7",
        Key::Num8 => "8",
        Key::Num9 => "9",
        Key::Comma => ",",
        Key::Period => ".",
        Key::Space => "Space",
        Key::Enter => "Enter",
        Key::Tab => "Tab",
        _ => key.name(),
    }
}

fn parse_key(name: &str) -> Option<Key> {
    match name {
        "a" => Some(Key::A),
        "b" => Some(Key::B),
        "c" => Some(Key::C),
        "d" => Some(Key::D),
        "e" => Some(Key::E),
        "f" => Some(Key::F),
        "g" => Some(Key::G),
        "h" => Some(Key::H),
        "i" => Some(Key::I),
        "j" => Some(Key::J),
        "k" => Some(Key::K),
        "l" => Some(Key::L),
        "m" => Some(Key::M),
        "n" => Some(Key::N),
        "o" => Some(Key::O),
        "p" => Some(Key::P),
        "q" => Some(Key::Q),
        "r" => Some(Key::R),
        "s" => Some(Key::S),
        "t" => Some(Key::T),
        "u" => Some(Key::U),
        "v" => Some(Key::V),
        "w" => Some(Key::W),
        "x" => Some(Key::X),
        "y" => Some(Key::Y),
        "z" => Some(Key::Z),
        "0" => Some(Key::Num0),
        "1" => Some(Key::Num1),
        "2" => Some(Key::Num2),
        "3" => Some(Key::Num3),
        "4" => Some(Key::Num4),
        "5" => Some(Key::Num5),
        "6" => Some(Key::Num6),
        "7" => Some(Key::Num7),
        "8" => Some(Key::Num8),
        "9" => Some(Key::Num9),
        "," => Some(Key::Comma),
        "." => Some(Key::Period),
        "space" => Some(Key::Space),
        "enter" => Some(Key::Enter),
        "tab" => Some(Key::Tab),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::Keybinding;

    #[test]
    fn keybinding_round_trips_through_its_settings_string() {
        let binding: Keybinding = "Ctrl+Shift+8".parse().unwrap();
        assert_eq!(binding.to_string(), "Ctrl+Shift+8");
    }
}
