# TinyHand for Linux

A native Rust/X11 port of [TinyHand](https://github.com/nilbuild/tinyhand), for
Linux Mint Cinnamon and other composited X11 desktops. It includes the original
36 hand artworks, laser pointer, ink colours and six tap sounds. Settings use
**GTK 3**; the transparent pointer and toolbar use **Cairo and X11**.

## Install and run

Install the Debian package, or extract the portable archive and run its binary:

```sh
sudo apt install ./tinyhand-rs_0.2.2_amd64.deb
tinyhand-rs
```

The app opens settings without capturing your pointer. Click **Start pointer**
to present. Click to tap; hold the left button and drag to draw. Right and middle
buttons tap without drawing. Hold **Alt** to use the underlying app normally,
including clicking, dragging and scrolling. The interaction key can be changed
to Control or Super, or disabled. Settings can keep the hand visible while you
interact. Panels, docks and popup menus receive ordinary clicks automatically.

**Escape** exits presentation mode. **Ctrl+Alt+H** toggles it globally;
**Ctrl+Alt+X** clears ink. Shortcuts can use Control, Alt, Super and Shift with
letters, numbers, function keys and supported navigation keys. Conflicting edits
restore the previous shortcuts.

The toolbar sits eight logical pixels above the desktop panel. It fades away
when you draw or move away; approach the bottom of the screen to reveal it.
Choose a pinned hand, change ink colour, mute taps, clear, open settings or exit.
Toolbar fades follow the active display refresh rate (60 Hz when unavailable);
cached artwork and controls keep animation work small and stop updating at rest.
Drag pointers to reorder them, or drag them away to remove them. Settings offers
all 37 pointers, up to eight toolbar pins, a replacement picker when full, and a toolbar reset.

The modern StatusNotifier tray provides start/stop, clear, sound, settings and
quit actions on desktops with a compatible host. TinyHand never creates an
XEmbed tray window. You can also control the running instance from a terminal:

```sh
tinyhand-rs --present
tinyhand-rs --settings
tinyhand-rs --toggle
tinyhand-rs --stop
tinyhand-rs --clear
tinyhand-rs --quit
```

## Settings and custom hands

The six settings pages follow the original sidebar layout: General, Pointers,
Appearance, Drawing, Sound and About. They use native, scrollable, resizable,
keyboard-accessible GTK controls. Appearance includes a live preview, pointer
tiles show their names and toolbar badges, and slider values use readable units.
Opening settings releases pointer capture.

Ink matches the selected pointer by default; eight fixed colours are available.
Each stroke keeps its own colour, thickness and fade preference. Choose a delay
of 3, 6 or 10 seconds, followed by a short fade, or keep lines until cleared.
Hand and laser sounds have independent on/off preferences. New taps interrupt
old ones rather than building a queue.

**Pointers → Make your own hand** provides an idea field, the original image
prompt, copy/browser actions, image selection or file drop, preview, naming,
and an Add button. Use a transparent hand-only image with one upright pointing
finger and a flat wrist, without a rod, shadow or glow. Supported image formats
come from GdkPixbuf. Images must be 64–4096 pixels on each side and at most
32 MB. The app checks the shape, composes the rod and copies the result into
its managed library; moving the original file does not break your saved hand.
Custom hands can be pinned, selected and deleted.

Preferences live in `${XDG_CONFIG_HOME:-~/.config}/tinyhand-rs/settings.conf`.
Custom hands live in `${XDG_DATA_HOME:-~/.local/share}/tinyhand-rs/hands`.
Lifecycle/error logs live in
`${XDG_STATE_HOME:-~/.local/state}/tinyhand-rs/tinyhand.log` and rotate at 1 MB
on launch. Reset preserves custom hands and the launch-at-login preference.
Autostart runs in the background without activating presentation mode.

Update/release checking is intentionally not included in this version.

## Build and verify

On Linux Mint or Ubuntu:

```sh
sudo apt update
sudo apt install -y build-essential pkg-config libgtk-3-dev libx11-dev libxext-dev libxrender-dev libxinerama-dev libxi-dev libxtst-dev libxfixes-dev libcairo2-dev libasound2-dev
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
. "$HOME/.cargo/env"
rustup component add rustfmt clippy
cargo fmt --all -- --check
CARGO_BUILD_JOBS=1 cargo clippy --all-targets -- -D warnings
CARGO_BUILD_JOBS=1 cargo test --all-targets
CARGO_BUILD_JOBS=1 cargo build --release
./target/release/tinyhand-rs
```

One build job limits compiler memory use on smaller machines. To build the
Debian package, portable archive and clean source archive, run
`CARGO_BUILD_JOBS=1 scripts/package.sh`.

Runtime libraries: GTK 3/GdkPixbuf/Gio, X11, Xext, Xrender, Xinerama, Xi, Xtst,
Xfixes, Cairo and ALSA. A compositor with a 32-bit ARGB visual is required.
There is no Python, web server or browser runtime in the app.

The integration test runs in its own Xvfb, private D-Bus and Cinnamon session.
It never sends input to your current desktop:

```sh
sudo apt install xvfb cinnamon dbus-x11 xdotool x11-utils x11-apps imagemagick python3-pil python3-gi python3-pyatspi
python3 scripts/test-cinnamon.py --cycles 3
```

It tests native settings through accessibility actions, custom-hand import,
input capture and interaction, drawing, tray protocol, shortcuts, idle CPU and
repeated teardown. Logs and screenshots remain in the printed temporary folder.
For image validation/composition without launching the UI:

```sh
tinyhand-rs --make-hand input.png composed.png
```

## Troubleshooting

Use an **X11 login session**. Wayland presentation is unsupported; an XWayland
window cannot capture input across native Wayland applications. Check with
`echo "$XDG_SESSION_TYPE"`.

If a shortcut conflicts with another app, change it in General. If presentation
cannot capture the pointer, close another application's popup/grab and retry.
If the overlay becomes opaque, check that the compositor is running. Escape,
normal exit and SIGINT/SIGTERM release capture. The X server also releases it
if the process dies. `tinyhand-rs --quit` stops the running instance.

Original TinyHand artwork, sounds and prompt are MIT licensed by Kamran Ahmed;
see [NOTICE.md](NOTICE.md) and [assets/LICENSE.upstream](assets/LICENSE.upstream).
