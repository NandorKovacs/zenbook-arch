# zenbook-arch

System-level fixes for the ASUS Zenbook Duo (UX8406MA) on Arch Linux.

| Program | What it does |
| --- | --- |
| `duo-keyd` | Makes the keyboard's hotkey row (brightness, keyboard backlight, ...) arrive as ordinary keys, for every desktop. |
| `duo-backlight-sync` | Copies the top screen's brightness to the lower screen whenever it changes. |

Neither touches the displays themselves, and neither polls.

## Why the hotkeys need help

The detachable keyboard (USB `0b05:1b2c`) sends its hotkeys on a vendor
interface that the kernel has no driver for, and only after a handshake. Until
then the F-keys are plain F1-F12, with or without Fn.

`duo-keyd` sends the handshake, reads the hotkey reports from the interface's
hidraw node and presses the matching key on a virtual keyboard (uinput). The
keyboard backlight key is handled inside `duo-keyd`, because changing the
backlight needs the device.

Detached, the keyboard connects over Bluetooth as another device
(`0b05:1b2d`) that behaves the same way, so `duo-keyd` serves both. Pair it
once, as usual.

What a key *does* is left to the desktop: bind `XF86MonBrightnessUp` and
`XF86MonBrightnessDown` like on any laptop.

## Install

```sh
cargo build --release
sudo ./install.sh
```

This installs the two programs to `/usr/local/bin`, two units to
`/etc/systemd/system`, udev rules and, if there is none yet,
`/etc/duo-keyd.conf`. `sudo ./install.sh uninstall` removes everything but the
config.

- `duo-keyd@.service` has one instance per way the keyboard can be connected:
  `duo-keyd@usb` (docked) and `duo-keyd@bt` (detached). udev starts an
  instance when the keyboard appears that way, and it stops when the keyboard
  goes away. systemd opens the two devices and passes them in, so the service
  runs as a throwaway unprivileged user.
- `duo-backlight-sync.service` runs from boot, as root, because only root may
  write the backlight files.

## Keys

Defaults, with what X calls each key:

| Hotkey code | Key on the keyboard | Sent as | X keysym |
| --- | --- | --- | --- |
| `10` | brightness down | `KEY_BRIGHTNESSDOWN` | `XF86MonBrightnessDown` |
| `20` | brightness up | `KEY_BRIGHTNESSUP` | `XF86MonBrightnessUp` |
| `c7` | keyboard backlight | handled internally: next level | |
| `7c` | microphone mute | `KEY_F20` | `XF86AudioMicMute` |
| `7e` | emoji | `KEY_PROG1` | `XF86Launch1` |
| `86` | MyASUS | `KEY_PROG2` | `XF86Launch2` |
| `9c` | swap displays | `KEY_PROG3` | `XF86Launch3` |
| `6a` | lower display | `KEY_SWITCHVIDEOMODE` | `XF86Display` |

Volume, mute and the display-mode key are sent by the keyboard itself; the
display-mode key (between brightness up and swap displays) as Super+P.

Change the mapping, the Fn mode or the start-up backlight level in
`/etc/duo-keyd.conf`; the file explains itself.

## Troubleshooting

```sh
systemctl status 'duo-keyd@*' duo-backlight-sync
journalctl -u 'duo-keyd@*' -b
```

- A hotkey that has no mapping is logged as `unmapped hotkey <code>`.
- To watch every report, stop the service and run
  `sudo duo-keyd --verbose --hidraw /dev/duo_keyboard_usb` (or
  `/dev/duo_keyboard_bt`) by hand. Over Bluetooth the ordinary keys arrive on
  the same device, so this prints every keystroke as well.

## Not verified on hardware yet

- Which of the two enabling commands the keyboard needs (the hid-asus
  handshake or the Fn-mode command); `duo-keyd` sends both.

## Development

```sh
cargo test                  # logic, with fake devices
cargo test -- --ignored     # also creates a real virtual keyboard; needs /dev/uinput
```
