//! Managed custom-hand library and upstream-compatible image validation.
use gtk::gdk_pixbuf::{Colorspace, Pixbuf};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CustomHand {
    pub id: String,
    pub name: String,
    pub ink: u32,
}
#[derive(Clone, Default)]
pub struct Library {
    pub hands: Vec<CustomHand>,
}
pub struct PreparedHand {
    pub png: Vec<u8>,
    pub ink: u32,
    pub name: String,
}

impl Library {
    pub fn directory() -> PathBuf {
        if let Some(p) =
            std::env::var_os("TINYHAND_HANDS_DIR").or_else(|| std::env::var_os("TH_HANDS_DIR"))
        {
            return p.into();
        }
        let base = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/share")
            });
        base.join("tinyhand-rs/hands")
    }
    pub fn load() -> Self {
        let mut hands: Vec<CustomHand> = std::fs::read(Self::directory().join("hands.json"))
            .ok()
            .and_then(|v| serde_json::from_slice(&v).ok())
            .unwrap_or_default();
        hands.retain(|h| {
            crate::config::valid_style_id(&h.id)
                && h.id.starts_with("custom-")
                && Self::path(&h.id).is_file()
        });
        hands.truncate(100);
        Self { hands }
    }
    pub fn path(id: &str) -> PathBuf {
        Self::directory().join(format!("{id}.png"))
    }
    fn save(&self) -> Result<(), String> {
        let dir = Self::directory();
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let bytes = serde_json::to_vec_pretty(&self.hands).map_err(|e| e.to_string())?;
        std::fs::write(dir.join("hands.json.tmp"), bytes)
            .and_then(|_| std::fs::rename(dir.join("hands.json.tmp"), dir.join("hands.json")))
            .map_err(|e| e.to_string())
    }
    pub fn add(&mut self, hand: PreparedHand) -> Result<String, String> {
        if self.hands.len() >= 100 {
            return Err("Remove a custom hand before adding another.".into());
        }
        let id = format!(
            "custom-{:x}-{:x}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
            std::process::id()
        );
        std::fs::create_dir_all(Self::directory()).map_err(|e| e.to_string())?;
        std::fs::write(Self::path(&id), hand.png).map_err(|e| e.to_string())?;
        self.hands.push(CustomHand {
            id: id.clone(),
            name: if hand.name.trim().is_empty() {
                format!("My hand {}", self.hands.len() + 1)
            } else {
                hand.name.trim().chars().take(16).collect()
            },
            ink: hand.ink,
        });
        if let Err(e) = self.save() {
            self.hands.pop();
            let _ = std::fs::remove_file(Self::path(&id));
            return Err(e);
        }
        Ok(id)
    }
    pub fn remove(&mut self, id: &str) -> Result<(), String> {
        let old = self.hands.clone();
        self.hands.retain(|h| h.id != id);
        if let Err(e) = self.save() {
            self.hands = old;
            return Err(e);
        }
        let _ = std::fs::remove_file(Self::path(id));
        Ok(())
    }
    pub fn hand(&self, id: &str) -> Option<&CustomHand> {
        self.hands.iter().find(|h| h.id == id)
    }
}

pub fn prompt(idea: &str) -> String {
    include_str!("../assets/hand-prompt.txt").replace(
        "[describe your hand, e.g. knitted wool in mustard yellow]",
        if idea.trim().is_empty() {
            "a soft vinyl toy in a bright colour"
        } else {
            idea.trim()
        },
    )
}

pub fn prepare(path: &Path, name: &str) -> Result<PreparedHand, String> {
    if std::fs::metadata(path).map_err(|e| e.to_string())?.len() > 32 * 1024 * 1024 {
        return Err("Choose an image smaller than 32 MB.".into());
    }
    let (w, h) = Pixbuf::file_info(path)
        .map(|(_, w, h)| (w, h))
        .ok_or("That file is not a supported image.")?;
    if w < 64 || h < 64 || w > 4096 || h > 4096 {
        return Err("Use an image between 64 and 4096 pixels on each side.".into());
    }
    let image = Pixbuf::from_file(path).map_err(|e| e.to_string())?;
    if !image.has_alpha() {
        return Err("The image needs a transparent background.".into());
    }
    let (w, h, stride) = (
        image.width() as usize,
        image.height() as usize,
        image.rowstride() as usize,
    );
    let bytes = image.read_pixel_bytes();
    let p = bytes.as_ref();
    let alpha = |x: usize, y: usize| p[y * stride + x * 4 + 3];
    let mut edge = 0;
    for x in 0..w {
        edge += usize::from(alpha(x, 0) > 32) + usize::from(alpha(x, h - 1) > 32);
    }
    for y in 0..h {
        edge += usize::from(alpha(0, y) > 32) + usize::from(alpha(w - 1, y) > 32);
    }
    if edge as f64 / (2 * (w + h)) as f64 > 0.02 {
        return Err("Use a transparent background with some space around the hand.".into());
    }
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
    let mut any = false;
    for y in 0..h {
        for x in 0..w {
            if alpha(x, y) > 128 {
                any = true;
                x0 = x0.min(x);
                x1 = x1.max(x);
                y0 = y0.min(y);
                y1 = y1.max(y);
            }
        }
    }
    if !any {
        return Err("The image is empty.".into());
    }
    let (bw, bh) = (x1 - x0 + 1, y1 - y0 + 1);
    let aspect = bh as f64 / bw as f64;
    if !(1.05..=2.8).contains(&aspect) {
        return Err("The hand should stand upright, taller than it is wide.".into());
    }
    if (bh as f64) / (h as f64) < 0.5 {
        return Err("Crop closer so the hand fills most of the image.".into());
    }
    let step = (w.min(h) / 256).max(1);
    let (gw, gh) = (w / step, h / step);
    let mut grid = vec![false; gw * gh];
    for gy in 0..gh {
        for gx in 0..gw {
            grid[gy * gw + gx] = alpha(gx * step, gy * step) > 128;
        }
    }
    let (mut total, mut largest) = (0, 0);
    for start in 0..grid.len() {
        if !grid[start] {
            continue;
        }
        let mut stack = vec![start];
        grid[start] = false;
        let mut count = 0;
        while let Some(i) = stack.pop() {
            count += 1;
            let (x, y) = (i % gw, i / gw);
            for (nx, ny) in [
                (x as i32 + 1, y as i32),
                (x as i32 - 1, y as i32),
                (x as i32, y as i32 + 1),
                (x as i32, y as i32 - 1),
            ] {
                if nx >= 0 && ny >= 0 && nx < gw as i32 && ny < gh as i32 {
                    let n = ny as usize * gw + nx as usize;
                    if grid[n] {
                        grid[n] = false;
                        stack.push(n);
                    }
                }
            }
        }
        total += count;
        largest = largest.max(count);
    }
    if total == 0 || largest as f64 / (total as f64) < 0.97 {
        return Err("Use one connected hand, without extra objects.".into());
    }
    let (mut finger_lo, mut finger_hi) = (w, 0);
    for y in y0..(y0 + (bh * 8 / 100).max(2)).min(h) {
        for x in x0..=x1 {
            if alpha(x, y) > 128 {
                finger_lo = finger_lo.min(x);
                finger_hi = finger_hi.max(x);
            }
        }
    }
    if (finger_hi.saturating_sub(finger_lo) + 1) as f64 / bw as f64 > 0.4 {
        return Err("The highest part should be a single finger pointing up.".into());
    }
    let (mut lo, mut hi) = (w, 0);
    for y in y1.saturating_sub((bh * 4 / 100).max(2))..=y1 {
        for x in x0..=x1 {
            if alpha(x, y) > 128 {
                lo = lo.min(x);
                hi = hi.max(x);
            }
        }
    }
    if (hi.saturating_sub(lo) + 1) as f64 / (bw as f64) < 0.2 {
        return Err("The hand should end in a flat wrist or cuff.".into());
    }
    let (mut soft, mut solid) = (0, 0);
    for y in (0..h).step_by(2) {
        for x in (0..w).step_by(2) {
            let a = alpha(x, y);
            if a > 20 {
                solid += 1;
                if a < 200 {
                    soft += 1;
                }
            }
        }
    }
    if soft as f64 / (solid.max(1) as f64) > 0.08 {
        return Err("Use a hand without a soft shadow or glow.".into());
    }
    let mut buckets = [(0_u32, 0.0, 0.0, 0.0); 216];
    for y in (0..h).step_by(3) {
        for x in (0..w).step_by(3) {
            let i = y * stride + x * 4;
            if p[i + 3] <= 200 {
                continue;
            }
            let (r, g, b) = (
                p[i] as f64 / 255.0,
                p[i + 1] as f64 / 255.0,
                p[i + 2] as f64 / 255.0,
            );
            let key = (r * 5.0) as usize * 36 + (g * 5.0) as usize * 6 + (b * 5.0) as usize;
            let e = &mut buckets[key];
            e.0 += 1;
            e.1 += r;
            e.2 += g;
            e.3 += b;
        }
    }
    let mut best = (-1.0, (0.53_f64, 0.33_f64, 0.95_f64));
    for (n, r, g, b) in buckets.into_iter().filter(|e| e.0 > 0) {
        let (r, g, b) = (r / n as f64, g / n as f64, b / n as f64);
        let high = r.max(g).max(b);
        let low = r.min(g).min(b);
        let score = n as f64
            * (0.25 + if high > 0.0 { (high - low) / high } else { 0.0 })
            * if (0.12..0.85).contains(&(0.3 * r + 0.59 * g + 0.11 * b)) {
                1.0
            } else {
                0.15
            };
        if score > best.0 {
            best = (score, (r, g, b));
        }
    }
    let (mut r, mut g, mut b) = best.1;
    let high = r.max(g).max(b);
    if high - r.min(g).min(b) < 0.08 {
        (r, g, b) = (0.42, 0.45, 0.5);
    } else if high > 0.82 {
        r *= 0.82 / high;
        g *= 0.82 / high;
        b *= 0.82 / high;
    }
    let ink = ((r * 255.0) as u32) << 16 | ((g * 255.0) as u32) << 8 | (b * 255.0) as u32;
    let scale = (560.0 / bh as f64).min(700.0 / bw as f64);
    let cropped = image.new_subpixbuf(x0 as i32, y0 as i32, bw as i32, bh as i32);
    let scaled = cropped
        .scale_simple(
            (bw as f64 * scale).round() as i32,
            (bh as f64 * scale).round() as i32,
            gtk::gdk_pixbuf::InterpType::Bilinear,
        )
        .ok_or("Cannot resize this image.")?;
    let mut rgba = vec![0_u8; 1024 * 1536 * 4];
    let hand_bottom = 150 + scaled.height() as usize;
    let rod_width = (bw as f64 * scale * 0.18).clamp(54.0, 76.0) as usize;
    let rod_top = hand_bottom.saturating_sub(30);
    let radius = rod_width as f64 / 2.0;
    let stops = [
        (0.0, [0.0, 0.0, 0.0]),
        (0.22, [18.0, 18.0, 20.0]),
        (0.45, [58.0, 58.0, 62.0]),
        (0.75, [18.0, 18.0, 20.0]),
        (1.0, [0.0, 0.0, 0.0]),
    ];
    for y in rod_top..1462 {
        for x in (512 - rod_width / 2)..(512 + rod_width / 2) {
            let dx = (x as f64 + 0.5 - 512.0).abs();
            let cap = if (y as f64) < rod_top as f64 + radius {
                rod_top as f64 + radius - y as f64 - 0.5
            } else if y as f64 > 1462.0 - radius {
                y as f64 + 0.5 - (1462.0 - radius)
            } else {
                0.0
            };
            let alpha = (radius - (dx * dx + cap * cap).sqrt() + 0.5).clamp(0.0, 1.0);
            let t = (x as f64 + 0.5 - (512.0 - radius)) / (radius * 2.0);
            let mut colour = [0; 3];
            for pair in stops.windows(2) {
                if t >= pair[0].0 && t <= pair[1].0 {
                    let f = (t - pair[0].0) / (pair[1].0 - pair[0].0);
                    for (k, c) in colour.iter_mut().enumerate() {
                        *c = (pair[0].1[k] + (pair[1].1[k] - pair[0].1[k]) * f).round() as u8;
                    }
                    break;
                }
            }
            let i = (y * 1024 + x) * 4;
            rgba[i..i + 4].copy_from_slice(&[
                colour[0],
                colour[1],
                colour[2],
                (alpha * 255.0).round() as u8,
            ]);
        }
    }
    let src = scaled.read_pixel_bytes();
    let src = src.as_ref();
    let sh = scaled.height() as usize;
    let sw = scaled.width() as usize;
    let ss = scaled.rowstride() as usize;
    let cuff = ((lo + hi) as f64 / 2.0 + 0.5 - x0 as f64) * scale;
    let ox = (512.0 - cuff).round() as i32;
    for y in 0..sh {
        for x in 0..sw {
            let dx = ox + x as i32;
            if !(0..1024).contains(&dx) {
                continue;
            }
            let si = y * ss + x * 4;
            let di = ((150 + y) * 1024 + dx as usize) * 4;
            let a = src[si + 3] as f64 / 255.0;
            let da = rgba[di + 3] as f64 / 255.0;
            let out = a + da * (1.0 - a);
            for k in 0..3 {
                rgba[di + k] = if out > 0.0 {
                    ((src[si + k] as f64 * a + rgba[di + k] as f64 * da * (1.0 - a)) / out).round()
                        as u8
                } else {
                    0
                };
            }
            rgba[di + 3] = (out * 255.0).round() as u8;
        }
    }
    let composed = Pixbuf::from_mut_slice(rgba, Colorspace::Rgb, true, 8, 1024, 1536, 4096);
    let png = composed
        .save_to_bufferv("png", &[])
        .map_err(|e| e.to_string())?;
    Ok(PreparedHand {
        png,
        ink,
        name: name.into(),
    })
}
