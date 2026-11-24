//! The core of duo-keyd: one hotkey report in, key events or a backlight
//! change out. Device access sits behind two traits so that this logic can be
//! tested without hardware.

use std::collections::BTreeMap;
use std::io;

use crate::config::{Action, BACKLIGHT_LEVELS, Config};
use crate::keys;

/// Report id of the keyboard's hotkey reports: `5a <code> 00 00 00 00`.
pub const REPORT_ID: u8 = 0x5a;

/// The part of the keyboard the translator writes to.
pub trait Keyboard {
    fn set_backlight(&mut self, level: u8) -> io::Result<()>;
}

/// Where translated keys go.
pub trait KeySink {
    fn press(&mut self, key: u16) -> io::Result<()>;
    fn release(&mut self, key: u16) -> io::Result<()>;
}

pub struct Translator {
    actions: BTreeMap<u8, Action>,
    /// The key held down on behalf of the hotkey that is currently pressed.
    pressed: Option<u16>,
    backlight: u8,
    verbose: bool,
}

impl Translator {
    pub fn new(config: &Config, verbose: bool) -> Self {
        Translator {
            actions: config.actions.clone(),
            pressed: None,
            backlight: config.backlight_level.unwrap_or(0),
            verbose,
        }
    }

    /// Handles one report from the keyboard's hotkey interface.
    pub fn handle(
        &mut self,
        report: &[u8],
        keyboard: &mut impl Keyboard,
        sink: &mut impl KeySink,
    ) -> io::Result<()> {
        let &[REPORT_ID, code, ..] = report else {
            if self.verbose {
                eprintln!("ignored report {report:02x?}");
            }
            return Ok(());
        };

        // The keyboard reports one hotkey at a time: code 0 is the release,
        // and a new code means the previous key is no longer held.
        if let Some(key) = self.pressed.take() {
            sink.release(key)?;
        }
        if code == 0 {
            return Ok(());
        }

        match self.actions.get(&code).copied() {
            Some(Action::Key(key)) => {
                if self.verbose {
                    eprintln!("hotkey {code:02x} -> {}", keys::name(key));
                }
                sink.press(key)?;
                self.pressed = Some(key);
            }
            Some(Action::Backlight) => {
                self.backlight = (self.backlight + 1) % BACKLIGHT_LEVELS;
                if self.verbose {
                    eprintln!("hotkey {code:02x} -> backlight level {}", self.backlight);
                }
                keyboard.set_backlight(self.backlight)?;
            }
            Some(Action::None) => {}
            None => eprintln!("unmapped hotkey {code:02x}"),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::cell::RefCell;
    use std::rc::Rc;

    /// Stands in for both devices; clones share one log of what was done.
    #[derive(Clone, Default)]
    struct Fake {
        events: Rc<RefCell<Vec<String>>>,
    }

    impl Keyboard for Fake {
        fn set_backlight(&mut self, level: u8) -> io::Result<()> {
            self.events.borrow_mut().push(format!("backlight {level}"));
            Ok(())
        }
    }

    impl KeySink for Fake {
        fn press(&mut self, key: u16) -> io::Result<()> {
            self.events.borrow_mut().push(format!("press {key}"));
            Ok(())
        }
        fn release(&mut self, key: u16) -> io::Result<()> {
            self.events.borrow_mut().push(format!("release {key}"));
            Ok(())
        }
    }

    /// Runs reports through a translator and returns what it did, in order.
    fn run(config: &Config, reports: &[&[u8]]) -> Vec<String> {
        let mut translator = Translator::new(config, false);
        let mut keyboard = Fake::default();
        let mut sink = keyboard.clone();
        for report in reports {
            translator.handle(report, &mut keyboard, &mut sink).unwrap();
        }
        keyboard.events.take()
    }

    const DOWN: &[u8] = &[0x5a, 0x10, 0, 0, 0, 0];
    const UP: &[u8] = &[0x5a, 0x20, 0, 0, 0, 0];
    const BACKLIGHT: &[u8] = &[0x5a, 0xc7, 0, 0, 0, 0];
    const RELEASE: &[u8] = &[0x5a, 0, 0, 0, 0, 0];

    #[test]
    fn press_and_release_follow_the_hotkey() {
        let events = run(&Config::default(), &[DOWN, RELEASE]);
        assert_eq!(events, ["press 224", "release 224"]);
    }

    #[test]
    fn a_new_hotkey_releases_the_previous_key() {
        let events = run(&Config::default(), &[DOWN, UP, RELEASE]);
        assert_eq!(
            events,
            ["press 224", "release 224", "press 225", "release 225"]
        );
    }

    #[test]
    fn release_without_a_press_does_nothing() {
        assert!(run(&Config::default(), &[RELEASE, RELEASE]).is_empty());
    }

    #[test]
    fn backlight_key_cycles_through_the_levels() {
        let reports = [
            BACKLIGHT, RELEASE, BACKLIGHT, BACKLIGHT, BACKLIGHT, BACKLIGHT,
        ];
        let events = run(&Config::default(), &reports);
        assert_eq!(
            events,
            [
                "backlight 1",
                "backlight 2",
                "backlight 3",
                "backlight 0",
                "backlight 1"
            ]
        );
    }

    #[test]
    fn backlight_key_releases_a_held_key_first() {
        let events = run(&Config::default(), &[DOWN, BACKLIGHT, RELEASE]);
        assert_eq!(events, ["press 224", "release 224", "backlight 1"]);
    }

    #[test]
    fn backlight_cycle_starts_from_the_configured_level() {
        let (config, _) = Config::parse("backlight_level = 3");
        assert_eq!(run(&config, &[BACKLIGHT]), ["backlight 0"]);
    }

    #[test]
    fn none_and_unmapped_hotkeys_send_nothing() {
        let (config, _) = Config::parse("[keys]\n10 = none\n");
        let unmapped: &[u8] = &[0x5a, 0x35, 0, 0, 0, 0];
        assert!(run(&config, &[DOWN, RELEASE, unmapped, RELEASE]).is_empty());
    }

    #[test]
    fn other_reports_are_ignored_and_keep_the_key_held() {
        let other: &[u8] = &[0x42, 0x10, 0];
        let events = run(&Config::default(), &[DOWN, other, &[], &[0x5a], RELEASE]);
        assert_eq!(events, ["press 224", "release 224"]);
    }
}
