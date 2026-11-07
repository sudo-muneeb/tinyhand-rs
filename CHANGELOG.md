# Changelog

All notable changes to TinyHand for Linux are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Dates are commit
dates. The first commit, `b5c61e2`, is version 0.1.0. Versions 0.1.1 and 0.1.2
have packages in `dist/` but no separate commit, so their changes are folded
into 0.1.3. Entries are summarised from commit history and `BUILD_REPORT.md`.

Upstream project: [nilbuild/tinyhand](https://github.com/nilbuild/tinyhand)
(macOS). Artwork, sounds and prompt are MIT licensed by Kamran Ahmed; see
[NOTICE.md](NOTICE.md).

## [Unreleased]

### Added
- `CHANGELOG.md`.
- README logo, badges, feature list, shortcut and CLI tables, and a
  Contributing section.
- README compatibility table, Fedora and Arch build dependencies (untested),
  Rust version requirement, and sound, tray and asset troubleshooting notes.

## [0.2.2] - 2026-11-03

Commits `110b852`, `4030e2f`, `2effaeb`.

### Added
- GTK 3 stylesheet (`assets/settings.css`) for the settings window, with a
  larger rework of `native_settings.rs`.
- Cinnamon regression test checks for ink colour selection and persistence.

### Changed
- The application launcher entry now runs `tinyhand-rs --toggle`, so the
  launcher and tray icon start or stop the pointer directly.
- Running `tinyhand-rs` in a terminal opens settings; **Start pointer** begins
  presenting.
- README documents `--settings` and the launcher behaviour.

## [0.2.1] - 2026-11-03

Commit `a4b5c36`.

### Changed
- Hand rendering reworked, and toolbar painting moved into
  `render/toolbar_paint.rs` with toolbar layout in `toolbar.rs`.
- Updated hand motion and ink handling.
- Packaging script and Cinnamon test updated.

## [0.2.0] - 2026-11-03

Commit `8f80c5d`.

### Added
- Native GTK 3 settings window replacing the Cairo-drawn settings panel.
- StatusNotifier (D-Bus) tray with start/stop, clear, sound, settings and quit.
  The legacy XEmbed tray is never created.
- XInput2 and XTEST based interaction mode: hold Alt (or Control, Super) to
  click, drag and scroll the app underneath. Panels, docks and popup menus
  receive normal clicks automatically.
- Custom hands: image validation and composition, a managed library under
  `${XDG_DATA_HOME:-~/.local/share}/tinyhand-rs/hands`, and
  `tinyhand-rs --make-hand input.png composed.png`.
- Single-instance control with `--present`, `--settings`, `--toggle`, `--stop`,
  `--clear` and `--quit`.
- Lifecycle and error log at `${XDG_STATE_HOME:-~/.local/state}/tinyhand-rs/tinyhand.log`,
  rotated at 1 MB on launch.
- Per-stroke fade delay (`fade_after`), with ink colour and thickness kept per
  stroke.
- GitHub Actions workflow running fmt, Clippy, tests and a release build.
- Original hand-generation prompt (`assets/hand-prompt.txt`) and full-resolution
  hand artwork.

### Changed
- Sound worker reworked: new taps interrupt old ones, the ALSA device closes
  after 30 seconds idle.
- README rewritten.

## [0.1.5] - 2026-11-03

Commit `46629e7`.

### Added
- Ink colours: 37 pointer-matched colours and eight fixed swatches, checked
  against the upstream palette by `scripts/check-upstream.py`.
- Ink can match the selected pointer's colour (`match_color` setting).

### Changed
- Settings panel and hand rendering updated for the new colour options.

## [0.1.4] - 2026-11-02

Commit `fedffa9`.

### Added
- Natural hand motion: hand and rod are separate artwork segments, with the
  fingertip kept on the cursor while the rod aims at a grip below the monitor.
  Grip follow uses a 0.20 s time constant.
- Press animation: hand compresses 6%, shadow lowers, release eases back over
  0.084 s. Laser core grows slightly on press.
- Rendering stops when motion settles.

### Fixed
- Stray lines or filled wedges between click or laser arcs and toolbar labels.
  Cairo arcs now start fresh paths and text helpers clear their paths.
- Pressing UI controls ends any previous ink gesture, and motion without a held
  button ends ink.

## [0.1.3] - 2026-11-02

Commit `3b36712`. Also covers 0.1.1 and 0.1.2.

### Added
- Original-style settings UI with General, Pointers, Appearance, Drawing, Sound
  and About pages, hand thumbnails and selectable sound tiles.
- All six upstream tap sounds with their licence.
- Custom PNG import with size and dimension checks.
- Isolated Cinnamon regression test, `scripts/test-cinnamon.py`.
- Back buffer: frames render into a 32-bit X11 pixmap and publish with one
  `XCopyArea` over the damage bounds.

### Changed
- On Cinnamon the legacy XEmbed tray is disabled. Detection uses desktop
  environment variables and the window manager's EWMH name, including
  `Mutter (Muffin)`. The old startup block and `TINYHAND_UNSAFE_CINNAMON`
  override were removed.
- Pointer and toolbar are painted above the settings surface.
- Damage rectangles are rounded outward to whole pixels.

### Fixed
- Cinnamon 6.6.9 / Muffin 6.6.3 crash in `meta_compositor_sync_stack` after
  TinyHand closed, reproduced with the XEmbed tray and avoided by disabling it.
- Pointer motion suppressed during scripted drags: the grab no longer requests
  `PointerMotionHintMask` without acknowledging hints.
- Incomplete thumbnail grids seen when the compositor sampled mid-frame
  painting.
- Cairo, pixmap and image resources are released before the X11 connection
  closes.

## [0.1.0] - 2026-11-02

Commit `b5c61e2`. Initial commit.

### Added
- Native Rust/X11 overlay with 32-bit ARGB visual and input shape.
- Pointer grab, hidden system cursor, tap pulse, laser dot and fading Cairo ink.
- Hand artworks ported from upstream (36 hands plus a laser dot), with the
  fingertip located from the image alpha channel.
- Toolbar, XDG-backed settings, autostart entry and global shortcuts
  (`Ctrl+Alt+H` toggle, `Ctrl+Alt+X` clear, `Escape` exit).
- ALSA tap sound, `.deb` and portable archive.

### Known limitations
- X11 only. Wayland global input capture is not implemented.
- Not at feature parity with macOS. Early versions lack pinned-pointer drag
  reordering, AI prompt generation, and Mac permission and update controls.
  Update checking is intentionally not included.
