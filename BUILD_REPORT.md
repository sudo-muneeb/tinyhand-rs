# Build and verification report

Updated on 2 November 2026 for Linux x86_64 with Rust 1.99.1.
Original reference: `nilbuild/tinyhand` at commit `f243657`.

## Version 0.1.4 hand motion and click artifacts

The previous working version was committed as `3b36712` before these changes.
Its commit is unsigned because the configured SSH signer waited on the key
agent; no signing configuration was changed.

The supplied video's tap sequence and upstream `ArmView.swift` were reviewed
again. The hand and rod are now drawn as separate artwork segments. Measured
tip/wrist/rod geometry keeps the fingertip on the cursor while the rod aims at
a grip below the current monitor. Grip follow uses time-based easing with a
0.20 second time constant. A press compresses the hand by 6% and lowers its
shadow, then release eases back with a 0.084 second time constant. The laser
core grows slightly on press. Idle rendering stops once motion settles;
animation uses the existing approximately 60 FPS frame limit.

Cairo click/laser arcs now start fresh paths, and text helpers clear their
current paths. This prevents a circle from connecting to the previous toolbar
label and producing a stray line or filled wedge. UI presses also end any
previous ink gesture, and motion without a held drawing button ends ink.

Eleven automated tests passed, including press/release settling, monitor-aware
grip geometry, and checking transparency between a tap and the toolbar.
Rest/press/release PNGs are written to `/tmp/tinyhand-tap-*.png` by the rendering
test for visual comparison. The compositor regression also compares a remote
screen region before and during a laser tap to detect stray lines.
Three isolated Cinnamon cycles passed with the new motion, settings workflow,
PNG chooser, and screenshot comparison. Idle app CPU returned to 0.0% in the
short sample after settling; maximum sampled RSS was 37,272 KiB. Logs and
screenshots are under `/tmp/tinyhand-cinnamon-8_r5hcgp/`. The live Cinnamon
process remained unchanged; these new motion checks ran on a separate display.

## Version 0.1.3 settings update

The upstream repository was downloaded to
`/home/minty/data/tinyhand-original` and reviewed at commit
`c5fb36b7c4d7c87480958872a724b30a52427a47`. `SettingsView.swift`,
`SettingsWindow.swift`, `ArmView.swift`, and `TapSound.swift` were used as
references alongside the supplied 84 second Mac demonstration video.

The Rust UI now has a dark sidebar with General, Pointers, Appearance,
Drawing, Sound, and About pages, coloured sidebar icons, grouped cards,
hand thumbnails, and selectable sound tiles. Existing preferences remain
functional; size, shadow, tilt, opacity, drawing parameters, sound selection,
laser sound, and custom PNG choices persist. All six upstream tap samples are
included with their original license. Thumbnail decoding is lazy and cached.

The hand and tap pulse are painted after the toolbar and settings, fixing
the pointer being covered by buttons. Rendering checks verify pointer pixels
above both surfaces and write screenshots for every page. The settings panel
scales to its monitor and avoids the toolbar on smaller screens. Activation
queries the actual pointer position before drawing the first frame.
Damage rectangles are rounded outward to whole pixels before clipping,
preventing partial-alpha seams during fading ink and pointer movement. A
pixel-stability check verifies the settings surface stays unchanged while
ink fades underneath it.
Frames are now rendered into a 32 bit X11 pixmap and published with one
`XCopyArea` over the damage bounds. This fixes incomplete thumbnail grids
visible when the compositor sampled direct window painting mid-frame.
Cairo surfaces, image caches, the graphics context, and the pixmap are
released before closing the X11 connection. The GUI test checks every
pointer tile remains present after pointer movement.

Custom PNG import uses an asynchronous native `zenity` chooser. Input capture
is released while it is open. PNG dimensions and file size are checked before
decoding; failed imports retain the previous pointer. This is a Linux
adaptation, not complete Mac feature parity: upstream pinned-pointer drag
reordering, AI prompt generation, and Mac-specific permission/update controls
are not implemented.

Verification: formatting, Clippy with warnings denied, nine automated tests,
and release build passed. The expanded isolated Cinnamon test passed six
cycles and exercised every page, appearance/drawing persistence, all six
sounds through an ALSA null sink, native file selection, custom hand import,
input restoration and shutdown. Maximum sampled app RSS was 28,340 KiB
including decoded hand thumbnails; idle CPU was 0.0% in a short sample.
These GUI logs and screenshots are under `/tmp/tinyhand-cinnamon-sqr603_3/`.
After adding the back buffer, three further compositor cycles passed,
including checks that all 37 pointer tiles are visible after movement. The
completed-grid screenshots and logs are under
`/tmp/tinyhand-cinnamon-cdaxvq26/`; maximum sampled app RSS was 28,000 KiB and
the short idle sample was 0.0% of one core.
Physical audio output was not verified. The live desktop test below refers
to version 0.1.2; the new settings workflow was tested in isolated Cinnamon.

## Implemented

The native X11 process creates a transparent 32 bit ARGB, input shaped overlay.
In presentation mode it takes an X11 pointer grab, hides the normal cursor,
consumes mouse press and release, and tracks root coordinates. A matching
release occurs on exit, signal handling and teardown; X11 also releases grabs
when the connection dies. The overlay renders 36 downsampled original hand
styles and a laser dot, click pulse, Cairo ink strokes with configurable fade,
and a toolbar. The hand tip is located from each asset's alpha pixels for
alignment with the cursor. A single ALSA worker plays the original tap sample.

Version 0.1.2 starts on Cinnamon with its legacy XEmbed tray disabled. Detection
checks desktop environment variables and the X11 window manager's EWMH name,
including `Mutter (Muffin)`. Other X11 desktops retain the tray path. The old
startup block and `TINYHAND_UNSAFE_CINNAMON` override have been removed.

The redraw loop batches X11 events, limits motion frames to about 60 per
second, and clips Cairo updates to changed pointer, ink and toolbar regions.
Fade animation runs only while released strokes or a tap pulse need updates.
Long drags and queued sound taps are bounded.

Pointer grabs now request ordinary motion events. The former mask included
`PointerMotionHintMask` without acknowledging hints with `XQueryPointer`, which
suppressed movement during scripted drags. Screenshots showed no ink before
this correction and a complete visible path after it.

Settings persist under XDG config, and an XDG autostart entry can be enabled.
An XEmbed tray icon docks where supported. Keyboard shortcuts and Escape are
registered on the X11 root. Xinerama monitor geometry places the toolbar on
the monitor with the pointer. A `.deb` and portable archive include assets,
licenses, desktop entry, and the release executable.

## Checks completed

* `cargo fmt --all -- --check`: passed.
* `cargo clippy --all-targets -- -D warnings`: passed.
* `cargo test`: nine tests passed, none failed. They cover tap versus drag,
  pointer coordinates, fade and clear, settings persistence and validation,
  asset count, bounded long drags, Cinnamon/Muffin name detection, and transparent
  Cairo rendering with clipped pointer movement, pointer layering over buttons,
  all settings pages, and decoding of the six original audio samples.
* `cargo build --release`: passed; stripped x86_64 ELF executable built.
* `ldd`: all shared libraries resolved in the build environment.
* Headless rendering: verified transparent background and nontransparent
  hand, toolbar and ink pixels; inspected the output image visually.
* Xvfb at 1920×1080 without a desktop manager: the overlay appeared, the pointer grab kept clicks from
  an underlying X11 event window, and `Ctrl+Alt+H` released the grab so the
  next click reached that window. Terminating the app removed the overlay and
  restored normal clicks. Idle use was about 12 MB RSS and below 1% CPU in
  this short smoke test; a 150 event drag completed without a stall.
* Version 0.1.2 with isolated Cinnamon/Muffin at 1280×800: all 20 regression
  cycles passed, including visible ink verification. No compositor crash,
  disposed actor warning, or app X11 error occurred. Maximum sampled TinyHand
  RSS was 10,740 KiB; idle CPU was 0.0% of one core over a two second sample
  after animation ended. These measurements exclude the software-rendered
  Cinnamon test process. Full logs, screenshots, and JSON results are under
  `/tmp/tinyhand-cinnamon-o50hpbgx/` on the test machine.
* Live Cinnamon X11 desktop: the 0.1.2 overlay ran for about 16 seconds with
  a 30 second external watchdog and temporary muted settings. Click capture,
  drag input, Escape, normal click restoration, global reactivation, and
  process shutdown passed. TinyHand sampled at 12,368 KiB RSS and 0.2% CPU.
  Five subsequent window creation/destruction cycles and 30 more seconds of
  observation caused no fallback or crash. Cinnamon PID 1841 remained
  unchanged; no TinyHand process or pointer grab remained. Logs and JSON are
  under `/tmp/tinyhand-live-b_79f4l5/`. This is a short live smoke test, not a
  long duration stability guarantee.

## Cinnamon crash investigation

Two Cinnamon 6.6.9 / Muffin 6.6.3 processes dumped core in
`meta_compositor_sync_stack`. GDB showed a null `MetaWindow` returned for a
window actor that Cinnamon had already logged as disposed after TinyHand
closed. The XEmbed tray appeared in the session log as `unknown (1x1px)`.
The failing legacy tray path was then reproduced on a disposable Xvfb display
with a private D-Bus session, temporary desktop configuration, and software GL.
Enabling the tray crashed the isolated Cinnamon process after the first app
shutdown. Core dump PID 33892 at 20:58 on 2 November has the identical
`meta_compositor_sync_stack` / `meta_stack_tracker_sync_stack` stack and offset
`libmuffin.so.0 + 0x8b700`. The live Cinnamon process PID 1841 was unaffected.
With the tray disabled, five initial lifecycle cycles and another fifteen
cycles passed before the drawing test was strengthened. This establishes a
reproducible workaround for this tray-triggered crash; it does not repair
Muffin itself.

`scripts/test-cinnamon.py` is the repeatable regression check. It verifies
visible ink pixels, underlying click suppression, Escape and shutdown click
restoration, global reactivation, pointer style changes, settings persistence,
SIGINT/SIGTERM cleanup, repeated window stack changes after exit, and absence
of disposed-actor diagnostics. Alternate launches omit desktop variables to
exercise detection of the actual Muffin window manager. Logs, screenshots,
and a JSON result remain in a unique temporary directory.

## Desktop verification still required

Audio output, arbitrary shortcut collisions, tray behavior on other desktops,
and physical multi monitor switching were not end to end verified. Wayland
global input capture is not implemented.
