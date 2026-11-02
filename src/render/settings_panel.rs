//! Cairo adaptation of upstream SettingsView.swift's sidebar, cards, and tiles.
use super::*;

impl Renderer {
    pub(super) unsafe fn settings_panel(&mut self, c: *mut Cairo, config: &Config, ui: SettingsUi) {
        cairo_save(c);
        cairo_translate(c, self.settings_x, self.settings_y);
        cairo_scale(c, self.settings_scale, self.settings_scale);
        // A quiet solid shadow avoids another window or compositor effect.
        fill(
            c,
            (-7.0, -4.0, settings::WIDTH + 14.0, settings::HEIGHT + 18.0),
            19.0,
            (0.0, 0.0, 0.0, 0.24),
        );
        rounded(c, 0.0, 0.0, settings::WIDTH, settings::HEIGHT, 13.0);
        cairo_clip(c);
        fill(
            c,
            (0.0, 0.0, settings::WIDTH, settings::HEIGHT),
            0.0,
            (0.105, 0.105, 0.11, 1.0),
        );
        fill(
            c,
            (0.0, 0.0, 180.0, settings::HEIGHT),
            0.0,
            (0.235, 0.25, 0.26, 1.0),
        );
        for (x, color) in [
            (20.0, (1.0, 0.37, 0.34)),
            (40.0, (1.0, 0.75, 0.28)),
            (60.0, (0.42, 0.46, 0.46)),
        ] {
            dot(c, x, 18.0, 5.5, color);
        }
        let tints = [
            (0.55, 0.55, 0.56),
            (0.55, 0.42, 0.95),
            (0.95, 0.45, 0.6),
            (0.96, 0.58, 0.2),
            (0.2, 0.55, 0.98),
            (0.45, 0.45, 0.46),
        ];
        for (tab, title) in settings::TABS.iter().enumerate() {
            let y = 50.0 + tab as f64 * 34.0;
            if tab == ui.tab {
                fill(c, (12.0, y, 156.0, 30.0), 6.0, (0.34, 0.355, 0.365, 1.0));
            }
            let (r, g, b) = tints[tab];
            fill(c, (20.0, y + 5.0, 20.0, 20.0), 5.0, (r, g, b, 1.0));
            if tab == 1 {
                self.thumbnail(c, 0, 21.0, y + 5.0, 18.0);
            } else {
                sidebar_icon(c, 30.0, y + 15.0, tab);
            }
            text(c, 49.0, y + 20.0, 13.0, title, false, false);
        }
        fill(c, (20.0, 451.0, 16.0, 16.0), 4.0, (0.16, 0.07, 0.2, 1.0));
        text(c, 23.0, 463.0, 10.0, "T", true, false);
        text(c, 44.0, 463.0, 12.0, "TinyHand", true, false);
        if ui.tab == 1 && ui.custom {
            self.custom_hand_page(c, config);
            if let Some(notice) = ui.notice {
                text(c, 224.0, 455.0, 11.0, notice, false, false);
            }
        } else {
            text(
                c,
                204.0,
                66.0,
                21.0,
                settings::TABS[ui.tab.min(5)],
                true,
                false,
            );
            match ui.tab {
                0 => {
                    caption(c, 100.0, "Startup");
                    card(c, 112.0, 58.0);
                    row(
                        c,
                        112.0,
                        "Open TinyHand when you log in",
                        "Start ready for your next presentation.",
                    );
                    toggle(c, 602.0, 132.0, config.startup);
                    caption(c, 185.0, "Shortcuts");
                    card(c, 196.0, 116.0);
                    for (i, (label, value, capture)) in [
                        ("Turn the pointer on or off", &config.toggle, true),
                        ("Clear the drawing", &config.clear, false),
                    ]
                    .iter()
                    .enumerate()
                    {
                        let y = 196.0 + i as f64 * 58.0;
                        row(c, y, label, "");
                        fill(
                            c,
                            (478.0, y + 14.0, 164.0, 30.0),
                            7.0,
                            (0.22, 0.22, 0.235, 1.0),
                        );
                        let label = if ui.capture == Some(*capture) {
                            "Press keys..."
                        } else {
                            value
                        };
                        text(
                            c,
                            487.0,
                            y + 33.0,
                            11.0,
                            label,
                            false,
                            ui.capture != Some(*capture),
                        );
                    }
                    caption(c, 348.0, "Clicking for real");
                    card(c, 360.0, 76.0);
                    row(
                        c,
                        360.0,
                        "Press Escape to release the pointer",
                        "Press the toggle shortcut to bring the hand back.",
                    );
                    text(
                        c,
                        220.0,
                        420.0,
                        11.0,
                        "Esc also cancels shortcut recording.",
                        false,
                        true,
                    );
                }
                1 => {
                    caption(c, 105.0, "Choose a pointer");
                    button(c, (598.0, 87.0, 58.0, 30.0), "Reset", false);
                    for index in 0..37 {
                        let rect = settings::style_rect(index);
                        let selected = index == config.style && !config.custom_enabled;
                        tile(c, rect, selected);
                        self.thumbnail(c, index, rect.0 + 5.0, rect.1 + 4.0, 34.0);
                    }
                    text(c, 204.0, 418.0, 12.0, "Your hands", true, false);
                    if config.custom_path.is_empty() {
                        text(c, 208.0, 451.0, 11.0, "No custom hands yet", false, true);
                    } else {
                        button(
                            c,
                            (204.0, 430.0, 210.0, 34.0),
                            "Use imported hand",
                            config.custom_enabled,
                        );
                    }
                    button(
                        c,
                        (430.0, 430.0, 226.0, 34.0),
                        "Make your own hand...",
                        false,
                    );
                }
                2 => {
                    caption(c, 100.0, "Pointer");
                    card(c, 112.0, 232.0);
                    row(c, 112.0, "Size", "Scale the hand to suit your screen.");
                    stepper(c, 112.0, &format!("{:.0}%", config.size * 100.0));
                    row(c, 170.0, "Shadow under the pointer", "");
                    toggle(c, 602.0, 190.0, config.shadow);
                    row(c, 228.0, "Tilt", "Rotate the hand around the fingertip.");
                    stepper(c, 228.0, &format!("{:.0} deg", config.angle.to_degrees()));
                    row(
                        c,
                        286.0,
                        "Opacity",
                        "Keep the pointer light or fully visible.",
                    );
                    stepper(c, 286.0, &format!("{:.0}%", config.opacity * 100.0));
                    text(
                        c,
                        204.0,
                        384.0,
                        12.0,
                        "Changes appear on your hand immediately.",
                        false,
                        true,
                    );
                }
                3 => {
                    caption(c, 100.0, "Ink");
                    card(c, 112.0, 174.0);
                    row(c, 112.0, "Ink colour", "");
                    for (i, color) in COLORS.iter().enumerate() {
                        let x = 378.0 + i as f64 * 34.0;
                        if i == config.color {
                            dot(c, x, 135.0, 13.5, (0.9, 0.9, 0.95));
                        }
                        dot(c, x, 135.0, 10.0, *color);
                    }
                    row(
                        c,
                        170.0,
                        "Lines fade",
                        "Clear them any time with your shortcut.",
                    );
                    stepper(c, 170.0, &format!("{:.1} s", config.fade));
                    row(
                        c,
                        228.0,
                        "Line thickness",
                        "Smooth ink for notes and gestures.",
                    );
                    stepper(c, 228.0, &format!("{:.0} px", config.thickness));
                    button(
                        c,
                        (204.0, 320.0, 452.0, 38.0),
                        "Clear the drawing now",
                        false,
                    );
                }
                4 => {
                    caption(c, 100.0, "Tap sound");
                    for (i, name) in settings::SOUNDS.iter().enumerate() {
                        let rect = settings::sound_rect(i);
                        let selected = if config.sound {
                            i == config.sound_kind
                        } else {
                            i == 0
                        };
                        tile(c, rect, selected);
                        waveform(c, rect.0 + rect.2 / 2.0, rect.1 + 24.0, i == 0, selected);
                        text(
                            c,
                            rect.0 + 12.0,
                            rect.1 + 55.0,
                            12.0,
                            name,
                            false,
                            !selected,
                        );
                    }
                    text(
                        c,
                        204.0,
                        282.0,
                        11.0,
                        "Click a sound to hear it. It plays each time the hand taps.",
                        false,
                        true,
                    );
                    card(c, 308.0, 116.0);
                    row(c, 308.0, "Volume", "");
                    stepper(c, 308.0, &format!("{:.0}%", config.volume * 100.0));
                    row(
                        c,
                        366.0,
                        "Laser dot taps too",
                        "Off by default, for quiet pointing.",
                    );
                    toggle(c, 602.0, 386.0, config.laser_sound);
                }
                _ => {
                    fill(c, (204.0, 108.0, 64.0, 64.0), 15.0, (0.20, 0.11, 0.25, 1.0));
                    text(c, 221.0, 152.0, 32.0, "T", true, false);
                    text(c, 286.0, 132.0, 21.0, "TinyHand for Linux", true, false);
                    text(
                        c,
                        286.0,
                        155.0,
                        12.0,
                        concat!("Version ", env!("CARGO_PKG_VERSION")),
                        false,
                        true,
                    );
                    text(
                        c,
                        204.0,
                        211.0,
                        14.0,
                        "A tiny hand for your screen.",
                        false,
                        false,
                    );
                    text(
                        c,
                        204.0,
                        237.0,
                        12.0,
                        "Free and open source. Built with Rust, X11 and Cairo.",
                        false,
                        true,
                    );
                    card(c, 274.0, 112.0);
                    text(
                        c,
                        220.0,
                        300.0,
                        13.0,
                        "Based on TinyHand by Kamran Ahmed",
                        true,
                        false,
                    );
                    text(
                        c,
                        220.0,
                        326.0,
                        12.0,
                        "github.com/nilbuild/tinyhand",
                        false,
                        true,
                    );
                    text(
                        c,
                        220.0,
                        359.0,
                        11.0,
                        "Original artwork, sounds and design under the MIT license.",
                        false,
                        true,
                    );
                }
            }
        }
        cairo_restore(c);
    }

    unsafe fn custom_hand_page(&mut self, c: *mut Cairo, config: &Config) {
        text(c, 204.0, 52.0, 12.0, "< Pointers", false, true);
        text(c, 204.0, 88.0, 21.0, "Make your own hand", true, false);
        for (n, y, title, detail) in [
            (
                1,
                128.0,
                "Imagine it",
                "Choose the look, material and colour of your hand.",
            ),
            (
                2,
                198.0,
                "Make the image",
                "Use your preferred image tool; save a transparent PNG.",
            ),
            (
                3,
                258.0,
                "Add it",
                "Show the back of a hand with the fingertip at the top.",
            ),
        ] {
            dot(c, 213.0, y - 4.0, 9.0, (0.40, 0.42, 0.43));
            text(c, 210.0, y, 10.0, &n.to_string(), true, false);
            text(c, 233.0, y, 13.0, title, true, false);
            text(c, 233.0, y + 20.0, 11.0, detail, false, true);
        }
        button(
            c,
            (224.0, 296.0, 412.0, 80.0),
            "Choose a PNG file...",
            false,
        );
        text(
            c,
            224.0,
            406.0,
            11.0,
            "Transparent PNG, up to 2048 x 2048 pixels.",
            false,
            true,
        );
        if !config.custom_path.is_empty() {
            text(
                c,
                224.0,
                432.0,
                12.0,
                "Your imported hand is saved and ready to use.",
                false,
                false,
            );
        }
    }

    unsafe fn thumbnail(&mut self, c: *mut Cairo, index: usize, x: f64, y: f64, size: f64) {
        if index == 36 {
            dot(c, x + size / 2.0, y + size / 2.0, 5.0, (1.0, 0.18, 0.16));
            return;
        }
        if self.previews.is_empty() {
            self.previews.resize(36, std::ptr::null_mut());
        }
        if self.previews[index].is_null() {
            let style = STYLES[index];
            let name = CString::new(
                self.asset_dir
                    .join(format!("hand_{style}.png"))
                    .to_string_lossy()
                    .as_bytes(),
            )
            .unwrap();
            self.previews[index] = cairo_image_surface_create_from_png(name.as_ptr());
        }
        let image = self.previews[index];
        if cairo_surface_status(image) != 0 {
            return;
        }
        let w = cairo_image_surface_get_width(image) as f64;
        let h = cairo_image_surface_get_height(image) as f64;
        // Crop the rod out of the selection thumbnail, as upstream does.
        cairo_save(c);
        cairo_rectangle(c, x, y, size, size);
        cairo_clip(c);
        cairo_translate(c, x, y);
        let scale = size / (h * 0.49);
        cairo_scale(c, scale, scale);
        cairo_set_source_surface(c, image, -w * 0.5 + size / scale * 0.5, 0.0);
        cairo_paint(c);
        cairo_restore(c);
    }
}

unsafe fn text(c: *mut Cairo, x: f64, y: f64, size: f64, label: &str, bold: bool, muted: bool) {
    cairo_set_source_rgba(
        c,
        if muted { 0.59 } else { 0.93 },
        if muted { 0.60 } else { 0.94 },
        if muted { 0.62 } else { 0.95 },
        1.0,
    );
    cairo_select_font_face(c, c"Sans".as_ptr(), 0, i32::from(bold));
    cairo_set_font_size(c, size);
    cairo_new_path(c);
    cairo_move_to(c, x, y);
    if let Ok(label) = CString::new(label) {
        cairo_show_text(c, label.as_ptr());
    }
    cairo_new_path(c);
}
unsafe fn fill(c: *mut Cairo, r: (f64, f64, f64, f64), radius: f64, color: (f64, f64, f64, f64)) {
    if radius > 0.0 {
        rounded(c, r.0, r.1, r.2, r.3, radius);
    } else {
        cairo_rectangle(c, r.0, r.1, r.2, r.3);
    }
    cairo_set_source_rgba(c, color.0, color.1, color.2, color.3);
    cairo_fill(c);
}
unsafe fn dot(c: *mut Cairo, x: f64, y: f64, r: f64, color: (f64, f64, f64)) {
    cairo_new_path(c);
    cairo_arc(c, x, y, r, 0.0, std::f64::consts::TAU);
    cairo_set_source_rgba(c, color.0, color.1, color.2, 1.0);
    cairo_fill(c);
}
unsafe fn caption(c: *mut Cairo, y: f64, label: &str) {
    text(c, 208.0, y, 12.0, label, true, true);
}
unsafe fn card(c: *mut Cairo, y: f64, h: f64) {
    fill(c, (204.0, y, 452.0, h), 9.0, (0.145, 0.145, 0.15, 1.0));
}
unsafe fn row(c: *mut Cairo, y: f64, title: &str, detail: &str) {
    text(c, 220.0, y + 25.0, 13.0, title, false, false);
    if !detail.is_empty() {
        text(c, 220.0, y + 43.0, 10.0, detail, false, true);
    }
}
unsafe fn button(c: *mut Cairo, r: (f64, f64, f64, f64), title: &str, selected: bool) {
    tile(c, r, selected);
    text(
        c,
        r.0 + 12.0,
        r.1 + r.3 / 2.0 + 4.0,
        12.0,
        title,
        false,
        false,
    );
}
unsafe fn tile(c: *mut Cairo, r: (f64, f64, f64, f64), selected: bool) {
    fill(
        c,
        r,
        7.0,
        if selected {
            (0.14, 0.22, 0.34, 1.0)
        } else {
            (0.145, 0.145, 0.15, 1.0)
        },
    );
    if selected {
        rounded(c, r.0, r.1, r.2, r.3, 7.0);
        cairo_set_source_rgba(c, 0.21, 0.52, 0.94, 1.0);
        cairo_set_line_width(c, 1.5);
        cairo_stroke(c);
    }
}
unsafe fn toggle(c: *mut Cairo, x: f64, y: f64, on: bool) {
    fill(
        c,
        (x, y, 38.0, 21.0),
        10.0,
        if on {
            (0.12, 0.48, 0.98, 1.0)
        } else {
            (0.34, 0.35, 0.37, 1.0)
        },
    );
    dot(
        c,
        x + if on { 27.5 } else { 10.5 },
        y + 10.5,
        8.0,
        (0.96, 0.97, 0.98),
    );
}
unsafe fn stepper(c: *mut Cairo, y: f64, value: &str) {
    button(c, (530.0, y + 15.0, 24.0, 28.0), "-", false);
    text(c, 559.0, y + 33.0, 11.0, value, false, false);
    button(c, (620.0, y + 15.0, 28.0, 28.0), "+", false);
}
unsafe fn waveform(c: *mut Cairo, x: f64, y: f64, off: bool, selected: bool) {
    cairo_set_source_rgba(
        c,
        if selected { 0.3 } else { 0.5 },
        if selected { 0.65 } else { 0.52 },
        if selected { 1.0 } else { 0.55 },
        1.0,
    );
    cairo_set_line_width(c, 2.0);
    cairo_set_line_cap(c, 1);
    for (i, h) in [4.0, 9.0, 14.0, 8.0, 4.0].iter().enumerate() {
        let px = x - 8.0 + i as f64 * 4.0;
        cairo_move_to(c, px, y - h / 2.0);
        cairo_line_to(c, px, y + h / 2.0);
    }
    if off {
        cairo_move_to(c, x - 9.0, y - 9.0);
        cairo_line_to(c, x + 9.0, y + 9.0);
    }
    cairo_stroke(c);
}

unsafe fn sidebar_icon(c: *mut Cairo, x: f64, y: f64, tab: usize) {
    cairo_set_source_rgba(c, 1.0, 1.0, 1.0, 0.95);
    cairo_set_line_width(c, 1.4);
    cairo_set_line_cap(c, 1);
    cairo_new_path(c);
    match tab {
        0 => {
            cairo_arc(c, x, y, 4.3, 0.0, std::f64::consts::TAU);
            for i in 0..8 {
                let a = i as f64 * std::f64::consts::TAU / 8.0;
                cairo_move_to(c, x + a.cos() * 4.2, y + a.sin() * 4.2);
                cairo_line_to(c, x + a.cos() * 6.6, y + a.sin() * 6.6);
            }
        }
        2 => {
            for (dx, dy) in [(0.0, 7.0), (7.0, 0.0), (0.0, -7.0), (-7.0, 0.0)] {
                cairo_move_to(c, x, y);
                cairo_line_to(c, x + dx, y + dy);
            }
            cairo_move_to(c, x + 4.0, y - 4.0);
            cairo_line_to(c, x + 7.0, y - 7.0);
        }
        3 => {
            cairo_move_to(c, x - 5.0, y + 6.0);
            cairo_line_to(c, x + 4.0, y - 6.0);
            cairo_line_to(c, x + 7.0, y - 3.5);
            cairo_line_to(c, x - 2.5, y + 8.0);
            cairo_close_path(c);
        }
        4 => {
            for (i, h) in [4.0, 9.0, 13.0, 8.0, 4.0].iter().enumerate() {
                let px = x - 6.0 + i as f64 * 3.0;
                cairo_move_to(c, px, y - h / 2.0);
                cairo_line_to(c, px, y + h / 2.0);
            }
        }
        _ => {
            cairo_arc(c, x, y, 6.5, 0.0, std::f64::consts::TAU);
            cairo_stroke(c);
            text(c, x - 1.5, y + 4.0, 11.0, "i", true, false);
            return;
        }
    }
    cairo_stroke(c);
}
