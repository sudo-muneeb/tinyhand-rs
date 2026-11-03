use crate::colors::{self, Rgb};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Brush {
    pub color: Rgb,
    pub width: f64,
    pub fade_after: f64,
}
impl Default for Brush {
    fn default() -> Self {
        Self {
            color: colors::PURPLE,
            width: 5.0,
            fade_after: 3.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}
impl Point {
    pub fn distance(self, other: Self) -> f64 {
        (self.x - other.x).hypot(self.y - other.y)
    }
}

pub struct Stroke {
    pub points: Vec<Point>,
    pub released: Option<Instant>,
    pub brush: Brush,
}
impl Stroke {
    pub fn opacity(&self, now: Instant) -> f64 {
        let Some(released) = self.released else {
            return 1.0;
        };
        if self.brush.fade_after <= 0.0 {
            return 1.0;
        }
        let t =
            (now.saturating_duration_since(released).as_secs_f64() - self.brush.fade_after) / 0.6;
        1.0 - t.clamp(0.0, 1.0).powi(2)
    }
    pub fn fading(&self, now: Instant) -> bool {
        self.released.is_some_and(|t| {
            self.brush.fade_after > 0.0
                && now.saturating_duration_since(t).as_secs_f64() >= self.brush.fade_after
                && self.opacity(now) > 0.0
        })
    }
}
pub struct Ink {
    pub strokes: Vec<Stroke>,
    start: Option<(Point, Brush)>,
    active: bool,
    revision: u64,
}

impl Default for Ink {
    fn default() -> Self {
        Self::new()
    }
}

impl Ink {
    pub fn new() -> Self {
        Self {
            strokes: vec![],
            start: None,
            active: false,
            revision: 0,
        }
    }
    pub fn down(&mut self, p: Point) {
        self.down_with_brush(p, Brush::default());
    }
    pub fn down_with_brush(&mut self, p: Point, brush: Brush) {
        self.end(Instant::now());
        self.start = Some((p, brush));
    }
    pub fn move_to(&mut self, p: Point) {
        if let Some((start, brush)) = self.start {
            if !self.active && start.distance(p) >= 4.0 {
                self.strokes.push(Stroke {
                    points: vec![start],
                    released: None,
                    brush,
                });
                self.active = true;
            }
            if self.active {
                let stroke = self.strokes.last_mut().expect("active stroke");
                if stroke
                    .points
                    .last()
                    .is_some_and(|last| last.distance(p) > 2.0)
                {
                    stroke.points.push(p);
                    self.revision = self.revision.wrapping_add(1);
                    if stroke.points.len() > 8192 {
                        stroke.points = stroke.points.iter().step_by(2).copied().collect();
                    }
                }
            }
        }
    }
    pub fn end(&mut self, now: Instant) {
        if self.active {
            if let Some(last) = self.strokes.last_mut() {
                last.released = Some(now);
                self.revision = self.revision.wrapping_add(1);
            }
        }
        self.start = None;
        self.active = false;
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn clear(&mut self) {
        self.revision = self.revision.wrapping_add(1);
        self.strokes.clear();
        self.start = None;
        self.active = false;
    }
    pub fn prune(&mut self, now: Instant, _fade: Duration) {
        let old = self.strokes.len();
        self.strokes.retain(|s| s.opacity(now) > 0.0);
        if old != self.strokes.len() {
            self.revision = self.revision.wrapping_add(1);
        }
    }
    pub fn needs_animation(&self, now: Instant) -> bool {
        self.strokes.iter().any(|s| s.fading(now))
    }
    pub fn next_change(&self, now: Instant) -> Option<Duration> {
        self.strokes
            .iter()
            .filter_map(|s| {
                let t = s.released?;
                if s.brush.fade_after <= 0.0 {
                    return None;
                }
                let elapsed = now.saturating_duration_since(t).as_secs_f64();
                (elapsed < s.brush.fade_after)
                    .then(|| Duration::from_secs_f64(s.brush.fade_after - elapsed))
            })
            .min()
    }
    pub fn is_drawing(&self) -> bool {
        self.active
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_new_brush_does_not_recolour_or_resize_existing_ink() {
        let mut ink = Ink::new();
        let now = Instant::now();
        let first = Brush {
            color: colors::RED,
            width: 3.0,
            ..Brush::default()
        };
        let next = Brush {
            color: colors::hex(0x2F86F0),
            width: 9.0,
            ..Brush::default()
        };
        ink.down_with_brush(Point { x: 0.0, y: 0.0 }, first);
        ink.move_to(Point { x: 20.0, y: 0.0 });
        ink.end(now);
        ink.down_with_brush(Point { x: 0.0, y: 40.0 }, next);
        ink.move_to(Point { x: 20.0, y: 40.0 });
        ink.end(now);
        assert_eq!(ink.strokes[0].brush, first);
        assert_eq!(ink.strokes[1].brush, next);
    }
    #[test]
    fn click_is_not_a_stroke_drag_fades() {
        let mut ink = Ink::new();
        let p = Point { x: 10.0, y: 10.0 };
        let t = Instant::now();
        ink.down(p);
        ink.move_to(Point { x: 12.0, y: 12.0 });
        ink.end(t);
        assert!(ink.strokes.is_empty());
        ink.down(p);
        ink.move_to(Point { x: 18.0, y: 15.0 });
        assert!(ink.is_drawing());
        ink.move_to(Point { x: 23.0, y: 20.0 });
        ink.end(t);
        assert_eq!(ink.strokes[0].points.len(), 3);
        ink.prune(t + Duration::from_millis(2999), Duration::from_secs(3));
        assert_eq!(ink.strokes.len(), 1);
        ink.prune(t + Duration::from_millis(3600), Duration::from_secs(3));
        assert!(ink.strokes.is_empty());
    }
}
