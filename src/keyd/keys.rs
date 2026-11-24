//! Names of the Linux key codes a hotkey can be mapped to.

/// Highest key code the kernel accepts (`KEY_MAX` in linux/input-event-codes.h).
const KEY_MAX: u16 = 0x2ff;

/// Key codes from linux/input-event-codes.h. Any other key can be given by
/// its number.
///
/// X11 only sees codes up to 247, so keys such as `KEY_MICMUTE` are of no use
/// there; `KEY_F20` is what X maps to XF86AudioMicMute.
const NAMES: &[(&str, u16)] = &[
    ("KEY_MUTE", 113),
    ("KEY_VOLUMEDOWN", 114),
    ("KEY_VOLUMEUP", 115),
    ("KEY_PROG1", 148),
    ("KEY_PROG2", 149),
    ("KEY_CONFIG", 171),
    ("KEY_F13", 183),
    ("KEY_F14", 184),
    ("KEY_F15", 185),
    ("KEY_F16", 186),
    ("KEY_F17", 187),
    ("KEY_F18", 188),
    ("KEY_F19", 189),
    ("KEY_F20", 190),
    ("KEY_F21", 191),
    ("KEY_F22", 192),
    ("KEY_F23", 193),
    ("KEY_F24", 194),
    ("KEY_PROG3", 202),
    ("KEY_PROG4", 203),
    ("KEY_BRIGHTNESSDOWN", 224),
    ("KEY_BRIGHTNESSUP", 225),
    ("KEY_SWITCHVIDEOMODE", 227),
    ("KEY_KBDILLUMTOGGLE", 228),
    ("KEY_KBDILLUMDOWN", 229),
    ("KEY_KBDILLUMUP", 230),
    ("KEY_DISPLAY_OFF", 245),
    ("KEY_MICMUTE", 248),
    ("KEY_CONTROLPANEL", 0x243),
    ("KEY_EMOJI_PICKER", 0x249),
];

pub const KEY_BRIGHTNESSDOWN: u16 = 224;
pub const KEY_BRIGHTNESSUP: u16 = 225;
pub const KEY_SWITCHVIDEOMODE: u16 = 227;
pub const KEY_PROG1: u16 = 148;
pub const KEY_PROG2: u16 = 149;
pub const KEY_PROG3: u16 = 202;
pub const KEY_F20: u16 = 190;

/// Key code for a name from the table above or for a decimal number.
pub fn parse(text: &str) -> Option<u16> {
    if let Some(&(_, code)) = NAMES.iter().find(|(name, _)| *name == text) {
        return Some(code);
    }
    text.parse()
        .ok()
        .filter(|code| (1..=KEY_MAX).contains(code))
}

/// Name of a key code for log messages.
pub fn name(code: u16) -> String {
    match NAMES.iter().find(|(_, c)| *c == code) {
        Some((name, _)) => name.to_string(),
        None => code.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_names_and_numbers() {
        assert_eq!(parse("KEY_BRIGHTNESSUP"), Some(225));
        assert_eq!(parse("KEY_EMOJI_PICKER"), Some(0x249));
        assert_eq!(parse("190"), Some(190));
    }

    #[test]
    fn rejects_unknown_names_and_out_of_range_numbers() {
        assert_eq!(parse("KEY_NOPE"), None);
        assert_eq!(parse("0"), None);
        assert_eq!(parse("768"), None);
        assert_eq!(parse(""), None);
    }

    #[test]
    fn names_codes_for_logs() {
        assert_eq!(name(224), "KEY_BRIGHTNESSDOWN");
        assert_eq!(name(30), "30");
    }

    #[test]
    fn constants_match_the_table() {
        assert_eq!(parse("KEY_BRIGHTNESSDOWN"), Some(KEY_BRIGHTNESSDOWN));
        assert_eq!(parse("KEY_BRIGHTNESSUP"), Some(KEY_BRIGHTNESSUP));
        assert_eq!(parse("KEY_SWITCHVIDEOMODE"), Some(KEY_SWITCHVIDEOMODE));
        assert_eq!(parse("KEY_PROG1"), Some(KEY_PROG1));
        assert_eq!(parse("KEY_PROG2"), Some(KEY_PROG2));
        assert_eq!(parse("KEY_PROG3"), Some(KEY_PROG3));
        assert_eq!(parse("KEY_F20"), Some(KEY_F20));
    }
}
