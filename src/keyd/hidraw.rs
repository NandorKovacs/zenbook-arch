//! The keyboard's hotkey interface (USB interface 4), through its hidraw node.
//!
//! The interface has one input report, `5a <code> 00 00 00 00`, and one
//! 16-byte feature report with the same id that takes commands.

use std::fs::File;
use std::io::{self, Read};
use std::os::fd::AsRawFd;

use crate::translator::{Keyboard, REPORT_ID};

/// Length of the feature report, id included.
const FEATURE_LEN: usize = 16;

/// `HIDIOCSFEATURE(len)` from linux/hidraw.h: `_IOC(_IOC_WRITE | _IOC_READ, 'H', 0x06, len)`.
const fn hidiocsfeature(len: usize) -> libc::c_ulong {
    const WRITE_READ: libc::c_ulong = 3;
    (WRITE_READ << 30) | ((len as libc::c_ulong) << 16) | ((b'H' as libc::c_ulong) << 8) | 0x06
}

pub struct Hidraw {
    file: File,
}

impl Hidraw {
    pub fn new(file: File) -> Self {
        Hidraw { file }
    }

    /// Reads the next input report. `Ok(0)` or an error means the keyboard
    /// is gone.
    pub fn read_report(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.file.read(buf)
    }

    /// Sends a command: the report id followed by `payload`, zero-padded.
    fn send_feature(&self, payload: &[u8]) -> io::Result<()> {
        let mut report = [0u8; FEATURE_LEN];
        report[0] = REPORT_ID;
        report[1..1 + payload.len()].copy_from_slice(payload);
        // SAFETY: the request encodes FEATURE_LEN, which is the size of the
        // buffer the kernel reads from.
        let rc = unsafe {
            libc::ioctl(
                self.file.as_raw_fd(),
                hidiocsfeature(FEATURE_LEN),
                report.as_mut_ptr(),
            )
        };
        if rc < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    /// The handshake of the kernel's hid-asus driver. Without it the keyboard
    /// sends plain F-keys and no hotkey reports.
    pub fn handshake(&self) -> io::Result<()> {
        self.send_feature(b"ASUS Tech.Inc.")
    }

    /// Chooses what the F-keys send without Fn: their hotkey, or F1-F12.
    pub fn set_hotkeys_first(&self, hotkeys_first: bool) -> io::Result<()> {
        self.send_feature(&[0xd0, 0x4e, if hotkeys_first { 0x00 } else { 0x01 }])
    }
}

impl Keyboard for Hidraw {
    fn set_backlight(&mut self, level: u8) -> io::Result<()> {
        self.send_feature(&[0xba, 0xc5, 0xc4, level])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feature_ioctl_number_matches_the_kernel_macro() {
        // HIDIOCSFEATURE(16): the value the keyboard accepted from the
        // Python prototype
        assert_eq!(hidiocsfeature(16), 0xc010_4806);
    }
}
