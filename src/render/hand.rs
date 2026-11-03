//! Segment the artwork and stretch its rod toward the off-screen grip,
//! following the geometry used by upstream ArmView.swift and HandAsset.
use super::*;

pub(super) unsafe fn custom_ink(image: *mut Surface, hand_rows: usize) -> Rgb {
    // Upstream HandMaker.ink: weighted dominant RGB bucket, then make pale
    // colours readable. Restrict built-in-shaped imports to the hand, not rod.
    cairo_surface_flush(image);
    let width = cairo_image_surface_get_width(image) as usize;
    let height = cairo_image_surface_get_height(image) as usize;
    let stride = cairo_image_surface_get_stride(image) as usize;
    let data = cairo_image_surface_get_data(image);
    if data.is_null() {
        return colors::PURPLE;
    }
    let mut buckets = [(0_u32, 0.0, 0.0, 0.0); 216];
    for y in (0..height.min(hand_rows)).step_by(3) {
        for x in (0..width).step_by(3) {
            let pixel = data.add(y * stride + x * 4);
            let a = *pixel.add(3) as f64;
            if a <= 200.0 {
                continue;
            }
            let (r, g, b) = (
                *pixel.add(2) as f64 / a,
                *pixel.add(1) as f64 / a,
                *pixel as f64 / a,
            );
            let key = (r * 5.0) as usize * 36 + (g * 5.0) as usize * 6 + (b * 5.0) as usize;
            let bucket = &mut buckets[key.min(215)];
            bucket.0 += 1;
            bucket.1 += r;
            bucket.2 += g;
            bucket.3 += b;
        }
    }
    let mut best = (-1.0, colors::PURPLE);
    for (n, r, g, b) in buckets.into_iter().filter(|e| e.0 > 0) {
        let (r, g, b) = (r / n as f64, g / n as f64, b / n as f64);
        let hi = r.max(g).max(b);
        let lo = r.min(g).min(b);
        let sat = if hi == 0.0 { 0.0 } else { (hi - lo) / hi };
        let lum = 0.3 * r + 0.59 * g + 0.11 * b;
        let score = n as f64 * (0.25 + sat) * if lum > 0.12 && lum < 0.85 { 1.0 } else { 0.15 };
        if score > best.0 {
            best = (score, (r, g, b));
        }
    }
    let (r, g, b) = best.1;
    let hi = r.max(g).max(b);
    let lo = r.min(g).min(b);
    if hi - lo < 0.08 {
        (0.42, 0.45, 0.5)
    } else if hi > 0.82 {
        (r * 0.82 / hi, g * 0.82 / hi, b * 0.82 / hi)
    } else {
        (r, g, b)
    }
}

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
    // Keep every visible rod pixel, including a wider cuff transition, while
    // excluding the large transparent side margins of the original canvas.
    let rod_x = center((wrist + rod_end) / 2);
    let mut rod_half = 0.5_f64;
    for y in wrist..rod_end {
        if let Some(first) = (0..w).find(|x| *data.add(y * stride + x * 4 + 3) > 8) {
            rod_half = rod_half.max((first as f64 - rod_x).abs());
        }
        if let Some(last) = (0..w).rfind(|x| *data.add(y * stride + x * 4 + 3) > 8) {
            rod_half = rod_half.max((last as f64 - rod_x).abs());
        }
    }
    HandGeometry {
        width: w as f64,
        top: (top as f64 - 1.0).max(0.0),
        wrist: wrist.max(top + 1) as f64,
        rod_end: rod_end as f64,
        tip: Point {
            x: center(top),
            y: top as f64,
        },
        rod_x,
        rod_width: rod_half * 2.0 + 1.0,
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
        let wrist = pose.rod_start;
        let bottom = pose.rod_start + pose.rod_length;
        let rod_left = (a.rod_x - a.tip.x - a.rod_width * 0.5 - 3.0) * pose.scale;
        let rod_right = (a.rod_x - a.tip.x + a.rod_width * 0.5 + 3.0) * pose.scale;
        let mut bounds = Rect::around(cursor, 36.0);
        for (x, y) in [
            (left, top),
            (right, top),
            (left, wrist),
            (right, wrist),
            (rod_left, wrist),
            (rod_right, wrist),
            (rod_left, bottom),
            (rod_right, bottom),
        ] {
            let p = Point {
                x: cursor.x + x * cos - y * sin,
                y: cursor.y + x * sin + y * cos,
            };
            bounds = bounds.union(Rect::around(p, 8.0));
        }
        if config.shadow {
            let blur = (2.5 / self.shadow_scale * pose.scale).max(4.0);
            bounds = bounds.union(Rect {
                x: bounds.x + pose.lift * 0.6 - blur,
                y: bounds.y + pose.lift - blur,
                w: bounds.w + 2.0 * blur,
                h: bounds.h + 2.0 * blur,
            });
        }
        bounds
    }

    /// Build a small alpha mask once per hand; moving shadows need one pass.
    pub(super) unsafe fn prepare_shadow(&mut self) {
        if !self.shadow.is_null() {
            cairo_surface_destroy(self.shadow);
        }
        let w = cairo_image_surface_get_width(self.hand);
        let h = cairo_image_surface_get_height(self.hand);
        let sw = 128.min(w);
        let sh = (h as f64 * sw as f64 / w as f64).round() as i32;
        self.shadow_scale = sw as f64 / w as f64;
        self.shadow = cairo_image_surface_create(2, sw, sh);
        let c = cairo_create(self.shadow);
        cairo_scale(c, self.shadow_scale, self.shadow_scale);
        cairo_set_source_surface(c, self.hand, 0.0, 0.0);
        cairo_paint(c);
        cairo_destroy(c);
        cairo_surface_flush(self.shadow);
        let stride = cairo_image_surface_get_stride(self.shadow) as usize;
        let data = cairo_image_surface_get_data(self.shadow);
        let (sw, sh) = (sw as usize, sh as usize);
        let mut horizontal = vec![0_u8; sw * sh];
        for y in 0..sh {
            for x in 0..sw {
                let mut sum = 0_u32;
                for dx in -2..=2 {
                    let sx = x as i32 + dx;
                    if (0..sw as i32).contains(&sx) {
                        sum += *data.add(y * stride + sx as usize) as u32;
                    }
                }
                horizontal[y * sw + x] = (sum / 5) as u8;
            }
        }
        for y in 0..sh {
            for x in 0..sw {
                let mut sum = 0_u32;
                for dy in -2..=2 {
                    let sy = y as i32 + dy;
                    if (0..sh as i32).contains(&sy) {
                        sum += horizontal[sy as usize * sw + x] as u32;
                    }
                }
                *data.add(y * stride + x) = (sum / 5) as u8;
            }
        }
        cairo_surface_mark_dirty(self.shadow);
    }

    pub(super) unsafe fn paint_hand(
        &self,
        c: *mut Cairo,
        config: &Config,
        cursor: Point,
        pose: HandPose,
    ) {
        if config.shadow && !self.shadow.is_null() {
            cairo_save(c);
            cairo_translate(c, cursor.x + pose.lift * 0.6, cursor.y + pose.lift);
            cairo_rotate(c, pose.angle);
            self.hand_parts(c, pose, config.opacity * 0.20, true);
            cairo_restore(c);
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
            let pad = if mask { 2.5 / self.shadow_scale } else { 2.0 };
            cairo_rectangle(
                c,
                a.rod_x - a.tip.x - a.rod_width * 0.5 - pad,
                0.0,
                a.rod_width + 2.0 * pad,
                a.rod_end - a.wrist,
            );
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
            cairo_save(c);
            cairo_scale(c, 1.0 / self.shadow_scale, 1.0 / self.shadow_scale);
            cairo_mask_surface(c, self.shadow, x * self.shadow_scale, y * self.shadow_scale);
            cairo_restore(c);
        } else {
            cairo_set_source_surface(c, self.hand, x, y);
            cairo_paint_with_alpha(c, opacity);
        }
    }
}
