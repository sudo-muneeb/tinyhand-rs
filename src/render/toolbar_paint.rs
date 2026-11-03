//! Cached neutral HUD chrome and vector controls, adapted from Toolbar.swift.
use super::*;
use crate::toolbar::{self, Item};
impl Renderer {
    pub(super) unsafe fn paint_toolbar_cache(
        &mut self,
        c: *mut Cairo,
        config: &Config,
        page: usize,
    ) {
        let rebuild = self.toolbar_cache.as_ref().is_none_or(|cache| {
            cache.config != *config
                || cache.page != page
                || cache.hint != self.hint
                || cache.hover != self.hover
                || cache.scale != self.ui_scale
                || cache.width != self.toolbar_width
        });
        if rebuild {
            let w = ((self.toolbar_width.max(366.0) + 16.0) * self.ui_scale).ceil() as i32;
            let h = (138.0 * self.ui_scale).ceil() as i32;
            let image = cairo_image_surface_create(0, w, h);
            let context = cairo_create(image);
            cairo_scale(context, self.ui_scale, self.ui_scale);
            cairo_translate(context, 8.0, 76.0);
            self.toolbar(context, config, page);
            cairo_destroy(context);
            self.toolbar_cache = Some(ToolbarCache {
                image,
                config: config.clone(),
                page,
                hint: self.hint,
                hover: self.hover.clone(),
                scale: self.ui_scale,
                width: self.toolbar_width,
            });
        }
        let cache = self.toolbar_cache.as_ref().unwrap();
        cairo_set_source_surface(
            c,
            cache.image,
            self.bar_x - 8.0 * self.ui_scale,
            self.bar_y - 76.0 * self.ui_scale,
        );
        cairo_paint_with_alpha(c, self.toolbar_alpha);
    }
    unsafe fn toolbar(&mut self, c: *mut Cairo, config: &Config, page: usize) {
        glass(c, 0.0, 0.0, self.toolbar_width, 54.0, 16.0);
        let selected = config.selected_id();
        let items = toolbar::items(config);
        let mut separator = false;
        for (index, item) in items.iter().enumerate() {
            let x = 6.0 + index as f64 * 44.0;
            if !matches!(item, Item::Pointer(_)) && !separator {
                separator = true;
                cairo_new_path(c);
                cairo_set_source_rgba(c, 1.0, 1.0, 1.0, 0.18);
                cairo_set_line_width(c, 1.0);
                cairo_move_to(c, x - 2.0, 15.0);
                cairo_line_to(c, x - 2.0, 39.0);
                cairo_stroke(c);
            }
            let active = matches!(item,Item::Pointer(id) if *id==selected)
                || matches!(item, Item::Ink) && page > 0;
            if active || self.hover.as_ref() == Some(item) {
                rounded(c, x + 2.0, 7.0, 40.0, 40.0, 9.0);
                cairo_set_source_rgba(c, 1.0, 1.0, 1.0, if active { 0.18 } else { 0.08 });
                cairo_fill(c);
            }
            match item {
                Item::Pointer(id) => {
                    if let Some(style) = STYLES.iter().position(|s| *s == id) {
                        self.thumbnail(c, style, x + 5.0, 10.0, 32.0);
                    } else {
                        self.custom_thumbnail(c, id, x + 5.0, 10.0, 32.0);
                    }
                }
                Item::Ink => dot(c, x + 22.0, 27.0, self.ink_color(config), false),
                _ => icon(c, x + 22.0, 27.0, item, config.tap_sound_enabled()),
            }
        }
        if page > 0 {
            glass(c, 0.0, -62.0, 366.0, 54.0, 14.0);
            for i in 0..9 {
                let x = 23.0 + i as f64 * 40.0;
                let selected = if i == 0 {
                    config.match_color
                } else {
                    !config.match_color && config.color == colors::SWATCH_ORDER[i - 1]
                };
                if selected {
                    rounded(c, x - 17.0, -52.0, 34.0, 34.0, 8.0);
                    cairo_set_source_rgba(c, 1.0, 1.0, 1.0, 0.20);
                    cairo_fill(c);
                }
                let colour = if i == 0 {
                    self.ink_color(config)
                } else {
                    SWATCHES[colors::SWATCH_ORDER[i - 1]]
                };
                dot(c, x, -35.0, colour, i == 0);
            }
        } else if self.hint {
            glass(c, 0.0, -50.0, self.toolbar_width.max(350.0), 38.0, 12.0);
            set_text(
                c,
                12.0,
                -26.0,
                12.0,
                &format!(
                    "Drag to draw · Hold {} to interact · Esc to exit",
                    config.interact_key
                ),
            );
        }
    }
}
unsafe fn glass(c: *mut Cairo, x: f64, y: f64, w: f64, h: f64, r: f64) {
    for (spread, alpha) in [(5.0, 0.035), (3.0, 0.07), (1.0, 0.13)] {
        rounded(
            c,
            x - spread,
            y - spread + 2.0,
            w + 2.0 * spread,
            h + 2.0 * spread,
            r + spread,
        );
        cairo_set_source_rgba(c, 0.0, 0.0, 0.0, alpha);
        cairo_fill(c);
    }
    rounded(c, x, y, w, h, r);
    cairo_set_source_rgba(c, 0.07, 0.07, 0.07, 0.94);
    cairo_fill(c);
    rounded(c, x + 0.5, y + 0.5, w - 1.0, h - 1.0, r - 0.5);
    cairo_set_source_rgba(c, 1.0, 1.0, 1.0, 0.12);
    cairo_set_line_width(c, 1.0);
    cairo_stroke(c);
}
unsafe fn dot(c: *mut Cairo, x: f64, y: f64, colour: Rgb, rainbow: bool) {
    cairo_new_path(c);
    cairo_arc(c, x, y + 0.7, 10.0, 0.0, std::f64::consts::TAU);
    cairo_set_source_rgba(c, 0.0, 0.0, 0.0, 0.35);
    cairo_fill(c);
    if rainbow {
        for (i, colour) in [
            colors::RED,
            SWATCHES[2],
            SWATCHES[5],
            SWATCHES[3],
            SWATCHES[4],
            SWATCHES[0],
        ]
        .iter()
        .enumerate()
        {
            let a = i as f64 * std::f64::consts::TAU / 6.0;
            cairo_new_path(c);
            cairo_move_to(c, x, y);
            cairo_arc(c, x, y, 8.5, a, a + std::f64::consts::TAU / 6.0 + 0.01);
            cairo_close_path(c);
            cairo_set_source_rgba(c, colour.0, colour.1, colour.2, 1.0);
            cairo_fill(c);
        }
    } else {
        cairo_new_path(c);
        cairo_arc(c, x, y, 8.5, 0.0, std::f64::consts::TAU);
        cairo_set_source_rgba(c, colour.0, colour.1, colour.2, 1.0);
        cairo_fill(c);
    }
    cairo_save(c);
    cairo_translate(c, x - 2.0, y - 3.5);
    cairo_scale(c, 1.5, 0.8);
    cairo_new_path(c);
    cairo_arc(c, 0.0, 0.0, 2.8, 0.0, std::f64::consts::TAU);
    cairo_set_source_rgba(c, 1.0, 1.0, 1.0, 0.35);
    cairo_fill(c);
    cairo_restore(c);
}
unsafe fn icon(c: *mut Cairo, x: f64, y: f64, item: &Item, sound: bool) {
    cairo_save(c);
    cairo_translate(c, x, y);
    cairo_new_path(c);
    cairo_set_line_width(c, 1.8);
    cairo_set_line_cap(c, 1);
    cairo_set_line_join(c, 1);
    cairo_set_source_rgba(
        c,
        1.0,
        1.0,
        1.0,
        if matches!(item, Item::Sound) && !sound {
            0.45
        } else {
            0.85
        },
    );
    match item {
        Item::Sound => {
            cairo_move_to(c, -9.0, -3.5);
            cairo_line_to(c, -5.0, -3.5);
            cairo_line_to(c, 0.0, -8.0);
            cairo_line_to(c, 0.0, 8.0);
            cairo_line_to(c, -5.0, 3.5);
            cairo_line_to(c, -9.0, 3.5);
            cairo_close_path(c);
            cairo_fill(c);
            if sound {
                for r in [5.0, 9.0] {
                    cairo_new_path(c);
                    cairo_arc(c, 0.0, 0.0, r, -0.75, 0.75);
                    cairo_stroke(c);
                }
            } else {
                cairo_move_to(c, 4.0, -3.0);
                cairo_line_to(c, 10.0, 3.0);
                cairo_move_to(c, 10.0, -3.0);
                cairo_line_to(c, 4.0, 3.0);
                cairo_stroke(c);
            }
        }
        Item::Clear => {
            cairo_move_to(c, -9.0, 2.0);
            cairo_line_to(c, 1.0, -8.0);
            cairo_line_to(c, 9.0, 0.0);
            cairo_line_to(c, 1.0, 8.0);
            cairo_line_to(c, -3.0, 8.0);
            cairo_close_path(c);
            cairo_move_to(c, -3.0, -4.0);
            cairo_line_to(c, 5.0, 4.0);
            cairo_move_to(c, 2.0, 8.0);
            cairo_line_to(c, 10.0, 8.0);
            cairo_stroke(c);
        }
        Item::Settings => {
            for i in 0..8 {
                let a = i as f64 * std::f64::consts::TAU / 8.0;
                cairo_move_to(c, 6.5 * a.cos(), 6.5 * a.sin());
                cairo_line_to(c, 9.0 * a.cos(), 9.0 * a.sin());
            }
            cairo_stroke(c);
            cairo_new_path(c);
            cairo_arc(c, 0.0, 0.0, 6.5, 0.0, std::f64::consts::TAU);
            cairo_stroke(c);
            cairo_arc(c, 0.0, 0.0, 2.4, 0.0, std::f64::consts::TAU);
            cairo_stroke(c);
        }
        Item::Exit => {
            cairo_move_to(c, -5.0, -5.0);
            cairo_line_to(c, 5.0, 5.0);
            cairo_move_to(c, 5.0, -5.0);
            cairo_line_to(c, -5.0, 5.0);
            cairo_stroke(c);
        }
        _ => {}
    }
    cairo_restore(c);
}
