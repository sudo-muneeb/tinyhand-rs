use std::time::{Duration, Instant};

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
}
pub struct Ink {
    pub strokes: Vec<Stroke>,
    start: Option<Point>,
    active: bool,
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
        }
    }
    pub fn down(&mut self, p: Point) {
        self.end(Instant::now());
        self.start = Some(p);
    }
    pub fn move_to(&mut self, p: Point) {
        if let Some(start) = self.start {
            if !self.active && start.distance(p) >= 4.0 {
                self.strokes.push(Stroke {
                    points: vec![start],
                    released: None,
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
            }
        }
        self.start = None;
        self.active = false;
    }
    pub fn clear(&mut self) {
        self.strokes.clear();
        self.start = None;
        self.active = false;
    }
    pub fn prune(&mut self, now: Instant, fade: Duration) {
        self.strokes.retain(|s| {
            s.released
                .is_none_or(|t| now.saturating_duration_since(t) < fade)
        });
    }
    pub fn is_drawing(&self) -> bool {
        self.active
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        ink.prune(t + Duration::from_secs(3), Duration::from_secs(3));
        assert!(ink.strokes.is_empty());
    }
}
