//! Time-based adaptation of upstream ArmView's grip follow and tap compression.
use crate::ink::Point;
use std::time::Instant;

#[derive(Clone, Copy, Debug)]
pub struct HandGeometry {
    pub width: f64,
    pub top: f64,
    pub wrist: f64,
    pub rod_end: f64,
    pub tip: Point,
    pub rod_x: f64,
    pub rod_width: f64,
}

impl Default for HandGeometry {
    fn default() -> Self {
        Self {
            width: 256.0,
            top: 0.0,
            wrist: 180.0,
            rod_end: 384.0,
            tip: Point { x: 128.0, y: 0.0 },
            rod_x: 128.0,
            rod_width: 16.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HandPose {
    pub angle: f64,
    pub scale: f64,
    pub rod_start: f64,
    pub rod_length: f64,
    pub lift: f64,
}

impl HandPose {
    pub fn new(
        asset: HandGeometry,
        cursor: Point,
        grip: Point,
        screen_h: f64,
        size: f64,
        tilt: f64,
        press: f64,
    ) -> Self {
        let height = (size * screen_h).clamp(30.0, screen_h * 0.35) * (1.0 - 0.06 * press);
        let scale = height / (asset.wrist - asset.top).max(1.0);
        let dx = grip.x - cursor.x;
        let dy = grip.y - cursor.y;
        let reach = dx.hypot(dy);
        let side = (asset.rod_x - asset.tip.x) * scale;
        let along = (reach * reach - side * side).max(0.0).sqrt();
        let rod_start = (asset.wrist - asset.tip.y) * scale;
        let rod_length = (along - rod_start).max(screen_h * 0.05);
        let angle = dy.atan2(dx) - (rod_start + rod_length).atan2(side) + tilt;
        Self {
            angle,
            scale,
            rod_start,
            rod_length,
            lift: screen_h * 0.04 * (1.0 - 0.8 * press),
        }
    }
}

#[derive(Default)]
pub struct HandMotion {
    grip_x: Option<f64>,
    monitor: Option<(i32, i32, i32, i32)>,
    last_tick: Option<Instant>,
    pressed: bool,
    amount: f64,
    following: bool,
}

impl HandMotion {
    pub fn press(&mut self) {
        self.pressed = true;
        self.amount = 1.0;
    }
    pub fn release(&mut self) {
        self.pressed = false;
    }
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    pub fn is_animating(&self) -> bool {
        self.following || (!self.pressed && self.amount > 0.0)
    }

    pub fn advance(
        &mut self,
        cursor: Point,
        monitor: (i32, i32, i32, i32),
        now: Instant,
        follow: bool,
    ) -> (Point, f64) {
        let (x, y, w, h) = monitor;
        let target = (cursor.x + 0.30 * w as f64)
            .clamp(x as f64 + 0.15 * w as f64, x as f64 + 1.05 * w as f64);
        let dt = self.last_tick.map_or(1.0 / 60.0, |last| {
            now.saturating_duration_since(last).as_secs_f64().min(0.05)
        });
        if self.monitor != Some(monitor) {
            self.grip_x = None;
            self.monitor = Some(monitor);
        }
        let mut grip = self.grip_x.unwrap_or(target);
        if follow {
            grip += (target - grip) * (1.0 - (-dt / 0.20).exp());
        } else {
            grip = target;
        }
        self.following = follow && (target - grip).abs() > 0.5;
        if !self.following {
            grip = target;
        }
        if !self.pressed {
            self.amount *= (-dt / 0.084).exp();
            if self.amount < 0.002 {
                self.amount = 0.0;
            }
        }
        self.grip_x = Some(grip);
        self.last_tick = Some(now);
        (
            Point {
                x: grip,
                y: (y + h) as f64 + 0.12 * h as f64,
            },
            self.amount,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    #[test]
    fn press_compresses_then_settles_without_an_idle_animation_loop() {
        let now = Instant::now();
        let p = Point { x: 400.0, y: 200.0 };
        let monitor = (0, 0, 800, 600);
        let mut motion = HandMotion::default();
        let (grip, rest) = motion.advance(p, monitor, now, true);
        let asset = HandGeometry::default();
        let resting = HandPose::new(asset, p, grip, 600.0, 0.13, 0.0, rest);
        motion.press();
        let (grip, amount) = motion.advance(p, monitor, now + Duration::from_millis(16), true);
        let pressed = HandPose::new(asset, p, grip, 600.0, 0.13, 0.0, amount);
        assert!((pressed.scale / resting.scale - 0.94).abs() < 1e-6);
        assert!(pressed.lift < resting.lift);
        assert!(
            !motion.is_animating(),
            "a held stationary press needs no loop"
        );
        motion.release();
        assert!(motion.is_animating());
        for frame in 2..100 {
            motion.advance(p, monitor, now + Duration::from_millis(frame * 16), true);
        }
        assert!(
            !motion.is_animating(),
            "return to an idle event wait after settling"
        );
    }
    #[test]
    fn rod_follows_the_grip_and_stays_on_the_current_monitor() {
        let now = Instant::now();
        let monitor = (1920, 0, 1920, 1080);
        let mut motion = HandMotion::default();
        let p = Point {
            x: 2100.0,
            y: 200.0,
        };
        let (grip, _) = motion.advance(p, monitor, now, true);
        let new = Point {
            x: 3100.0,
            y: 300.0,
        };
        let (following, _) = motion.advance(new, monitor, now + Duration::from_millis(16), true);
        assert!(following.x > grip.x && following.x < new.x + 0.3 * 1920.0);
        assert!(motion.is_animating());
        let pose = HandPose::new(
            HandGeometry::default(),
            new,
            following,
            1080.0,
            0.13,
            0.0,
            0.0,
        );
        let endpoint = Point {
            x: new.x - (pose.rod_start + pose.rod_length) * pose.angle.sin(),
            y: new.y + (pose.rod_start + pose.rod_length) * pose.angle.cos(),
        };
        assert!(endpoint.distance(following) < 0.001);
    }
}
