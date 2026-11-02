#!/usr/bin/env python3
"""Exercise the release build in a disposable X11/Cinnamon session.

Requires Xvfb, Cinnamon, dbus-run-session, xdotool, xprop, xev, xmessage,
stdbuf, Pillow, Zenity, and ImageMagick's import. Never sends input to the current display.
Logs and screenshots are retained in the printed temporary directory.
"""

import argparse
import json
import os
from pathlib import Path
import select
import shutil
import signal
import subprocess
import tempfile
import time
from PIL import Image


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cycles", type=int, default=20)
    args = parser.parse_args()
    if args.cycles < 1:
        parser.error("--cycles must be positive")
    for name in ("Xvfb", "cinnamon", "dbus-run-session", "xdotool", "xprop",
                 "xev", "xmessage", "stdbuf", "import", "zenity"):
        if not shutil.which(name):
            parser.error(f"missing dependency: {name}")
    binary = Path(__file__).resolve().parents[1] / "target/release/tinyhand-rs"
    if not binary.exists():
        parser.error("build first with cargo build --release")
    output = Path(tempfile.mkdtemp(prefix="tinyhand-cinnamon-"))
    print(f"Logs: {output}", flush=True)
    env = os.environ.copy()
    for key in ("DBUS_SESSION_BUS_ADDRESS", "SESSION_MANAGER", "WAYLAND_DISPLAY",
                "TINYHAND_UNSAFE_CINNAMON", "GDK_SCALE", "QT_SCALE_FACTOR"):
        env.pop(key, None)
    for key in ("XDG_CONFIG_HOME", "XDG_CACHE_HOME", "XDG_DATA_HOME", "XDG_RUNTIME_DIR"):
        directory = output / key
        directory.mkdir(mode=0o700)
        env[key] = str(directory)
    env.update(GSETTINGS_BACKEND="memory", LIBGL_ALWAYS_SOFTWARE="1",
               XDG_SESSION_TYPE="x11", XDG_CURRENT_DESKTOP="X-Cinnamon",
               DESKTOP_SESSION="cinnamon", TINYHAND_TEST_SESSION=str(output))
    alsa = output / "asound.conf"
    alsa.write_text("pcm.!default { type null }\n")
    env["ALSA_CONFIG_PATH"] = str(alsa)
    processes = []

    def start(command, name, **kwargs):
        with (output / f"{name}.log").open("w") as log:
            process = subprocess.Popen(command, env=env, stdout=log,
                                       stderr=subprocess.STDOUT,
                                       start_new_session=True, **kwargs)
        processes.append(process)
        return process

    def run(*command, check=True):
        result = subprocess.run(command, env=env, capture_output=True,
                                text=True, timeout=10)
        if check and result.returncode:
            raise RuntimeError(f"{command}: {result.stderr}")
        return result

    def wait_for(predicate, message, timeout=10):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            if predicate():
                return
            time.sleep(.1)
        raise RuntimeError(message)

    def visible():
        return run("xdotool", "search", "--onlyvisible", "--name",
                   "^TinyHand overlay$", check=False).returncode == 0

    def clicks():
        return (output / "input.log").read_text().count("ButtonPress event")

    def click():
        run("xdotool", "mousemove", "100", "100", "click", "1")
        time.sleep(.15)

    try:
        read_fd, write_fd = os.pipe()
        try:
            start(["Xvfb", "-displayfd", str(write_fd), "-screen", "0",
                   "1280x800x24", "-nolisten", "tcp", "-noreset"],
                  "xvfb", pass_fds=(write_fd,))
            os.close(write_fd)
            if not select.select([read_fd], [], [], 10)[0]:
                raise RuntimeError("Xvfb did not start")
            number = os.read(read_fd, 64).decode().strip()
            if not number.isdecimal():
                raise RuntimeError("Xvfb did not return a display number")
            env["DISPLAY"] = f":{number}"
        finally:
            os.close(read_fd)
        print(f"Isolated display: {env['DISPLAY']}", flush=True)
        wm = start(["dbus-run-session", "--", "cinnamon", "--sm-disable", "--x11"],
                   "cinnamon")
        wait_for(lambda: "window id #" in run("xprop", "-root",
                 "_NET_SUPPORTING_WM_CHECK").stdout, "Cinnamon did not start", 30)
        time.sleep(5)
        config = Path(env["XDG_CONFIG_HOME"]) / "tinyhand-rs/settings.conf"
        config.parent.mkdir()
        config.write_text("sound=false\n")
        probe = start(["stdbuf", "-oL", "xev", "-name", "TinyHand input probe",
                       "-geometry", "240x140+20+20", "-event", "button"], "input")
        time.sleep(1)
        click()
        assert clicks() > 0, "Input probe is not receiving baseline clicks"
        rss = []
        for cycle in range(args.cycles):
            # Missing desktop variables must not re-enable the unsafe tray.
            env["XDG_CURRENT_DESKTOP"] = "" if cycle % 2 else "X-Cinnamon"
            env["DESKTOP_SESSION"] = ""
            app = start([str(binary)], f"app-{cycle + 1}")
            wait_for(visible, "Overlay did not appear")
            before = clicks()
            click()
            assert clicks() == before, "Active overlay leaked a click"
            moves = ["xdotool", "mousemove", "300", "220", "mousedown", "1"]
            for i in range(100):
                moves += ["mousemove", str(300 + i * 6), str(220 + i % 35 * 5),
                          "sleep", "0.005"]
            moves += ["mouseup", "1"]
            run(*moves)
            if cycle == 0:
                time.sleep(.2)
                run("import", "-window", "root", str(output / "drawing.png"))
                pixels = Image.open(output / "drawing.png").convert("RGB").crop(
                    (280, 200, 850, 420))
                ink_pixels = sum(1 for r, g, b in pixels.getdata()
                                 if r > 70 and b > 110 and b > r * 1.2 and r > g * 1.3)
                assert ink_pixels > 100, f"Drag did not render ink: {ink_pixels} pixels"
                # Navigate every sidebar page and exercise the real controls.
                run("xdotool", "click", "5")
                run("xdotool", "mousemove", "770", "750", "click", "1")
                time.sleep(.2)
                for tab in range(6):
                    run("xdotool", "mousemove", "380", str(221 + tab * 34), "click", "1")
                    time.sleep(.15)
                    run("xdotool", "mousemove", "1050", "110")
                    time.sleep(.1)
                    run("import", "-window", "root", str(output / f"settings-{tab}.png"))
                    if tab == 1:
                        shot = Image.open(output / "settings-1.png").convert("RGB")
                        for index in range(37):
                            rgb = shot.getpixel((507 + index % 9 * 50, 321 + index // 9 * 52))
                            assert sum(rgb) / 3 > 30, f"Thumbnail tile {index} disappeared: {rgb}"

                # Appearance size +; the panel's origin is (300,156).
                run("xdotool", "mousemove", "380", "289", "click", "1")
                run("xdotool", "mousemove", "934", "298", "click", "1")
                wait_for(lambda: "size=0.14" in config.read_text(), "Pointer size did not persist")
                # Drawing colour, fade, and thickness.
                run("xdotool", "mousemove", "380", "323", "click", "1")
                run("xdotool", "mousemove", "712", "291", "click", "1")
                run("xdotool", "mousemove", "934", "356", "click", "1")
                run("xdotool", "mousemove", "934", "414", "click", "1")
                wait_for(lambda: all(line in config.read_text() for line in
                         ["color=1", "fade=3.5", "thickness=6"]), "Drawing settings did not persist")
                # Test all six original audio choices against ALSA's null sink.
                run("xdotool", "mousemove", "380", "357", "click", "1")
                for kind in range(1,7):
                    run("xdotool", "mousemove", str(552 + kind % 4 * 114),
                        str(302 + kind // 4 * 76), "click", "1")
                    wait_for(lambda: f"sound_kind={kind}" in config.read_text(), "Sound choice did not persist")
                run("xdotool", "mousemove", "552", "302", "click", "1")
                wait_for(lambda: "sound=false" in config.read_text(), "Sound off did not persist")
                # Pointers: choose one and open the custom hand flow.
                run("xdotool", "mousemove", "380", "255", "click", "1")
                run("xdotool", "mousemove", "576", "322", "click", "1")
                wait_for(lambda: "style=glove" in config.read_text(), "Pointer selection did not persist")
                run("xdotool", "mousemove", "850", "604", "click", "1")
                time.sleep(.15)
                run("xdotool", "mousemove", "1050", "110")
                run("import", "-window", "root", str(output / "settings-custom.png"))
                # Open the native file chooser: capture must be released.
                run("xdotool", "mousemove", "650", "486", "click", "1")
                wait_for(lambda: not visible(), "PNG chooser did not release the pointer grab")
                chooser = run("xdotool", "search", "--sync", "--onlyvisible", "--name", "Choose your TinyHand PNG").stdout.splitlines()[0]
                run("xdotool", "windowactivate", "--sync", chooser)
                run("xdotool", "key", "ctrl+l")
                fixture = str(binary.parents[2] / "assets/hand_lavender.png")
                run("xdotool", "type", "--clearmodifiers", fixture)
                run("xdotool", "key", "Return")
                time.sleep(.5)
                run("xdotool", "key", "Return")
                wait_for(visible, "PNG chooser did not return to settings")
                wait_for(lambda: "custom_enabled=true" in config.read_text(), "PNG import failed")
                # Restore an ordinary pointer for the remaining lifecycle cycles.
                run("xdotool", "mousemove", "540", "204", "click", "1")
                run("xdotool", "mousemove", "526", "322", "click", "1")
                wait_for(lambda: "custom_enabled=false" in config.read_text(), "Built-in pointer did not replace custom hand")
                time.sleep(3.3)
                ticks_before = sum(map(int, Path(f"/proc/{app.pid}/stat").read_text().split()[13:15]))
                time.sleep(2)
                ticks_after = sum(map(int, Path(f"/proc/{app.pid}/stat").read_text().split()[13:15]))
                print(f"Idle app CPU: {(ticks_after - ticks_before) / os.sysconf('SC_CLK_TCK') / 2 * 100:.1f}% of one core", flush=True)
            status = Path(f"/proc/{app.pid}/status").read_text()
            rss.append(int(status.split("VmRSS:")[1].split()[0]))
            run("xdotool", "key", "ctrl+alt+x")
            run("xdotool", "key", "Escape")
            wait_for(lambda: not visible(), "Escape did not hide overlay")
            click()
            assert clicks() > before, "Escape did not restore clicks"
            run("xdotool", "key", "ctrl+alt+h")
            wait_for(visible, "Global shortcut did not reactivate overlay")
            app.send_signal(signal.SIGINT if cycle % 2 else signal.SIGTERM)
            assert app.wait(timeout=5) == 0, "App did not exit cleanly"
            wait_for(lambda: not visible(), "Overlay remained after exit")
            before = clicks()
            click()
            assert clicks() > before, "Shutdown did not restore clicks"
            # Force delayed compositor stack reconciliation after teardown.
            for _ in range(3):
                run("xmessage", "-timeout", "1", "-buttons", "OK", "Teardown check")
                assert wm.poll() is None, "Cinnamon crashed after overlay teardown"
            log = (output / f"app-{cycle + 1}.log").read_text()
            assert "legacy tray disabled" in log, "Muffin was not detected"
            assert "X11 error" not in log, log
            assert probe.poll() is None, "Input probe exited"
            print(f"Cycle {cycle + 1}/{args.cycles}: capture, drawing, Escape, toggle, shutdown passed", flush=True)
        time.sleep(10)
        assert wm.poll() is None, "Cinnamon crashed during final observation"
        wm_log = (output / "cinnamon.log").read_text()
        assert "Adding systray: unknown" not in wm_log, "Unexpected legacy tray"
        assert "has been already disposed" not in wm_log, "Disposed compositor actor"
        summary = {"result": "PASS", "cycles": args.cycles,
                   "max_app_rss_kib": max(rss), "logs": str(output)}
        (output / "result.json").write_text(json.dumps(summary, indent=2) + "\n")
        print(json.dumps(summary), flush=True)
    finally:
        for process in reversed(processes):
            if process.poll() is None:
                os.killpg(process.pid, signal.SIGTERM)
        for process in processes:
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait(timeout=5)
        # Some desktop services daemonize into their own process groups.
        # Only stop processes carrying this test's unique inherited marker.
        marker = f"TINYHAND_TEST_SESSION={output}".encode()
        for proc in Path("/proc").iterdir():
            if proc.name.isdecimal():
                try:
                    if marker in (proc / "environ").read_bytes().split(b"\0"):
                        os.kill(int(proc.name), signal.SIGTERM)
                except (PermissionError, FileNotFoundError, ProcessLookupError):
                    pass
        print(f"Isolated processes stopped. Logs: {output}", flush=True)


if __name__ == "__main__":
    main()
