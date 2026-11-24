//! duo-backlight-sync: keeps the lower screen of the Zenbook Duo as bright as
//! the top one.
//!
//! Brightness keys and desktop sliders only change the top screen's backlight.
//! This waits for that value to change (inotify, no polling) and copies it to
//! the lower screen's backlight, scaled to that device's range.

use std::ffi::CString;
use std::fs::{self, File};
use std::io::{self, Read};
use std::os::fd::FromRawFd;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const BACKLIGHTS: &str = "/sys/class/backlight";
const SOURCE: &str = "intel_backlight";
/// The lower screen's device is `card<N>-eDP-2-backlight`; N depends on the
/// order the graphics drivers load in.
const TARGET_SUFFIX: &str = "-eDP-2-backlight";

const USAGE: &str = "\
usage: duo-backlight-sync [SOURCE [TARGET]]

SOURCE and TARGET are device names in /sys/class/backlight. By default the
source is intel_backlight and the target the device ending in -eDP-2-backlight.";

fn read_number(path: &Path) -> io::Result<u64> {
    let text = fs::read_to_string(path)?;
    text.trim().parse().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("not a number: {text:?}"),
        )
    })
}

/// `value` out of `from_max`, expressed out of `to_max`, rounded.
fn scale(value: u64, from_max: u64, to_max: u64) -> u64 {
    if from_max == 0 {
        return 0;
    }
    ((value * to_max + from_max / 2) / from_max).min(to_max)
}

fn find_target() -> Option<String> {
    let mut names: Vec<String> = fs::read_dir(BACKLIGHTS)
        .ok()?
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .filter(|name| name.ends_with(TARGET_SUFFIX))
        .collect();
    names.sort();
    names.into_iter().next()
}

struct Sync {
    source: PathBuf,
    target: PathBuf,
}

impl Sync {
    fn new(source: &str, target: &str) -> Self {
        Sync {
            source: Path::new(BACKLIGHTS).join(source),
            target: Path::new(BACKLIGHTS).join(target),
        }
    }

    /// Copies the source's brightness to the target, if they differ.
    fn copy(&self) -> io::Result<()> {
        let value = scale(
            read_number(&self.source.join("brightness"))?,
            read_number(&self.source.join("max_brightness"))?,
            read_number(&self.target.join("max_brightness"))?,
        );
        let brightness = self.target.join("brightness");
        if read_number(&brightness)? != value {
            fs::write(&brightness, value.to_string())?;
        }
        Ok(())
    }

    /// Copies now and after every change of the source. Only returns on error.
    fn watch(&self) -> io::Result<()> {
        // Not `brightness`: a write to that file is only reported to watchers
        // on the same mount of /sys, and the service has its own
        // (PrivateNetwork=). The kernel itself reports every change on
        // `actual_brightness`, on all mounts.
        let path = self.source.join("actual_brightness");
        let c_path = CString::new(path.as_os_str().as_bytes())
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;

        // SAFETY: plain system calls; the descriptor is owned by `events`
        // from the moment it is known to be valid.
        let mut events = unsafe {
            let fd = libc::inotify_init1(libc::IN_CLOEXEC);
            if fd < 0 {
                return Err(io::Error::last_os_error());
            }
            let events = File::from_raw_fd(fd);
            if libc::inotify_add_watch(fd, c_path.as_ptr(), libc::IN_MODIFY) < 0 {
                return Err(io::Error::last_os_error());
            }
            events
        };

        // The watch is in place before the first copy, so a change in
        // between is not lost.
        let mut buf = [0u8; 4096];
        loop {
            if let Err(e) = self.copy() {
                eprintln!("cannot copy the brightness: {e}");
            }
            // blocks until the source changes; several queued changes are
            // read at once and need one copy
            if events.read(&mut buf)? == 0 {
                return Err(io::ErrorKind::UnexpectedEof.into());
            }
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() > 2 || args.iter().any(|arg| arg.starts_with('-')) {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    }
    let source = args.first().map_or(SOURCE, String::as_str);
    let Some(target) = args.get(1).cloned().or_else(find_target) else {
        eprintln!("no backlight device ending in {TARGET_SUFFIX} in {BACKLIGHTS}");
        return ExitCode::FAILURE;
    };

    eprintln!("copying the brightness of {source} to {target}");
    let error = Sync::new(source, &target).watch().unwrap_err();
    eprintln!("cannot watch {source}: {error}");
    ExitCode::FAILURE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_keeps_equal_ranges_unchanged() {
        assert_eq!(scale(0, 400, 400), 0);
        assert_eq!(scale(137, 400, 400), 137);
        assert_eq!(scale(400, 400, 400), 400);
    }

    #[test]
    fn scale_converts_between_ranges_with_rounding() {
        assert_eq!(scale(200, 400, 255), 128);
        assert_eq!(scale(400, 400, 255), 255);
        assert_eq!(scale(1, 400, 255), 1);
        assert_eq!(scale(255, 255, 400), 400);
    }

    #[test]
    fn scale_survives_odd_inputs() {
        assert_eq!(scale(5, 0, 400), 0);
        assert_eq!(scale(500, 400, 400), 400);
    }
}
