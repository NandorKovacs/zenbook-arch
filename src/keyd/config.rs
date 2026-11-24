//! Configuration: what each hotkey does. Every setting is optional.
//!
//! ```text
//! hotkeys_first = true     # F-keys send hotkeys; Fn + F-key sends F1-F12
//! backlight_level = 1      # keyboard backlight to set at start, 0-3
//!
//! [keys]                   # hotkey code (hex, as logged) = action
//! 10 = KEY_BRIGHTNESSDOWN  # a key name from keys.rs, or a key code number
//! c7 = backlight           # cycle the keyboard backlight
//! 86 = none                # ignore the key
//! ```

use std::collections::BTreeMap;

use crate::keys;

/// Number of keyboard backlight levels, off included.
pub const BACKLIGHT_LEVELS: u8 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Press this key for as long as the hotkey is held.
    Key(u16),
    /// Step the keyboard backlight to its next level.
    Backlight,
    /// Swallow the hotkey.
    None,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    /// The F-keys send their hotkey without Fn and F1-F12 with Fn.
    pub hotkeys_first: bool,
    /// Keyboard backlight level to set at start; `None` leaves it alone.
    pub backlight_level: Option<u8>,
    /// Hotkey code (second byte of the report) to action.
    pub actions: BTreeMap<u8, Action>,
}

impl Default for Config {
    fn default() -> Self {
        let actions = BTreeMap::from([
            (0x10, Action::Key(keys::KEY_BRIGHTNESSDOWN)),
            (0x20, Action::Key(keys::KEY_BRIGHTNESSUP)),
            (0xc7, Action::Backlight),
            // microphone mute; X maps F20 to XF86AudioMicMute
            (0x7c, Action::Key(keys::KEY_F20)),
            // emoji key
            (0x7e, Action::Key(keys::KEY_PROG1)),
            // MyASUS key
            (0x86, Action::Key(keys::KEY_PROG2)),
            // swap displays key
            (0x9c, Action::Key(keys::KEY_PROG3)),
            // lower display on/off key
            (0x6a, Action::Key(keys::KEY_SWITCHVIDEOMODE)),
        ]);
        Config {
            hotkeys_first: true,
            backlight_level: None,
            actions,
        }
    }
}

impl Config {
    /// Parses a config file on top of the defaults. A line that cannot be
    /// used is skipped and reported in the returned warnings, so that a typo
    /// does not take the hotkeys away.
    pub fn parse(text: &str) -> (Config, Vec<String>) {
        let mut config = Config::default();
        let mut warnings = Vec::new();
        let mut section = String::new();

        for (index, line) in text.lines().enumerate() {
            let line = line.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                section = name.trim().to_string();
                continue;
            }
            let result = match line.split_once('=') {
                Some((key, value)) => config.set(&section, unquote(key), unquote(value)),
                None => Err("expected `name = value`".to_string()),
            };
            if let Err(problem) = result {
                warnings.push(format!("line {}: {problem}", index + 1));
            }
        }
        (config, warnings)
    }

    fn set(&mut self, section: &str, key: &str, value: &str) -> Result<(), String> {
        match (section, key) {
            ("", "hotkeys_first") => {
                self.hotkeys_first = match value {
                    "true" => true,
                    "false" => false,
                    _ => {
                        return Err(format!(
                            "hotkeys_first must be true or false, not `{value}`"
                        ));
                    }
                };
            }
            ("", "backlight_level") => {
                let level = value.parse().ok().filter(|l| *l < BACKLIGHT_LEVELS);
                self.backlight_level = Some(level.ok_or_else(|| {
                    format!(
                        "backlight_level must be 0-{}, not `{value}`",
                        BACKLIGHT_LEVELS - 1
                    )
                })?);
            }
            ("keys", code) => {
                let code = u8::from_str_radix(code, 16)
                    .ok()
                    .filter(|c| *c != 0)
                    .ok_or_else(|| format!("`{code}` is not a hotkey code (hex, 01-ff)"))?;
                let action = match value {
                    "backlight" => Action::Backlight,
                    "none" => Action::None,
                    key => Action::Key(
                        keys::parse(key).ok_or_else(|| format!("unknown key or action `{key}`"))?,
                    ),
                };
                self.actions.insert(code, action);
            }
            ("", _) => return Err(format!("unknown setting `{key}`")),
            _ => return Err(format!("unknown section [{section}]")),
        }
        Ok(())
    }

    /// The keys the virtual keyboard has to announce.
    pub fn keys(&self) -> impl Iterator<Item = u16> + '_ {
        self.actions.values().filter_map(|action| match action {
            Action::Key(key) => Some(*key),
            _ => None,
        })
    }
}

fn unquote(text: &str) -> &str {
    let text = text.trim();
    text.strip_prefix('"')
        .and_then(|t| t.strip_suffix('"'))
        .unwrap_or(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_file_gives_the_defaults() {
        let (config, warnings) = Config::parse("");
        assert_eq!(config, Config::default());
        assert!(warnings.is_empty());
        assert!(config.hotkeys_first);
        assert_eq!(config.actions[&0x10], Action::Key(224));
        assert_eq!(config.actions[&0x20], Action::Key(225));
        assert_eq!(config.actions[&0xc7], Action::Backlight);
    }

    #[test]
    fn settings_and_keys_override_the_defaults() {
        let text = "
            # comment
            hotkeys_first = false
            backlight_level = 2   # trailing comment

            [keys]
            10 = KEY_F13
            \"86\" = \"none\"
            c7 = KEY_KBDILLUMTOGGLE
            35 = 200
            7e = backlight
        ";
        let (config, warnings) = Config::parse(text);
        assert_eq!(warnings, Vec::<String>::new());
        assert!(!config.hotkeys_first);
        assert_eq!(config.backlight_level, Some(2));
        assert_eq!(config.actions[&0x10], Action::Key(183));
        assert_eq!(config.actions[&0x86], Action::None);
        assert_eq!(config.actions[&0xc7], Action::Key(228));
        assert_eq!(config.actions[&0x35], Action::Key(200));
        assert_eq!(config.actions[&0x7e], Action::Backlight);
        // untouched default
        assert_eq!(config.actions[&0x20], Action::Key(225));
    }

    #[test]
    fn bad_lines_are_reported_and_skipped() {
        let text = "
            hotkeys_first = maybe
            backlight_level = 4
            colour = blue
            just some words
            [keys]
            zz = KEY_F13
            00 = KEY_F13
            10 = KEY_NOPE
            20 = KEY_F14
            [other]
            a = b
        ";
        let (config, warnings) = Config::parse(text);
        assert_eq!(warnings.len(), 8, "{warnings:?}");
        assert!(warnings[0].starts_with("line 2: hotkeys_first"));
        assert!(warnings[3].starts_with("line 5: expected"));
        // bad lines leave the defaults in place, good lines still apply
        assert!(config.hotkeys_first);
        assert_eq!(config.backlight_level, None);
        assert_eq!(config.actions[&0x10], Action::Key(224));
        assert_eq!(config.actions[&0x20], Action::Key(184));
    }

    #[test]
    fn keys_lists_only_key_actions() {
        let (config, _) = Config::parse("[keys]\n10 = none\n20 = KEY_F13\n");
        let keys: Vec<u16> = config.keys().collect();
        assert!(keys.contains(&183));
        assert!(!keys.contains(&224));
        assert!(!keys.contains(&225));
    }
}
