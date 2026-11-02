//! Segment the artwork and stretch its rod toward the off-screen grip,
//! following the geometry used by upstream ArmView.swift and HandAsset.
use super::*;

pub(super) unsafe fn measure_hand(image: *mut Surface) -> HandGeometry {
    cairo_surface_flush(image);
    let w = cairo_image_surface_get_width(image) as usize;
    let h = cairo_image_surface_get_height(image) as usize;
    let stride = cairo_image_surface_get_stride(image) as usize;
    let data = cairo_image_surface_get_data(image);
    if data.is_null() || w == 0 || h == 0 {
        return HandGeometry::default();
    }
    let counts = (0..h)
        .map(|y| {
            (0..w)
                .filter(|x| *data.add(y * stride + x * 4 + 3) > 128)
                .count()
        })
        .collect::<Vec<_>>();
    let top = counts.iter().position(|n| *n > 0).unwrap_or(0);
    let end = (top + (h as f64 * 0.45) as usize).min(h).max(top + 1);
    let max_width = *counts[top..end].iter().max().unwrap_or(&1) as f64;
    let fist = (top..h)
        .find(|y| counts[*y] as f64 > max_width * 0.6)
        .unwrap_or(top);
    let wrist = (fist..h)
        .find(|y| (counts[*y] as f64) < max_width * 0.35)
        .unwrap_or(h - 1);
    let rod_width = counts[(wrist + h / 50).min(h - 1)];
    let last = counts.iter().rposition(|n| *n > 0).unwrap_or(h - 1);
    let rod_end = ((wrist + h / 33).min(h - 1)..h)
        .find(|y| counts[*y] as f64 > 1.8 * rod_width as f64)
        .unwrap_or(last);
    let center = |y: usize| {
        let first = (0..w).find(|x| *data.add(y * stride + x * 4 + 3) > 128);
        let last = (0..w).rfind(|x| *data.add(y * stride + x * 4 + 3) > 128);
        match (first, last) {
            (Some(a), Some(b)) => (a + b) as f64 / 2.0,
            _ => w as f64 / 2.0,
        }
    };
    HandGeometry {
        width: w as f64,
        top: (top as f64 - 1.0).max(0.0),
        wrist: wrist.max(top + 1) as f64,
        rod_end: rod_end as f64,
        tip: Point {
            x: center(top),
            y: top as f64,
        },
        rod_x: center((wrist + rod_end) / 2),
    }
}

impl Renderer {
    pub(super) fn hand_bounds(&self, config: &Config, cursor: Point, pose: HandPose) -> Rect {
        if (config.style == 36 && !config.custom_enabled) || self.hand.is_null() {
            return Rect::around(cursor, 36.0);
        }
        let a = self.geometry;
        let (sin, cos) = pose.angle.sin_cos();
        let left = -a.tip.x * pose.scale;
        let right = (a.width - a.tip.x) * pose.scale;
        let top = (a.top - a.tip.y) * pose.scale;
        let bottom = pose.rod_start + pose.rod_length;
        let mut bounds = Rect::around(cursor, 36.0);
        for (x, y) in [(left, top), (right, top), (left, bottom), (right, bottom)] {
            let p = Point {
                x: cursor.x + x * cos - y * sin,
                y: cursor.y + x * sin + y * cos,
            };
            bounds = bounds.union(Rect::around(p, 8.0));
        }
        if config.shadow {
            bounds = bounds.union(Rect {
                x: bounds.x + pose.lift * 0.6 - 4.0,
                y: bounds.y + pose.lift - 4.0,
                w: bounds.w + 8.0,
                h: bounds.h + 8.0,
            });
        }
        bounds
    }

    pub(super) unsafe fn paint_hand(
        &self,
        c: *mut Cairo,
        config: &Config,
        cursor: Point,
        pose: HandPose,
    ) {
        if config.shadow {
            for spread in [-3.0, 0.0, 3.0] {
                cairo_save(c);
                cairo_translate(
                    c,
                    cursor.x + pose.lift * 0.6 + spread,
                    cursor.y + pose.lift + spread,
                );
                cairo_rotate(c, pose.angle);
                self.hand_parts(c, pose, config.opacity * 0.07, true);
                cairo_restore(c);
            }
        }
        cairo_save(c);
        cairo_translate(c, cursor.x, cursor.y);
        cairo_rotate(c, pose.angle);
        self.hand_parts(c, pose, config.opacity, false);
        cairo_restore(c);
    }

    unsafe fn hand_parts(&self, c: *mut Cairo, pose: HandPose, opacity: f64, mask: bool) {
        let a = self.geometry;
        if a.rod_end > a.wrist + 2.0 {
            cairo_save(c);
            cairo_translate(c, 0.0, pose.rod_start);
            cairo_scale(c, pose.scale, pose.rod_length / (a.rod_end - a.wrist));
            cairo_new_path(c);
            cairo_rectangle(c, -a.tip.x, 0.0, a.width, a.rod_end - a.wrist);
            cairo_clip(c);
            self.paint_piece(c, -a.tip.x, -a.wrist, opacity, mask);
            cairo_restore(c);
        } else {
            // A custom PNG may contain only a hand. Give it a simple rod.
            cairo_new_path(c);
            cairo_move_to(c, (a.rod_x - a.tip.x) * pose.scale, pose.rod_start);
            cairo_line_to(
                c,
                (a.rod_x - a.tip.x) * pose.scale,
                pose.rod_start + pose.rod_length,
            );
            cairo_set_source_rgba(c, 0.09, 0.09, 0.10, opacity);
            cairo_set_line_width(c, 6.0 * pose.scale.max(0.5));
            cairo_stroke(c);
        }
        cairo_save(c);
        cairo_scale(c, pose.scale, pose.scale);
        cairo_new_path(c);
        cairo_rectangle(c, -a.tip.x, a.top - a.tip.y, a.width, a.wrist - a.top + 1.0);
        cairo_clip(c);
        self.paint_piece(c, -a.tip.x, -a.tip.y, opacity, mask);
        cairo_restore(c);
    }

    unsafe fn paint_piece(&self, c: *mut Cairo, x: f64, y: f64, opacity: f64, mask: bool) {
        if mask {
            cairo_set_source_rgba(c, 0.0, 0.0, 0.0, opacity);
            cairo_mask_surface(c, self.hand, x, y);
        } else {
            cairo_set_source_surface(c, self.hand, x, y);
            cairo_paint_with_alpha(c, opacity);
        }
    }
}
