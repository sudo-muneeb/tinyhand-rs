# TinyHand for Linux

Native Rust and X11 presentation pointer inspired by [TinyHand](https://github.com/nilbuild/tinyhand).
The original artwork and tap sample are included with attribution and their MIT license.

**Cinnamon compatibility:** Version 0.1.4 disables the legacy XEmbed tray on
Cinnamon/Muffin. The tray path reproduced the compositor crash in an isolated
Cinnamon session, with the same stack trace as the earlier live crashes.
Detection checks both desktop variables and the actual X11 window manager.
Use `Ctrl+Alt+H` to toggle presentation mode and `Esc` to restore normal clicks.
The former Cinnamon startup block and development override are no longer used.

## Install

On a compatible Debian based x86_64 X11 desktop:

```sh
sudo apt install ./tinyhand-rs_0.1.4_amd64.deb
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
The hand aims toward a grip below the current monitor, with a gently following
rod. Pressing compresses the hand and lowers its shadow; releasing lets it ease
back to rest. The fingertip stays aligned with the actual pointer. Appearance's
tilt control adds an angle offset to this natural aim. Animation stops when the
grip and press motion have settled.

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
The XEmbed tray icon appears if a compatible system tray is running, except on
Cinnamon where the legacy tray path is disabled. If there is
no tray, use the global toggle shortcut. Closing presentation mode leaves the
process running; middle click the tray icon, or terminate its process to quit.

## Settings

The settings window follows the original TinyHand design: a dark sidebar with
General, Pointers, Appearance, Drawing, Sound, and About pages. It offers hand
thumbnails, PNG import, size, shadow, tilt, opacity, ink colour, fade, thickness,
all six original tap sounds, volume, start at login, and editable shortcuts.
Click a shortcut row and press a letter with Control, Alt or Shift. Settings are saved
in `${XDG_CONFIG_HOME:-~/.config}/tinyhand-rs/settings.conf`.
Start at login writes an XDG autostart entry in the same config tree.
The hand is rendered above the toolbar and settings controls. Close settings
with the red corner button. PNG import uses `zenity` and temporarily releases
pointer capture while the native file chooser is open. An imported PNG must
be at most 2048 x 2048 pixels and smaller than 16 MB; it remains at its chosen
file path, so keep that file available. Install the chooser with
`sudo apt install zenity` if necessary.

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

## Install the build tools

On Linux Mint or Ubuntu, install the native libraries and the current Rust toolchain:

```sh
sudo apt update
sudo apt install -y build-essential pkg-config libx11-dev libxext-dev libxrender-dev libxinerama-dev libcairo2-dev libasound2-dev
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
. "$HOME/.cargo/env"
rustup component add rustfmt clippy
```

Open a new terminal if `cargo` is not found after installation. Build and check the application with:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
```

The project has no third party Rust crate dependencies.
Only run the GUI on an X11 display. Presentation mode captures mouse clicks;
`Esc` or `Ctrl+Alt+H` releases the capture.

## Verification on a real X11 desktop

1. Start from a terminal and confirm the hand and toolbar appear. Cinnamon
   intentionally has no legacy tray icon.
2. Move across each display and check pointer alignment and toolbar placement.
3. Open a text editor beneath the overlay. Click in it and confirm focus and
   text cursor do not move; hold and drag to see an ink line fade after release.
4. Press `Ctrl+Alt+X` to clear; press `Esc` and verify a normal click now reaches
   the editor immediately. Toggle on again with `Ctrl+Alt+H`.
5. Select a hand and laser dot, adjust every settings control, quit and restart,
   then check that preferences persisted. Try the tray actions and audio toggle.
6. Try a resolution change and reconnect a second display if available.

## Verification limits

The repeatable compositor regression test uses a separate Xvfb display, private
D-Bus session, temporary settings, and software rendering. It checks visible
ink, click capture and release, settings persistence, shortcuts, repeated
startup/shutdown, and delayed compositor stack updates. Run it with:

```sh
sudo apt install xvfb xdotool x11-utils imagemagick python3-pil zenity
cargo build --release
python3 scripts/test-cinnamon.py --cycles 20
```

Cinnamon must also be installed. Logs and screenshots stay in the printed
temporary directory. The test never sends input to your existing display.
Sound output and physical multi monitor switching remain unverified.
No Wayland global input capture is claimed. See `BUILD_REPORT.md` for results.
