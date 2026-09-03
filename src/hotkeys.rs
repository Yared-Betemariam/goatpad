use egui::{Key, Modifiers};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{collections::HashMap, fmt, str::FromStr};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Action {
    ToggleBold,
    ToggleItalic,
    ToggleUnderline,
    ToggleStrikethrough,
    ToggleBulletList,
    ToggleNumberedList,
    InsertLink,
    NewTab,
    #[serde(alias = "DeleteTab")]
    CloseTab,
    NextTab,
    PreviousTab,
    OpenSettings,
}

impl Action {
    pub const ALL: [Self; 12] = [
        Self::ToggleBold,
        Self::ToggleItalic,
        Self::ToggleUnderline,
        Self::ToggleStrikethrough,
        Self::ToggleBulletList,
        Self::ToggleNumberedList,
        Self::InsertLink,
        Self::NewTab,
        Self::CloseTab,
        Self::NextTab,
        Self::PreviousTab,
        Self::OpenSettings,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::ToggleBold => "Bold",
            Self::ToggleItalic => "Italic",
            Self::ToggleUnderline => "Underline",
            Self::ToggleStrikethrough => "Strikethrough",
            Self::ToggleBulletList => "Bullet list",
            Self::ToggleNumberedList => "Numbered list",
            Self::InsertLink => "Insert link",
            Self::NewTab => "New tab",
            Self::CloseTab => "Close tab",
            Self::NextTab => "Next tab",
            Self::PreviousTab => "Previous tab",
            Self::OpenSettings => "Settings",
        }
    }

    pub const fn is_formatting(self) -> bool {
        matches!(
            self,
            Self::ToggleBold
                | Self::ToggleItalic
                | Self::ToggleUnderline
                | Self::ToggleStrikethrough
                | Self::ToggleBulletList
                | Self::ToggleNumberedList
                | Self::InsertLink
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
            Action::ToggleStrikethrough,
            Keybinding::new(
                Key::X,
                Modifiers {
                    ctrl: true,
                    shift: true,
                    ..Modifiers::NONE
                },
            ),
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
        (
            Action::ToggleNumberedList,
            Keybinding::new(
                Key::Num7,
                Modifiers {
                    ctrl: true,
                    shift: true,
                    ..Modifiers::NONE
                },
            ),
        ),
        (Action::InsertLink, Keybinding::new(Key::K, Modifiers::CTRL)),
        (Action::NewTab, Keybinding::new(Key::T, Modifiers::CTRL)),
        (
            Action::CloseTab,
            Keybinding::new(
                Key::W,
                Modifiers {
                    ctrl: true,
                    shift: true,
                    ..Modifiers::NONE
                },
            ),
        ),
        (Action::NextTab, Keybinding::new(Key::Tab, Modifiers::CTRL)),
        (
            Action::PreviousTab,
            Keybinding::new(
                Key::Tab,
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
        } if !is_modifier_key(*key) => Some(Keybinding::new(*key, *modifiers)),
        _ => None,
    }
}

fn is_modifier_key(key: Key) -> bool {
    matches!(
        key,
        Key::ShiftLeft
            | Key::ShiftRight
            | Key::ControlLeft
            | Key::ControlRight
            | Key::AltLeft
            | Key::AltRight
            | Key::SuperLeft
            | Key::SuperRight
    )
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
    Key::ALL
        .iter()
        .copied()
        .find(|key| key.name().eq_ignore_ascii_case(name) || key.symbol_or_name() == name)
        .filter(|key| !is_modifier_key(*key))
}

#[cfg(test)]
mod tests {
    use super::{Action, Keybinding, default_bindings, keybinding_from_event};
    use egui::{Event, Key, Modifiers};

    #[test]
    fn keybinding_round_trips_through_its_settings_string() {
        let binding: Keybinding = "Ctrl+Shift+8".parse().unwrap();
        assert_eq!(binding.to_string(), "Ctrl+Shift+8");
    }

    #[test]
    fn modifier_keys_cannot_become_shortcuts() {
        let event = Event::Key {
            key: Key::ControlLeft,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::CTRL,
        };
        assert!(keybinding_from_event(&event).is_none());
        assert!("Ctrl+ControlLeft".parse::<Keybinding>().is_err());
    }

    #[test]
    fn keyboard_tab_navigation_has_default_bindings() {
        let bindings = default_bindings();
        assert_eq!(bindings[&Action::NextTab].to_string(), "Ctrl+Tab");
        assert_eq!(bindings[&Action::PreviousTab].to_string(), "Ctrl+Shift+Tab");
        assert_eq!(bindings[&Action::OpenSettings].to_string(), "Ctrl+,");
    }
}
