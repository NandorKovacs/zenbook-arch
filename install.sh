#!/bin/sh
# Installs or removes duo-keyd and duo-backlight-sync. Build first, as your
# own user:
#
#   cargo build --release
#   sudo ./install.sh              # or: sudo ./install.sh uninstall
set -eu

here=$(cd "$(dirname "$0")" && pwd)
bin=$here/target/release
units=/etc/systemd/system
rules=/etc/udev/rules.d/70-duo-keyboard.rules
config=/etc/duo-keyd.conf

if [ "$(id -u)" -ne 0 ]; then
    echo "run as root: sudo $0 ${1:-install}" >&2
    exit 1
fi

case ${1:-install} in
install)
    for program in duo-keyd duo-backlight-sync; do
        if [ ! -x "$bin/$program" ]; then
            echo "$bin/$program is missing; run 'cargo build --release' first" >&2
            exit 1
        fi
    done

    install -Dm755 "$bin/duo-keyd" /usr/local/bin/duo-keyd
    install -Dm755 "$bin/duo-backlight-sync" /usr/local/bin/duo-backlight-sync
    install -Dm644 "$here/system/duo-keyd.service" "$units/duo-keyd.service"
    install -Dm644 "$here/system/duo-backlight-sync.service" "$units/duo-backlight-sync.service"
    install -Dm644 "$here/system/70-duo-keyboard.rules" "$rules"
    # keep an existing configuration
    [ -e "$config" ] || install -Dm644 "$here/system/duo-keyd.conf" "$config"

    systemctl daemon-reload
    systemctl enable duo-backlight-sync.service
    systemctl restart duo-backlight-sync.service

    # Let udev tag the keyboard, which starts duo-keyd if it is attached.
    # The restart picks up a new binary when it was already running.
    udevadm control --reload
    udevadm trigger --subsystem-match=hidraw
    udevadm settle
    if [ -e /dev/duo_keyboard_hotkeys ]; then
        systemctl restart duo-keyd.service
    else
        echo "keyboard not attached; duo-keyd starts when it is"
    fi

    if systemctl is-active --quiet brightness-sync.service; then
        echo "note: brightness-sync.service does the same job as duo-backlight-sync;"
        echo "      disable one of them: systemctl disable --now brightness-sync.service"
    fi
    ;;
uninstall)
    systemctl disable --now duo-backlight-sync.service 2>/dev/null || true
    systemctl stop duo-keyd.service 2>/dev/null || true
    rm -f /usr/local/bin/duo-keyd /usr/local/bin/duo-backlight-sync
    rm -f "$units/duo-keyd.service" "$units/duo-backlight-sync.service" "$rules"
    systemctl daemon-reload
    udevadm control --reload
    udevadm trigger --subsystem-match=hidraw
    echo "removed; $config is kept"
    ;;
*)
    echo "usage: $0 [install|uninstall]" >&2
    exit 2
    ;;
esac
