//! duo-keyd: turns the hotkey reports of the ASUS Zenbook Duo keyboard into
//! ordinary key events.
//!
//! The keyboard sends the keys of its top row (brightness, keyboard backlight,
//! ...) on a vendor interface that the kernel has no driver for. duo-keyd
//! enables those reports, reads them from the interface's hidraw node and
//! presses the matching keys on a virtual keyboard, so that every desktop
//! sees them like the keys of any laptop. The keyboard backlight key is
//! handled here, because changing the backlight needs the device.
//!
//! It runs until the keyboard is detached; udev starts it again on attach.

mod config;
mod fds;
mod hidraw;
mod keys;
mod translator;
mod uinput;

use std::fs::File;
use std::io;
use std::path::PathBuf;
use std::process::ExitCode;

use config::Config;
use fds::PassedFiles;
use hidraw::Hidraw;
use translator::{Keyboard, Translator};
use uinput::VirtualKeyboard;

const USAGE: &str = "\
usage: duo-keyd [--config FILE] [--hidraw DEVICE] [--uinput DEVICE] [--verbose]

Devices passed by systemd under the names `hidraw` and `uinput` (OpenFile= in
the unit) take precedence over the paths.";

struct Args {
    config: PathBuf,
    hidraw: PathBuf,
    uinput: PathBuf,
    verbose: bool,
}

fn parse_args(mut args: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut parsed = Args {
        config: "/etc/duo-keyd.conf".into(),
        hidraw:
            "/dev/input/by-id/usb-Primax_Electronics_Ltd._ASUS_Zenbook_Duo_Keyboard-if04-hidraw"
                .into(),
        uinput: "/dev/uinput".into(),
        verbose: false,
    };
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or_else(|| format!("{arg} needs a value"));
        match arg.as_str() {
            "--config" => parsed.config = value()?.into(),
            "--hidraw" => parsed.hidraw = value()?.into(),
            "--uinput" => parsed.uinput = value()?.into(),
            "--verbose" => parsed.verbose = true,
            _ => return Err(format!("unknown argument {arg}")),
        }
    }
    Ok(parsed)
}

/// Reads the config file. A missing file means the defaults.
fn read_config(path: &PathBuf) -> Result<Config, String> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Config::default()),
        Err(e) => return Err(format!("cannot read {}: {e}", path.display())),
    };
    let (config, warnings) = Config::parse(&text);
    for warning in warnings {
        eprintln!("{}: {warning}", path.display());
    }
    Ok(config)
}

fn open(passed: &mut PassedFiles, name: &str, path: &PathBuf) -> Result<File, String> {
    if let Some(file) = passed.take(name) {
        return Ok(file);
    }
    File::options()
        .read(true)
        .write(true)
        .open(path)
        .map_err(|e| format!("cannot open {}: {e}", path.display()))
}

fn run(args: &Args) -> Result<(), String> {
    let config = read_config(&args.config)?;

    let mut passed = PassedFiles::from_env();
    let mut keyboard = Hidraw::new(open(&mut passed, "hidraw", &args.hidraw)?);
    let uinput = open(&mut passed, "uinput", &args.uinput)?;

    // Neither failure is fatal by itself: which of the two the keyboard needs
    // may depend on its firmware.
    if let Err(e) = keyboard.handshake() {
        eprintln!("handshake failed: {e}");
    }
    if let Err(e) = keyboard.set_hotkeys_first(config.hotkeys_first) {
        eprintln!("setting the Fn mode failed: {e}");
    }
    if let Some(level) = config.backlight_level
        && let Err(e) = keyboard.set_backlight(level)
    {
        eprintln!("setting the backlight failed: {e}");
    }

    let mut sink = VirtualKeyboard::new(uinput, config.keys())
        .map_err(|e| format!("cannot create the virtual keyboard: {e}"))?;
    let mut translator = Translator::new(&config, args.verbose);
    eprintln!("listening for hotkeys");

    let mut report = [0u8; 64];
    loop {
        let len = match keyboard.read_report(&mut report) {
            Ok(0) => break,
            Ok(len) => len,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => {
                eprintln!("keyboard gone: {e}");
                break;
            }
        };
        if let Err(e) = translator.handle(&report[..len], &mut keyboard, &mut sink) {
            eprintln!("cannot act on report {:02x?}: {e}", &report[..len]);
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    let args = match parse_args(std::env::args().skip(1)) {
        Ok(args) => args,
        Err(problem) => {
            eprintln!("{problem}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(problem) => {
            eprintln!("{problem}");
            ExitCode::FAILURE
        }
    }
}
