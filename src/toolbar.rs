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
