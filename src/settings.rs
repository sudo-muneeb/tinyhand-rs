//! Shared settings geometry keeps drawing and click targets aligned.

pub const WIDTH: f64 = 680.0;
pub const HEIGHT: f64 = 488.0;
pub const TABS: [&str; 6] = [
    "General",
    "Pointers",
    "Appearance",
    "Drawing",
    "Sound",
    "About",
];
pub const SOUNDS: [&str; 7] = [
    "Off",
    "Thock",
    "Click",
    "Pop",
    "Wood block",
    "Boop",
    "Squeak",
];

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SettingsUi {
    pub open: bool,
    pub tab: usize,
    pub custom: bool,
    pub capture: Option<bool>,
    pub notice: Option<&'static str>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    Close,
    Tab(usize),
    Style(usize),
    Custom,
    Back,
    Import,
    UseCustom,
    Startup,
    Capture(bool),
    Size(f64),
    Angle(f64),
    Opacity(f64),
    Shadow,
    Thickness(f64),
    Fade(f64),
    Color(usize),
    Sound(usize),
    Volume(f64),
    LaserSound,
    Reset,
    Clear,
}

pub fn contains(x: f64, y: f64, rect: (f64, f64, f64, f64)) -> bool {
    x >= rect.0 && x < rect.0 + rect.2 && y >= rect.1 && y < rect.1 + rect.3
}

pub fn style_rect(index: usize) -> (f64, f64, f64, f64) {
    (
        204.0 + (index % 9) as f64 * 50.0,
        142.0 + (index / 9) as f64 * 52.0,
        44.0,
        46.0,
    )
}

pub fn sound_rect(index: usize) -> (f64, f64, f64, f64) {
    (
        204.0 + (index % 4) as f64 * 114.0,
        112.0 + (index / 4) as f64 * 76.0,
        106.0,
        68.0,
    )
}

pub fn hit(x: f64, y: f64, ui: SettingsUi) -> Option<Action> {
    if contains(x, y, (8.0, 6.0, 24.0, 24.0)) {
        return Some(Action::Close);
    }
    for tab in 0..6 {
        if contains(x, y, (12.0, 50.0 + tab as f64 * 34.0, 156.0, 30.0)) {
            return Some(Action::Tab(tab));
        }
    }
    let step = if x < 554.0 {
        -1.0
    } else if x >= 620.0 {
        1.0
    } else {
        0.0
    };
    match ui.tab {
        0 => {
            if contains(x, y, (204.0, 112.0, 452.0, 58.0)) {
                return Some(Action::Startup);
            }
            if contains(x, y, (204.0, 196.0, 452.0, 58.0)) {
                return Some(Action::Capture(true));
            }
            if contains(x, y, (204.0, 254.0, 452.0, 58.0)) {
                return Some(Action::Capture(false));
            }
        }
        1 if ui.custom => {
            if contains(x, y, (204.0, 34.0, 120.0, 26.0)) {
                return Some(Action::Back);
            }
            if contains(x, y, (224.0, 296.0, 412.0, 80.0)) {
                return Some(Action::Import);
            }
        }
        1 => {
            for index in 0..37 {
                if contains(x, y, style_rect(index)) {
                    return Some(Action::Style(index));
                }
            }
            if contains(x, y, (204.0, 430.0, 210.0, 34.0)) {
                return Some(Action::UseCustom);
            }
            if contains(x, y, (430.0, 430.0, 226.0, 34.0)) {
                return Some(Action::Custom);
            }
            if contains(x, y, (598.0, 87.0, 58.0, 30.0)) {
                return Some(Action::Reset);
            }
        }
        2 => {
            if step != 0.0 && contains(x, y, (530.0, 112.0, 126.0, 58.0)) {
                return Some(Action::Size(step));
            }
            if contains(x, y, (204.0, 170.0, 452.0, 58.0)) {
                return Some(Action::Shadow);
            }
            if step != 0.0 && contains(x, y, (530.0, 228.0, 126.0, 58.0)) {
                return Some(Action::Angle(step));
            }
            if step != 0.0 && contains(x, y, (530.0, 286.0, 126.0, 58.0)) {
                return Some(Action::Opacity(step));
            }
        }
        3 => {
            for color in 0..8 {
                if contains(x, y, (364.0 + color as f64 * 34.0, 121.0, 28.0, 28.0)) {
                    return Some(Action::Color(color));
                }
            }
            if step != 0.0 && contains(x, y, (530.0, 170.0, 126.0, 58.0)) {
                return Some(Action::Fade(step));
            }
            if step != 0.0 && contains(x, y, (530.0, 228.0, 126.0, 58.0)) {
                return Some(Action::Thickness(step));
            }
            if contains(x, y, (204.0, 320.0, 452.0, 38.0)) {
                return Some(Action::Clear);
            }
        }
        4 => {
            for sound in 0..7 {
                if contains(x, y, sound_rect(sound)) {
                    return Some(Action::Sound(sound));
                }
            }
            if step != 0.0 && contains(x, y, (530.0, 308.0, 126.0, 58.0)) {
                return Some(Action::Volume(step));
            }
            if contains(x, y, (204.0, 366.0, 452.0, 58.0)) {
                return Some(Action::LaserSound);
            }
        }
        _ => {}
    }
    None
}
