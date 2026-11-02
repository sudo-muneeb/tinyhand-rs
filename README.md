# TinyHand for Linux

Native Rust and X11 presentation pointer inspired by [TinyHand](https://github.com/nilbuild/tinyhand).
The original artwork and tap sample are included with attribution and their MIT license.

## Install

On Linux Mint 22.3, Ubuntu 24.04, or another compatible Debian based x86_64 system:

```sh
sudo apt install ./tinyhand-rs_0.1.0_amd64.deb
```

Launch **TinyHand for Linux** in the application menu, or run `tinyhand-rs`.
The executable and assets are also available in the portable archive. Extract it
and run `./tinyhand-rs` from its directory. Rust and Cargo are not needed to use
either release. Other distributions can use the portable archive after installing
the shared runtime libraries listed below.

Required runtime libraries: X11, Xext, Xrender, Xinerama, Cairo and ALSA.
A compositing manager with a 32 bit ARGB visual is needed for transparency.
Cinnamon normally provides one.

## Use

The app starts in presentation mode. The hand follows the real pointer.
The normal cursor is hidden while the X11 pointer grab is active.

| Action | Result |
| --- | --- |
| Click | Tap pulse and optional sound, with no click delivered to the underlying app |
| Hold and drag | Draw a line; after release it fades in about 3 seconds by default |
| Scroll | Move through the 36 hands and laser dot |
| Toolbar `Hands` | Open the grid; click again for the laser dot page |
| Toolbar arrows | Previous or next pointer |
| Toolbar `Sound on` or `Muted` | Toggle tap sound |
| Toolbar `Clear` | Erase ink |
| Toolbar `Settings` | Open settings panel |
| Toolbar `Exit` or `Esc` | Exit presentation mode and restore normal clicks |
| `Ctrl+Alt+H` | Toggle presentation mode globally |
| `Ctrl+Alt+X` | Clear ink globally |
| Tray left click | Toggle presentation mode |
| Tray right click | Activate and show settings |
| Tray middle click | Quit the process |

The toolbar appears near the bottom of the monitor containing the pointer.
The XEmbed tray icon appears if a compatible system tray is running. If there is
no tray, use the global toggle shortcut. Closing presentation mode leaves the
process running; middle click the tray icon, or terminate its process to quit.

## Settings

The settings panel offers pointer size, line thickness, color, fade time,
sound volume, start at login, and editable toggle and clear shortcuts. Click a
shortcut row and press a letter with Control, Alt or Shift. Settings are saved
in `${XDG_CONFIG_HOME:-~/.config}/tinyhand-rs/settings.conf`.
Start at login writes an XDG autostart entry in the same config tree.

## Safety and troubleshooting

The app requires an X11 session. Run `echo "$XDG_SESSION_TYPE"`; it should
print `x11`. Wayland compositors do not expose this global input grab, so this
release does not implement Wayland presentation mode.

If the app says `XGrabPointer failed`, another application may hold an X11
pointer grab. Exit that app and retry. If a global shortcut conflicts with the
desktop, change it in settings. The app prints diagnostics to the terminal.
If the background is opaque, ensure Cinnamon's compositor is enabled.

The X server automatically releases a pointer grab if the application process
dies or disconnects. Normal exit, `Esc`, and SIGINT/SIGTERM explicitly release
the grab. As a last resort after a hang, switch to another virtual terminal
and terminate `tinyhand-rs`.

## Build

On a development machine with Rust, X11 development headers, Cairo and ALSA:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
```

The project has no third party Rust crate dependencies.

## Verification on a real X11 desktop

1. Start from a terminal and confirm the hand, toolbar and tray icon appear.
2. Move across each display and check pointer alignment and toolbar placement.
3. Open a text editor beneath the overlay. Click in it and confirm focus and
   text cursor do not move; hold and drag to see an ink line fade after release.
4. Press `Ctrl+Alt+X` to clear; press `Esc` and verify a normal click now reaches
   the editor immediately. Toggle on again with `Ctrl+Alt+H`.
5. Select a hand and laser dot, adjust every settings control, quit and restart,
   then check that preferences persisted. Try the tray actions and audio toggle.
6. Try a resolution change and reconnect a second display if available.

## Known verification limits

The release was compiled, linted, tested and its transparent rendering inspected
in a headless build environment. That environment rejects local socket creation,
so Xvfb could not accept a display connection. Real X11 event delivery, Cinnamon
compositing, tray docking, sound output and multi monitor switching therefore
still require the above desktop check. No Wayland global input capture is claimed.
