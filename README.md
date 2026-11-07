<p align="left">
  <img src="assets/icons/tinyhand-rs.svg" alt="TinyHand logo" width="128" height="128">
</p>

# TinyHand for Linux

**A cute hand pointer for presentations, screen recordings and demos on Linux.**
Taps, a laser dot and fading ink, drawn on top of your desktop.

![License: MIT](https://img.shields.io/badge/license-MIT-blue)
![Platform: Linux X11](https://img.shields.io/badge/platform-Linux%20%7C%20X11-informational)
![Rust 1.99+](https://img.shields.io/badge/rust-1.99%2B-orange)
![Wayland: not supported](https://img.shields.io/badge/Wayland-not%20supported-red)

[Features](#features) ·
[Compatibility](#compatibility) ·
[Install](#install) ·
[Usage](#usage) ·
[Build](#build-from-source) ·
[Troubleshooting](#troubleshooting) ·
[Changelog](CHANGELOG.md)

A native Rust/X11 port of [TinyHand](https://github.com/nilbuild/tinyhand), for
Linux Mint Cinnamon and other composited X11 desktops. Settings use **GTK 3**;
the transparent pointer and toolbar use **Cairo and X11**. There is no Python,
web server or browser runtime in the app.

## Features

- **37 pointers:** the original 36 hand artworks plus a laser dot, with your
  own hands on top.
- **Tap effects:** a pulse and one of six original tap sounds on every click.
- **Fading ink:** hold the left button and drag to draw. Lines fade after 3, 6
  or 10 seconds, or stay until cleared.
- **Interaction key:** hold Alt (or Control or Super) to click, drag and scroll
  the app underneath without leaving presentation mode.
- **Toolbar:** pinned hands, ink colour, mute, clear, settings and exit.
- **Global shortcuts:** toggle and clear from any window.
- **Tray and CLI:** StatusNotifier tray icon and a command line to control the
  running instance.
- **Native settings:** six GTK 3 pages with a live preview.
- **Light on resources:** idle CPU was 0.0% and RSS about 37 MiB at most in the
  author's tests.

## Compatibility

| Environment | Status |
| --- | --- |
| X11 session with a compositor (Cinnamon, XFCE, MATE, KDE X11, GNOME on Xorg) | Supported. Only Cinnamon/Muffin and a bare Xvfb were tested. |
| X11 window manager without a compositor (i3, openbox) | Needs a compositor such as picom. Without a 32-bit ARGB visual the app exits with an error. |
| Native Wayland session (GNOME, KDE, Sway, Hyprland) | **Not supported.** Presentation refuses to start. |
| XWayland inside a Wayland session | Not supported. X11 grabs cannot reach native Wayland windows. |

Why Wayland is excluded: the pointer grab, global shortcuts, input shaping and
click forwarding use X11 calls (`XGrabPointer`, `XGrabKey`, XInput2, XTEST). A
Wayland compositor does not let a client capture input across other
applications, so this needs a different design, not a patch. Settings, tray and
CLI commands may start in a Wayland session, but the pointer will not.
Check your session with `echo "$XDG_SESSION_TYPE"`; it must print `x11`.

Other requirements and caveats:

- **Distros:** prebuilt packages are Debian/Ubuntu `.deb` and x86_64 `.tar.gz`
  only. On other distros, [build from source](#build-from-source). Only x86_64
  has been built.
- **Rust:** `Cargo.toml` declares `rust-version = "1.99"`. Install a current
  toolchain with rustup; an older distro-packaged Rust will not build it.
- **Sound:** taps play through ALSA's `default` device. On PipeWire or PulseAudio
  the ALSA compatibility plugin (`pipewire-alsa` or `pulseaudio-alsa`, usually
  preinstalled) must be present, or sound is silent.
- **Tray:** the tray uses the StatusNotifier D-Bus protocol. GNOME needs the
  AppIndicator extension. Without a tray host, use the `tinyhand-rs` commands
  or the application launcher.
- **Not verified:** audio output on real hardware, tray behaviour on desktops
  other than Cinnamon, and physical multi-monitor switching.

## Install

**Debian, Ubuntu, Linux Mint (x86_64):**

```sh
sudo apt install ./tinyhand-rs_0.2.2_amd64.deb
tinyhand-rs
```

**Portable archive (x86_64):** extract `tinyhand-rs-0.2.2-linux-x86_64.tar.gz`
and run `./tinyhand-rs` from the extracted folder. The runtime libraries below
must be installed.

**Other distros:** [build from source](#build-from-source).

Runtime libraries: GTK 3/GdkPixbuf/Gio, X11, Xext, Xrender, Xinerama, Xi, Xtst,
Xfixes, Cairo and ALSA. A compositor with a 32-bit ARGB visual is required.

## Usage

Running `tinyhand-rs` in a terminal opens settings; click **Start pointer** to
present. The application launcher and tray icon start or stop the pointer
directly.

### Mouse

| Action | Result |
| --- | --- |
| Click | Tap pulse and optional sound |
| Hold left button and drag | Draw a line |
| Right or middle click | Tap without drawing |
| Hold **Alt** | Use the underlying app normally: click, drag and scroll |

The interaction key can be changed to Control or Super, or disabled. Settings
can keep the hand visible while you interact. Panels, docks and popup menus
receive ordinary clicks automatically.

### Keyboard shortcuts

| Shortcut | Action |
| --- | --- |
| **Escape** | Exit presentation mode |
| **Ctrl+Alt+H** | Toggle presentation mode globally |
| **Ctrl+Alt+X** | Clear ink |

Shortcuts can use Control, Alt, Super and Shift with letters, numbers, function
keys and supported navigation keys. Conflicting edits restore the previous
shortcuts.

### Command line

```sh
tinyhand-rs --present    # start presenting
tinyhand-rs --settings   # open settings
tinyhand-rs --toggle     # start or stop presenting
tinyhand-rs --stop       # stop presenting
tinyhand-rs --clear      # clear ink
tinyhand-rs --quit       # quit the running instance
```

The tray menu offers start/stop, clear, sound, settings and quit. TinyHand
never creates an XEmbed tray window.

### Toolbar

The toolbar sits eight logical pixels above the desktop panel. It fades away
when you draw or move away; approach the bottom of the screen to reveal it. Use
it to choose a pinned hand, change ink colour, mute taps, clear, open settings
or exit. Drag pointers to reorder them, or drag them away to remove them.
Settings offers all 37 pointers, up to eight toolbar pins, a replacement picker
when full, and a toolbar reset. Toolbar fades follow the active display refresh
rate (60 Hz when unavailable) and stop updating at rest.

## Settings

The six pages follow the original sidebar layout: General, Pointers,
Appearance, Drawing, Sound and About. They use native, scrollable, resizable,
keyboard-accessible GTK controls. Opening settings releases pointer capture.

- **Ink** matches the selected pointer by default; eight fixed colours are
  available. Each stroke keeps its own colour, thickness and fade preference.
- **Sound:** hand and laser sounds have independent on/off preferences. New
  taps interrupt old ones rather than building a queue.
- **Reset** preserves custom hands and the launch-at-login preference.
  Autostart runs in the background without activating presentation mode.

### Custom hands

**Pointers → Make your own hand** provides an idea field, the original image
prompt, copy/browser actions, image selection or file drop, preview, naming,
and an Add button.

- Use a transparent hand-only image with one upright pointing finger and a flat
  wrist, without a rod, shadow or glow.
- Image formats come from GdkPixbuf. Images must be 64–4096 pixels on each
  side and at most 32 MB.
- The app checks the shape, composes the rod and copies the result into its
  managed library, so moving the original file does not break your saved hand.
- Custom hands can be pinned, selected and deleted.

To validate and compose an image without launching the UI:

```sh
tinyhand-rs --make-hand input.png composed.png
```

### Files

| What | Location |
| --- | --- |
| Preferences | `${XDG_CONFIG_HOME:-~/.config}/tinyhand-rs/settings.conf` |
| Custom hands | `${XDG_DATA_HOME:-~/.local/share}/tinyhand-rs/hands` |
| Log | `${XDG_STATE_HOME:-~/.local/state}/tinyhand-rs/tinyhand.log` (rotates at 1 MB on launch) |

Update and release checking is intentionally not included in this version.

## Build from source

Install the development packages for your distro. The Debian/Ubuntu/Mint set
is what CI and the author use. The Fedora and Arch names are the equivalent
packages and have not been tested.

```sh
# Debian, Ubuntu, Linux Mint
sudo apt update
sudo apt install -y build-essential pkg-config libgtk-3-dev libx11-dev libxext-dev libxrender-dev libxinerama-dev libxi-dev libxtst-dev libxfixes-dev libcairo2-dev libasound2-dev

# Fedora (untested)
sudo dnf install -y gcc pkgconf-pkg-config gtk3-devel libX11-devel libXext-devel libXrender-devel libXinerama-devel libXi-devel libXtst-devel libXfixes-devel cairo-devel alsa-lib-devel

# Arch (untested)
sudo pacman -S --needed base-devel pkgconf gtk3 libx11 libxext libxrender libxinerama libxi libxtst libxfixes cairo alsa-lib
```

Then install Rust and build:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
. "$HOME/.cargo/env"
CARGO_BUILD_JOBS=1 cargo build --release
./target/release/tinyhand-rs
```

One build job limits compiler memory use on smaller machines. To build the
Debian package, portable archive and clean source archive, run
`CARGO_BUILD_JOBS=1 scripts/package.sh`.

## Contributing

Run the same checks as CI before sending changes:

```sh
rustup component add rustfmt clippy
cargo fmt --all -- --check
CARGO_BUILD_JOBS=1 cargo clippy --all-targets -- -D warnings
CARGO_BUILD_JOBS=1 cargo test --all-targets
```

The integration test runs in its own Xvfb, private D-Bus and Cinnamon session.
It never sends input to your current desktop:

```sh
sudo apt install xvfb cinnamon dbus-x11 xdotool x11-utils x11-apps imagemagick python3-pil python3-gi python3-pyatspi
python3 scripts/test-cinnamon.py --cycles 3
```

It tests native settings through accessibility actions, custom-hand import,
input capture and interaction, drawing, tray protocol, shortcuts, idle CPU and
repeated teardown. Logs and screenshots remain in the printed temporary folder.
See [BUILD_REPORT.md](BUILD_REPORT.md) for verification details and
[UPSTREAM_PARITY.md](UPSTREAM_PARITY.md) for the comparison with the original.

## Troubleshooting

**Presentation will not start.** Use an **X11 login session**. Most display
managers offer an Xorg session on the login screen (for example "Ubuntu on
Xorg" or "Plasma (X11)"). Check with `echo "$XDG_SESSION_TYPE"`.

**The pointer cannot be captured.** Close another application's popup or grab
and retry.

**The overlay is opaque.** Check that the compositor is running.

**A shortcut conflicts with another app.** Change it in General.

**Assets are not found.** Set `TINYHAND_ASSETS` to the `assets` directory.

**No sound.** Check that an ALSA `default` device exists (`aplay -L | head`) and
that your sound server provides the ALSA plugin.

**Stuck input.** Escape, normal exit and SIGINT/SIGTERM release capture, and
the X server releases it if the process dies. `tinyhand-rs --quit` stops the
running instance.

## Credits and license

TinyHand for Linux is released under the [MIT License](LICENSE).

Original TinyHand artwork, sounds and prompt are MIT licensed by Kamran Ahmed;
see [NOTICE.md](NOTICE.md) and [assets/LICENSE.upstream](assets/LICENSE.upstream).
