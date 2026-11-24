//! A virtual keyboard made with the kernel's uinput device. Keys sent through
//! it look like keys of a real keyboard to X, Wayland and the console.

use std::fs::File;
use std::io::{self, Write};
use std::mem::size_of;
use std::os::fd::AsRawFd;

use crate::translator::KeySink;

const EV_SYN: u16 = 0x00;
const EV_KEY: u16 = 0x01;
const SYN_REPORT: u16 = 0;
const BUS_VIRTUAL: u16 = 0x06;

const NAME: &[u8] = b"Zenbook Duo hotkeys";

/// `struct input_id` from linux/input.h.
#[repr(C)]
struct InputId {
    bustype: u16,
    vendor: u16,
    product: u16,
    version: u16,
}

/// `struct uinput_setup` from linux/uinput.h.
#[repr(C)]
struct UinputSetup {
    id: InputId,
    name: [u8; 80],
    ff_effects_max: u32,
}

const IOC_NONE: libc::c_ulong = 0;
const IOC_WRITE: libc::c_ulong = 1;

/// `_IOC(dir, 'U', nr, size)` from the kernel's ioctl.h.
const fn uinput_ioc(dir: libc::c_ulong, nr: libc::c_ulong, size: usize) -> libc::c_ulong {
    (dir << 30) | ((size as libc::c_ulong) << 16) | ((b'U' as libc::c_ulong) << 8) | nr
}

const UI_DEV_CREATE: libc::c_ulong = uinput_ioc(IOC_NONE, 1, 0);
const UI_DEV_SETUP: libc::c_ulong = uinput_ioc(IOC_WRITE, 3, size_of::<UinputSetup>());
const UI_SET_EVBIT: libc::c_ulong = uinput_ioc(IOC_WRITE, 100, size_of::<libc::c_int>());
const UI_SET_KEYBIT: libc::c_ulong = uinput_ioc(IOC_WRITE, 101, size_of::<libc::c_int>());

fn check(rc: libc::c_int) -> io::Result<()> {
    if rc < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

pub struct VirtualKeyboard {
    /// The device exists for as long as this file is open.
    file: File,
}

impl VirtualKeyboard {
    /// Creates the keyboard on an open `/dev/uinput`. It can only ever send
    /// the given keys.
    pub fn new(file: File, keys: impl Iterator<Item = u16>) -> io::Result<Self> {
        let fd = file.as_raw_fd();
        let mut setup = UinputSetup {
            id: InputId {
                bustype: BUS_VIRTUAL,
                vendor: 0,
                product: 0,
                version: 1,
            },
            name: [0; 80],
            ff_effects_max: 0,
        };
        setup.name[..NAME.len()].copy_from_slice(NAME);

        // SAFETY: fd is open; the integer requests take their argument by
        // value, and UI_DEV_SETUP reads a `uinput_setup`, which `setup` is.
        unsafe {
            check(libc::ioctl(fd, UI_SET_EVBIT, libc::c_int::from(EV_KEY)))?;
            for key in keys {
                check(libc::ioctl(fd, UI_SET_KEYBIT, libc::c_int::from(key)))?;
            }
            check(libc::ioctl(fd, UI_DEV_SETUP, &setup as *const UinputSetup))?;
            check(libc::ioctl(fd, UI_DEV_CREATE))?;
        }
        Ok(VirtualKeyboard { file })
    }

    /// Writes one `struct input_event`. The kernel stamps the time itself.
    fn emit(&mut self, kind: u16, code: u16, value: i32) -> io::Result<()> {
        const TIME: usize = size_of::<libc::timeval>();
        let mut event = [0u8; TIME + 8];
        event[TIME..TIME + 2].copy_from_slice(&kind.to_ne_bytes());
        event[TIME + 2..TIME + 4].copy_from_slice(&code.to_ne_bytes());
        event[TIME + 4..].copy_from_slice(&value.to_ne_bytes());
        self.file.write_all(&event)
    }

    fn key(&mut self, key: u16, value: i32) -> io::Result<()> {
        self.emit(EV_KEY, key, value)?;
        self.emit(EV_SYN, SYN_REPORT, 0)
    }
}

impl KeySink for VirtualKeyboard {
    fn press(&mut self, key: u16) -> io::Result<()> {
        self.key(key, 1)
    }

    fn release(&mut self, key: u16) -> io::Result<()> {
        self.key(key, 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ioctl_numbers_match_the_kernel_header() {
        // values of the macros in linux/uinput.h on x86-64
        assert_eq!(UI_DEV_CREATE, 0x5501);
        assert_eq!(UI_DEV_SETUP, 0x405c_5503);
        assert_eq!(UI_SET_EVBIT, 0x4004_5564);
        assert_eq!(UI_SET_KEYBIT, 0x4004_5565);
    }

    /// Creates a real device and presses a key, so it needs access to
    /// /dev/uinput: `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn creates_a_device_and_sends_a_key() {
        let file = File::options()
            .read(true)
            .write(true)
            .open("/dev/uinput")
            .unwrap();
        let mut keyboard = VirtualKeyboard::new(file, [224, 225].into_iter()).unwrap();

        // give the display server time to pick the device up
        std::thread::sleep(std::time::Duration::from_secs(2));
        let devices = std::fs::read_to_string("/proc/bus/input/devices").unwrap();
        assert!(devices.contains("Zenbook Duo hotkeys"));

        keyboard.press(224).unwrap();
        keyboard.release(224).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
}
