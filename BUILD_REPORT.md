# Build and verification report

Built on 1 November 2026 for Linux x86_64 with Rust 1.75 on Ubuntu 24.04.
Original reference: `nilbuild/tinyhand` at commit `f243657`.

## Implemented

The native X11 process creates a transparent 32 bit ARGB, input shaped overlay.
In presentation mode it takes an X11 pointer grab, hides the normal cursor,
consumes mouse press and release, and tracks root coordinates. A matching
release occurs on exit, signal handling and teardown; X11 also releases grabs
when the connection dies. The overlay renders 36 downsampled original hand
styles and a laser dot, click pulse, Cairo ink strokes with configurable fade,
and a toolbar. The hand tip is located from each asset's alpha pixels for
alignment with the cursor. A single ALSA worker plays the original tap sample.

Settings persist under XDG config, and an XDG autostart entry can be enabled.
An XEmbed tray icon docks where supported. Keyboard shortcuts and Escape are
registered on the X11 root. Xinerama monitor geometry places the toolbar on
the monitor with the pointer. A `.deb` and portable archive include assets,
licenses, desktop entry, and the release executable.

## Checks completed

* `cargo fmt --all -- --check`: passed.
* `cargo clippy --all-targets -- -D warnings`: passed.
* `cargo test`: six tests passed, none failed. They cover tap versus drag,
  pointer coordinates, fade and clear, settings persistence and validation,
  asset count, and transparent Cairo rendering.
* `cargo build --release`: passed; stripped x86_64 ELF executable built.
* `ldd`: all shared libraries resolved in the build environment.
* Headless rendering: verified transparent background and nontransparent
  hand, toolbar and ink pixels; inspected the output image visually.
* Running without `DISPLAY`: exits with a meaningful diagnostic.

## Desktop verification still required

Xvfb started but could not open any local listening socket because the remote
process sandbox rejects socket creation. An attempted escalation to run the
virtual display outside that sandbox was automatically denied. Consequently
no GUI process could connect to it. Pointer grabs, XEmbed tray docking,
Cinnamon compositor behavior, audio output, shortcut collisions and physical
multi monitor switching were not end to end verified. See the README's
six step verification procedure on Linux Mint X11. Wayland global input
capture is not implemented.
