use std::time::{Duration, Instant};
use tinyhand_rs::{
    config::{valid_shortcut, Config, STYLES},
    ink::{Ink, Point},
};

#[test]
fn pointer_movement_and_drag_are_distinct_from_a_tap() {
    let mut ink = Ink::new();
    let p = Point { x: 100.0, y: 100.0 };
    ink.down(p);
    ink.move_to(Point { x: 102.0, y: 100.0 });
    assert!(!ink.is_drawing());
    ink.move_to(Point { x: 104.0, y: 100.0 });
    ink.move_to(Point { x: 130.0, y: 140.0 });
    assert_eq!(ink.strokes[0].points.len(), 3);
    assert_eq!(ink.strokes[0].points[2], Point { x: 130.0, y: 140.0 });
    ink.end(Instant::now());
    ink.move_to(Point { x: 500.0, y: 500.0 });
    assert_eq!(
        ink.strokes[0].points.len(),
        3,
        "motion after release cannot continue a stroke"
    );
}

#[test]
fn release_clear_and_fade_cleanup() {
    let mut ink = Ink::new();
    let t = Instant::now();
    ink.down(Point { x: 1.0, y: 1.0 });
    ink.move_to(Point { x: 20.0, y: 20.0 });
    ink.end(t);
    ink.prune(t + Duration::from_millis(2900), Duration::from_secs(3));
    assert_eq!(ink.strokes.len(), 1);
    assert_eq!(ink.strokes[0].opacity(t + Duration::from_secs(3)), 1.0);
    assert!((ink.strokes[0].opacity(t + Duration::from_millis(3300)) - 0.75).abs() < 0.001);
    ink.prune(t + Duration::from_millis(3600), Duration::from_secs(3));
    assert!(ink.strokes.is_empty());
    ink.down(Point { x: 10.0, y: 10.0 });
    ink.clear();
    ink.move_to(Point { x: 50.0, y: 50.0 });
    assert!(!ink.is_drawing());
    assert!(ink.strokes.is_empty());
}

#[test]
fn long_drag_keeps_a_bounded_path_and_its_latest_point() {
    let mut ink = Ink::new();
    ink.down(Point { x: 0.0, y: 0.0 });
    for x in 1..30_000 {
        ink.move_to(Point {
            x: x as f64,
            y: 0.0,
        });
    }
    ink.move_to(Point {
        x: 40_000.0,
        y: 0.0,
    });
    let points = &ink.strokes[0].points;
    assert!(points.len() <= 8192);
    assert_eq!(
        points.last(),
        Some(&Point {
            x: 40_000.0,
            y: 0.0
        })
    );
}

#[test]
fn every_pointer_asset_and_shortcuts() {
    assert_eq!(STYLES.len(), 37);
    for name in &STYLES[..36] {
        let file = format!("{}/assets/hand_{name}.png", env!("CARGO_MANIFEST_DIR"));
        assert!(std::path::Path::new(&file).exists(), "missing {file}");
    }
    assert_eq!(STYLES[36], "laser");
    assert!(valid_shortcut(&Config::default().toggle));
    assert!(valid_shortcut("Control+Shift+z"));
    assert!(!valid_shortcut("z"));
}

#[test]
fn fade_delay_and_keep_ink_do_not_request_busy_idle_timers() {
    use tinyhand_rs::ink::Brush;
    let now = Instant::now();
    let mut ink = Ink::new();
    ink.down_with_brush(
        Point { x: 0.0, y: 0.0 },
        Brush {
            fade_after: 6.0,
            ..Brush::default()
        },
    );
    ink.move_to(Point { x: 100.0, y: 100.0 });
    ink.end(now);
    assert!(!ink.needs_animation(now + Duration::from_secs(5)));
    assert_eq!(ink.next_change(now), Some(Duration::from_secs(6)));
    assert!(ink.needs_animation(now + Duration::from_millis(6200)));
    assert!(
        ink.next_change(now + Duration::from_millis(6200)).is_none(),
        "the frame timer handles the fade, without a 1ms wake loop"
    );
    ink.down_with_brush(
        Point { x: 0.0, y: 10.0 },
        Brush {
            fade_after: 0.0,
            ..Brush::default()
        },
    );
    ink.move_to(Point { x: 100.0, y: 110.0 });
    ink.end(now);
    ink.prune(now + Duration::from_secs(1000), Duration::ZERO);
    assert_eq!(ink.strokes.len(), 1);
    assert_eq!(ink.strokes[0].opacity(now + Duration::from_secs(1000)), 1.0);
    assert!(!ink.needs_animation(now + Duration::from_secs(1000)));
    assert!(ink.next_change(now + Duration::from_secs(1000)).is_none());
}
