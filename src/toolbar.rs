use crate::config::Config;
#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    Pointer(String),
    Ink,
    Sound,
    Clear,
    Settings,
    Exit,
}
pub fn items(c: &Config) -> Vec<Item> {
    let mut pins = c.pinned.clone();
    let selected = c.selected_id();
    if !pins.contains(&selected) {
        pins.push(selected);
    }
    let mut items = pins.into_iter().map(Item::Pointer).collect::<Vec<_>>();
    if c.show_ink_button {
        items.push(Item::Ink);
    }
    items.extend([Item::Sound, Item::Clear, Item::Settings, Item::Exit]);
    items
}
pub fn width(c: &Config) -> f64 {
    items(c).len() as f64 * 44.0 + 12.0
}
pub fn hit(c: &Config, x: f64, y: f64) -> Option<Item> {
    if !(5.0..49.0).contains(&y) || x < 6.0 {
        return None;
    }
    items(c).get(((x - 6.0) / 44.0) as usize).cloned()
}

/// Absolute-time fade: input frequency cannot change the transition duration.
pub struct Fade {
    from: f64,
    to: f64,
    started: std::time::Instant,
    duration: std::time::Duration,
}
impl Fade {
    pub fn new(value: f64, now: std::time::Instant) -> Self {
        Self {
            from: value,
            to: value,
            started: now,
            duration: std::time::Duration::ZERO,
        }
    }
    pub fn value(&self, now: std::time::Instant) -> f64 {
        if self.duration.is_zero() {
            return self.to;
        }
        let t = (now.saturating_duration_since(self.started).as_secs_f64()
            / self.duration.as_secs_f64())
        .clamp(0.0, 1.0);
        let ease = t * t * (3.0 - 2.0 * t);
        self.from + (self.to - self.from) * ease
    }
    pub fn target(&mut self, visible: bool, now: std::time::Instant) -> bool {
        let to = f64::from(visible);
        if self.to == to {
            return false;
        }
        self.from = self.value(now);
        self.to = to;
        self.started = now;
        self.duration = std::time::Duration::from_millis(if visible { 150 } else { 350 });
        true
    }
    pub fn animating(&self, now: std::time::Instant) -> bool {
        self.from != self.to && now.saturating_duration_since(self.started) < self.duration
    }
}

/// A deadline clock keeps frame starts aligned despite rounded poll timeouts.
pub struct FrameClock {
    next: std::time::Instant,
    interval: std::time::Duration,
}
impl FrameClock {
    pub fn new(now: std::time::Instant, millihertz: i32) -> Self {
        let hz = if millihertz > 0 {
            millihertz.clamp(30_000, 240_000)
        } else {
            60_000
        };
        Self {
            next: now,
            interval: std::time::Duration::from_nanos(1_000_000_000_000 / hz as u64),
        }
    }
    pub fn ready(&self, now: std::time::Instant) -> bool {
        now >= self.next
    }
    pub fn presented(&mut self, now: std::time::Instant) {
        if now >= self.next {
            let steps = now.duration_since(self.next).as_nanos() / self.interval.as_nanos() + 1;
            self.next += self.interval * steps.min(u32::MAX as u128) as u32;
        }
    }
    pub fn wait_ms(&self, now: std::time::Instant) -> u64 {
        let ns = self.next.saturating_duration_since(now).as_nanos();
        ns.div_ceil(1_000_000).min(500) as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};
    #[test]
    fn fades_follow_elapsed_time_despite_event_floods_and_reversals() {
        let t = Instant::now();
        let mut a = Fade::new(1.0, t);
        a.target(false, t);
        for i in 0..1750 {
            assert!(!a.target(false, t + Duration::from_micros(i * 100)));
            let _ = a.value(t + Duration::from_micros(i * 100));
        }
        assert!((a.value(t + Duration::from_millis(175)) - 0.5).abs() < 1e-9);
        let before = a.value(t + Duration::from_millis(175));
        a.target(true, t + Duration::from_millis(175));
        assert_eq!(a.value(t + Duration::from_millis(175)), before);
        assert_eq!(a.value(t + Duration::from_millis(325)), 1.0);
        assert!(!a.animating(t + Duration::from_millis(325)));
    }
    #[test]
    fn frame_clock_survives_poll_rounding_without_busy_waiting() {
        let t = Instant::now();
        let mut c = FrameClock::new(t, 60_000);
        c.presented(t);
        assert_eq!(c.wait_ms(t), 17);
        assert!(!c.ready(t + Duration::from_millis(16)));
        assert_eq!(c.wait_ms(t + Duration::from_millis(16)), 1);
        c.presented(t + Duration::from_millis(17));
        assert_eq!(c.wait_ms(t + Duration::from_millis(17)), 17);
        c.presented(t + Duration::from_secs(5));
        assert!(!c.ready(t + Duration::from_secs(5)));
    }
}
