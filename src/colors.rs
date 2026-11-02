//! Exact ink palette from upstream PointerStyle.swift and Preferences.swift,
//! commit c5fb36b7c4d7c87480958872a724b30a52427a47.
pub type Rgb = (f64, f64, f64);
pub const PURPLE: Rgb = (0.53, 0.33, 0.95);
pub const RED: Rgb = (1.0, 0.16, 0.12);
pub const fn hex(value: u32) -> Rgb {
    (
        ((value >> 16) & 255) as f64 / 255.0,
        ((value >> 8) & 255) as f64 / 255.0,
        (value & 255) as f64 / 255.0,
    )
}

pub const POINTER_COLORS: [(&str, Rgb); 37] = [
    ("lavender", PURPLE),
    ("glove", PURPLE),
    ("coral", RED),
    ("plush", hex(0x2F86F0)),
    ("robot", hex(0x16A87A)),
    ("pink", hex(0xEE4F98)),
    ("blue", hex(0x2F86F0)),
    ("mint", hex(0x16A87A)),
    ("lemon", hex(0xF2B705)),
    ("black", hex(0x1D1D21)),
    ("chrome", hex(0x6B7380)),
    ("holo", PURPLE),
    ("fuzzy", PURPLE),
    ("wood", hex(0x9A5E2C)),
    ("marble", hex(0x6B7380)),
    ("terrazzo", hex(0xF0561D)),
    ("glass", hex(0x1597E5)),
    ("jelly", hex(0xA33FE6)),
    ("pastel", hex(0xEE4F98)),
    ("pixel", hex(0x1D1D21)),
    ("lowpoly", PURPLE),
    ("knit", PURPLE),
    ("skeleton", hex(0x9A5E2C)),
    ("astro", hex(0x2F86F0)),
    ("space", RED),
    ("monster", hex(0x3E9E2E)),
    ("water", hex(0x1597E5)),
    ("lava", hex(0xF0561D)),
    ("grass", hex(0x3E9E2E)),
    ("cactus", hex(0x3E9E2E)),
    ("choco", hex(0x9A5E2C)),
    ("cloud", hex(0x2F86F0)),
    ("mech", hex(0xF0561D)),
    ("neon", hex(0xE0309A)),
    ("leaf", hex(0x3E9E2E)),
    ("cookie", hex(0x9A5E2C)),
    ("laser", RED),
];

// Keep legacy fixed-colour indices, using the former duplicate-purple slot
// for yellow. Config::load migrates old slot 5 to the retained purple slot.
pub const SWATCHES: [Rgb; 8] = [
    hex(0x8754F2),
    RED,
    hex(0xFF8A00),
    hex(0x30C25A),
    hex(0x0A84FF),
    hex(0xFFCC00),
    hex(0xFFFFFF),
    hex(0x1D1D21),
];

pub fn pointer_color(style: usize) -> Rgb {
    POINTER_COLORS.get(style).map_or(PURPLE, |entry| entry.1)
}
