// Standalone keyboard-to-guest-key mapping.

use std::str::FromStr;

use minifb::{Key, Window};
use nicaiemu_core::NicaiMachine;

const DEFAULT_KEY_MAP: &[(u8, &[Key])] = &[
    (0, &[Key::Key0]),
    (1, &[Key::Key1]),
    (2, &[Key::Key2]),
    (3, &[Key::Key3]),
    (4, &[Key::Key4]),
    (5, &[Key::Key5]),
    (6, &[Key::Key6]),
    (7, &[Key::Key7]),
    (8, &[Key::Key8]),
    (9, &[Key::Key9]),
    (12, &[Key::Q]),
    (13, &[Key::E]),
    (14, &[Key::Enter, Key::F]),
    (15, &[Key::Left, Key::A]),
    (16, &[Key::Right, Key::D]),
    (17, &[Key::Up, Key::W]),
    (18, &[Key::Down, Key::S]),
    (19, &[Key::N]),
    (20, &[Key::M]),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RemapSpec {
    guest_key: u8,
    key: Key,
}

impl std::fmt::Display for RemapSpec {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{}:{}",
            guest_key_name(self.guest_key),
            key_name(self.key)
        )
    }
}

impl FromStr for RemapSpec {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (guest_key, key) = value
            .split_once(':')
            .ok_or_else(|| "expected GUEST_KEY:KEY, for example enter:space".to_string())?;
        let guest_key = parse_guest_key(guest_key.trim())?;
        let key = parse_key(key.trim())?;
        Ok(Self { guest_key, key })
    }
}

/// Maps host keyboard state onto guest keys using the default table plus
/// any user-supplied remappings.
pub struct KeyboardMapper {
    mappings: Vec<(u8, Vec<Key>)>,
}

impl KeyboardMapper {
    pub fn new(remappings: &[RemapSpec]) -> Self {
        let mut mappings: Vec<(u8, Vec<Key>)> = DEFAULT_KEY_MAP
            .iter()
            .map(|&(guest_key, keys)| (guest_key, keys.to_vec()))
            .collect();
        for remapping in remappings {
            if let Some((_, keys)) = mappings
                .iter_mut()
                .find(|(guest_key, _)| *guest_key == remapping.guest_key)
            {
                keys.clear();
                keys.push(remapping.key);
            }
        }
        Self { mappings }
    }

    /// Push the current window key state into the machine.
    pub fn apply(&self, window: &Window, machine: &mut NicaiMachine) {
        self.update_keys(
            |key| window.is_key_down(key),
            |guest_key, down| machine.set_key(guest_key, down),
        );
    }

    fn update_keys(&self, is_down: impl FnMut(Key) -> bool, mut set_key: impl FnMut(u8, bool)) {
        let mut is_down = is_down;
        for &(guest_key, ref host_keys) in &self.mappings {
            set_key(guest_key, host_keys.iter().any(|key| is_down(*key)));
        }
    }
}

fn parse_guest_key(name: &str) -> Result<u8, String> {
    match name.to_ascii_lowercase().as_str() {
        "0" => Ok(0),
        "1" => Ok(1),
        "2" => Ok(2),
        "3" => Ok(3),
        "4" => Ok(4),
        "5" => Ok(5),
        "6" => Ok(6),
        "7" => Ok(7),
        "8" => Ok(8),
        "9" => Ok(9),
        "q" => Ok(12),
        "e" => Ok(13),
        "enter" => Ok(14),
        "left" => Ok(15),
        "right" => Ok(16),
        "up" => Ok(17),
        "down" => Ok(18),
        "n" => Ok(19),
        "m" => Ok(20),
        _ => Err(format!(
            "unknown guest key '{name}'; expected 0-9, q, e, enter, left, right, up, down, n, or m"
        )),
    }
}

fn guest_key_name(guest_key: u8) -> &'static str {
    match guest_key {
        0..=9 => match guest_key {
            0 => "0",
            1 => "1",
            2 => "2",
            3 => "3",
            4 => "4",
            5 => "5",
            6 => "6",
            7 => "7",
            8 => "8",
            _ => "9",
        },
        12 => "q",
        13 => "e",
        14 => "enter",
        15 => "left",
        16 => "right",
        17 => "up",
        18 => "down",
        19 => "n",
        20 => "m",
        _ => "?",
    }
}

fn key_name(key: Key) -> &'static str {
    match key {
        Key::Key0 => "0",
        Key::Key1 => "1",
        Key::Key2 => "2",
        Key::Key3 => "3",
        Key::Key4 => "4",
        Key::Key5 => "5",
        Key::Key6 => "6",
        Key::Key7 => "7",
        Key::Key8 => "8",
        Key::Key9 => "9",
        Key::A => "a",
        Key::B => "b",
        Key::C => "c",
        Key::D => "d",
        Key::E => "e",
        Key::F => "f",
        Key::G => "g",
        Key::H => "h",
        Key::I => "i",
        Key::J => "j",
        Key::K => "k",
        Key::L => "l",
        Key::M => "m",
        Key::N => "n",
        Key::O => "o",
        Key::P => "p",
        Key::Q => "q",
        Key::R => "r",
        Key::S => "s",
        Key::T => "t",
        Key::U => "u",
        Key::V => "v",
        Key::W => "w",
        Key::X => "x",
        Key::Y => "y",
        Key::Z => "z",
        Key::Up => "up",
        Key::Down => "down",
        Key::Left => "left",
        Key::Right => "right",
        Key::Space => "space",
        Key::Enter => "enter",
        Key::Backspace => "backspace",
        Key::Tab => "tab",
        Key::Delete => "delete",
        Key::Home => "home",
        Key::End => "end",
        Key::PageUp => "pageup",
        Key::PageDown => "pagedown",
        Key::LeftShift => "leftshift",
        Key::RightShift => "rightshift",
        Key::LeftCtrl => "leftctrl",
        Key::RightCtrl => "rightctrl",
        Key::LeftAlt => "leftalt",
        Key::RightAlt => "rightalt",
        _ => "?",
    }
}

fn parse_key(name: &str) -> Result<Key, String> {
    let normalized = name.to_ascii_lowercase();
    let key = match normalized.as_str() {
        "a" => Key::A,
        "b" => Key::B,
        "c" => Key::C,
        "d" => Key::D,
        "e" => Key::E,
        "f" => Key::F,
        "g" => Key::G,
        "h" => Key::H,
        "i" => Key::I,
        "j" => Key::J,
        "k" => Key::K,
        "l" => Key::L,
        "m" => Key::M,
        "n" => Key::N,
        "o" => Key::O,
        "p" => Key::P,
        "q" => Key::Q,
        "r" => Key::R,
        "s" => Key::S,
        "t" => Key::T,
        "u" => Key::U,
        "v" => Key::V,
        "w" => Key::W,
        "x" => Key::X,
        "y" => Key::Y,
        "z" => Key::Z,
        "0" => Key::Key0,
        "1" => Key::Key1,
        "2" => Key::Key2,
        "3" => Key::Key3,
        "4" => Key::Key4,
        "5" => Key::Key5,
        "6" => Key::Key6,
        "7" => Key::Key7,
        "8" => Key::Key8,
        "9" => Key::Key9,
        "f1" => Key::F1,
        "f2" => Key::F2,
        "f3" => Key::F3,
        "f4" => Key::F4,
        "f5" => Key::F5,
        "f6" => Key::F6,
        "f7" => Key::F7,
        "f8" => Key::F8,
        "f9" => Key::F9,
        "f10" => Key::F10,
        "f11" => Key::F11,
        "f12" => Key::F12,
        "up" => Key::Up,
        "down" => Key::Down,
        "left" => Key::Left,
        "right" => Key::Right,
        "space" => Key::Space,
        "enter" | "return" => Key::Enter,
        "backspace" => Key::Backspace,
        "tab" => Key::Tab,
        "delete" => Key::Delete,
        "home" => Key::Home,
        "end" => Key::End,
        "pageup" => Key::PageUp,
        "pagedown" => Key::PageDown,
        "leftshift" => Key::LeftShift,
        "rightshift" => Key::RightShift,
        "leftctrl" => Key::LeftCtrl,
        "rightctrl" => Key::RightCtrl,
        "leftalt" => Key::LeftAlt,
        "rightalt" => Key::RightAlt,
        "escape" | "esc" => {
            return Err("escape is reserved for exiting the standalone emulator".to_string());
        }
        _ => return Err(format!("unknown key '{name}'")),
    };
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn remap(value: &str) -> RemapSpec {
        value.parse().unwrap()
    }

    #[test]
    fn default_mapping_covers_all_guest_keys() {
        let mapper = KeyboardMapper::new(&[]);
        let mut pressed = Vec::new();
        mapper.update_keys(
            |_| true,
            |guest_key, down| {
                if down {
                    pressed.push(guest_key);
                }
            },
        );
        assert_eq!(
            pressed,
            DEFAULT_KEY_MAP
                .iter()
                .map(|&(guest_key, _)| guest_key)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn remapping_replaces_the_default_keys() {
        let mapper = KeyboardMapper::new(&[remap("enter:space")]);
        let mut pressed = Vec::new();
        mapper.update_keys(
            |key| key == Key::Space,
            |guest_key, down| {
                if down {
                    pressed.push(guest_key);
                }
            },
        );
        assert_eq!(pressed, [14]);

        let mut pressed = Vec::new();
        mapper.update_keys(
            |key| key == Key::Enter,
            |guest_key, down| {
                if down {
                    pressed.push(guest_key);
                }
            },
        );
        assert!(pressed.is_empty());
    }

    #[test]
    fn last_duplicate_remapping_wins() {
        let mapper = KeyboardMapper::new(&[remap("enter:space"), remap("enter:backspace")]);
        let mut pressed = Vec::new();
        mapper.update_keys(
            |key| key == Key::Space,
            |guest_key, down| {
                if down {
                    pressed.push(guest_key);
                }
            },
        );
        assert!(pressed.is_empty());

        let mut pressed = Vec::new();
        mapper.update_keys(
            |key| key == Key::Backspace,
            |guest_key, down| {
                if down {
                    pressed.push(guest_key);
                }
            },
        );
        assert_eq!(pressed, [14]);
    }

    #[test]
    fn parser_accepts_all_guest_key_names() {
        for name in [
            "0", "1", "9", "q", "e", "enter", "left", "right", "up", "down", "n", "m",
        ] {
            assert!(format!("{name}:space").parse::<RemapSpec>().is_ok());
        }
    }

    #[test]
    fn parser_rejects_invalid_specs_and_reserved_escape() {
        assert!("enter".parse::<RemapSpec>().is_err());
        assert!("a:space".parse::<RemapSpec>().is_err());
        assert!("enter:not-a-key".parse::<RemapSpec>().is_err());
        assert!("enter:escape".parse::<RemapSpec>().is_err());
    }
}
