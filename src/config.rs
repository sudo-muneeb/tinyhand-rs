use std::fs;
use std::io;
use std::path::PathBuf;

pub const STYLES: [&str; 37] = [
    "lavender", "glove", "coral", "plush", "robot", "pink", "blue", "mint", "lemon", "black",
    "chrome", "holo", "fuzzy", "wood", "marble", "terrazzo", "glass", "jelly", "pastel", "pixel",
    "lowpoly", "knit", "skeleton", "astro", "space", "monster", "water", "lava", "grass", "cactus",
    "choco", "cloud", "mech", "neon", "leaf", "cookie", "laser",
];

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub style: usize,
    pub size: f64,
    pub thickness: f64,
    pub color: usize,
    pub match_color: bool,
    pub fade: f64,
    pub sound: bool,
    pub volume: f64,
    pub startup: bool,
    pub toggle: String,
    pub clear: String,
    pub angle: f64,
    pub opacity: f64,
    pub shadow: bool,
    pub sound_kind: usize,
    pub laser_sound: bool,
    pub custom_path: String,
    pub custom_enabled: bool,
    pub custom_id: String,
    pub pinned: Vec<String>,
    pub interact_key: String,
    pub interact_keeps_hand: bool,
    pub show_ink_button: bool,
    pub welcomed: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            style: 0,
            size: 0.13,
            thickness: 5.0,
            color: 0,
            match_color: true,
            fade: 3.0,
            sound: true,
            volume: 0.55,
            startup: false,
            toggle: "Control+Alt+h".into(),
            clear: "Control+Alt+x".into(),
            angle: 0.0,
            opacity: 1.0,
            shadow: true,
            sound_kind: 1,
            laser_sound: false,
            custom_path: String::new(),
            custom_enabled: false,
            custom_id: String::new(),
            pinned: default_pins(),
            interact_key: "Alt".into(),
            interact_keeps_hand: false,
            show_ink_button: true,
            welcomed: false,
        }
    }
}

impl Config {
    pub fn path() -> PathBuf {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".config")))
            .unwrap_or_else(|| PathBuf::from("."));
        base.join("tinyhand-rs").join("settings.conf")
    }

    pub fn load(path: &std::path::Path) -> Self {
        let mut c = Self::default();
        let mut match_color = None;
        if let Ok(s) = fs::read_to_string(path) {
            for line in s.lines() {
                let Some((k, v)) = line.split_once('=') else {
                    continue;
                };
                match k.trim() {
                    "style" => c.style = STYLES.iter().position(|s| *s == v).unwrap_or(0),
                    "size" => c.size = num(v, c.size, 0.05, 0.35),
                    "thickness" => c.thickness = num(v, c.thickness, 1.0, 30.0),
                    "color" => c.color = v.parse::<usize>().unwrap_or(0).min(7),
                    "match_color" => match_color = Some(v == "true"),
                    "fade" => {
                        c.fade = if v == "0" {
                            0.0
                        } else {
                            num(v, c.fade, 0.5, 60.0)
                        }
                    }
                    "sound" => c.sound = v == "true",
                    "volume" => c.volume = num(v, c.volume, 0.0, 1.0),
                    "startup" => c.startup = v == "true",
                    "toggle" if valid_shortcut(v) => c.toggle = v.into(),
                    "clear" if valid_shortcut(v) => c.clear = v.into(),
                    "angle" => c.angle = num(v, c.angle, -1.2, 1.2),
                    "opacity" => c.opacity = num(v, c.opacity, 0.2, 1.0),
                    "shadow" => c.shadow = v == "true",
                    "sound_kind" => c.sound_kind = v.parse::<usize>().unwrap_or(1).min(6),
                    "laser_sound" => c.laser_sound = v == "true",
                    "custom_path" => c.custom_path = v.into(),
                    "custom_enabled" => c.custom_enabled = v == "true",
                    "custom_id" => c.custom_id = v.into(),
                    "pinned" => {
                        c.pinned = v
                            .split(',')
                            .filter(|s| valid_style_id(s))
                            .take(8)
                            .map(str::to_owned)
                            .collect()
                    }
                    "interact_key" if ["Alt", "Control", "Super", "Off"].contains(&v) => {
                        c.interact_key = v.into()
                    }
                    "interact_keeps_hand" => c.interact_keeps_hand = v == "true",
                    "show_ink_button" => c.show_ink_button = v == "true",
                    "welcomed" => c.welcomed = v == "true",
                    _ => {}
                }
            }
        }
        let legacy_color = c.color;
        if match_color.is_none() && c.color == 5 {
            c.color = 0;
        }
        c.match_color = match_color.unwrap_or(legacy_color == 0);
        let mut seen = std::collections::HashSet::new();
        c.pinned.retain(|s| seen.insert(s.clone()));
        if c.pinned.is_empty() {
            c.pinned = default_pins();
        }
        c
    }

    pub fn save(&self, path: &std::path::Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut s = format!("style={}\nsize={}\nthickness={}\ncolor={}\nfade={}\nsound={}\nvolume={}\nstartup={}\ntoggle={}\nclear={}\n",
            STYLES[self.style], self.size, self.thickness, self.color, self.fade,
            self.sound, self.volume, self.startup, self.toggle, self.clear);
        s.push_str(&format!("angle={}\nopacity={}\nsound_kind={}\nlaser_sound={}\ncustom_path={}\ncustom_enabled={}\n",
            self.angle, self.opacity, self.sound_kind, self.laser_sound,
            self.custom_path.replace(['\n', '\r'], ""), self.custom_enabled));
        s.push_str(&format!("shadow={}\n", self.shadow));
        s.push_str(&format!("match_color={}\n", self.match_color));
        s.push_str(&format!("custom_id={}\npinned={}\ninteract_key={}\ninteract_keeps_hand={}\nshow_ink_button={}\nwelcomed={}\n",
            self.custom_id,self.pinned.join(","),self.interact_key,self.interact_keeps_hand,self.show_ink_button,self.welcomed));
        let temp = path.with_extension("tmp");
        fs::write(&temp, s)?;
        fs::rename(temp, path)
    }
}

pub fn default_pins() -> Vec<String> {
    ["lavender", "glove", "coral", "plush", "robot", "laser"]
        .into_iter()
        .map(str::to_owned)
        .collect()
}
pub fn valid_style_id(id: &str) -> bool {
    STYLES.contains(&id)
        || (id.starts_with("custom-")
            && id.len() < 64
            && id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-'))
}
impl Config {
    pub fn selected_id(&self) -> String {
        if self.custom_enabled && !self.custom_id.is_empty() {
            self.custom_id.clone()
        } else {
            STYLES[self.style.min(36)].into()
        }
    }
    pub fn interaction_mask(&self) -> u32 {
        match self.interact_key.as_str() {
            "Alt" => 8,
            "Control" => 4,
            "Super" => 64,
            _ => 0,
        }
    }
    pub fn tap_sound_enabled(&self) -> bool {
        if self.style == 36 && !self.custom_enabled {
            self.laser_sound
        } else {
            self.sound
        }
    }
}

fn num(s: &str, default: f64, lo: f64, hi: f64) -> f64 {
    s.parse::<f64>()
        .ok()
        .filter(|n| n.is_finite())
        .map(|n| n.clamp(lo, hi))
        .unwrap_or(default)
}

pub fn valid_shortcut(s: &str) -> bool {
    let pieces = s.split('+').collect::<Vec<_>>();
    if pieces.len() < 2 {
        return false;
    }
    let mut strong = false;
    for part in &pieces[..pieces.len() - 1] {
        match part.to_ascii_lowercase().as_str() {
            "control" | "ctrl" | "alt" | "option" | "super" | "command" | "meta" => strong = true,
            "shift" => {}
            _ => return false,
        }
    }
    let key = pieces[pieces.len() - 1];
    let named = [
        "space",
        "Return",
        "Escape",
        "Tab",
        "BackSpace",
        "Delete",
        "Left",
        "Right",
        "Up",
        "Down",
        "Home",
        "End",
        "Page_Up",
        "Page_Down",
        "Insert",
        "plus",
        "minus",
    ];
    strong
        && ((key.len() == 1 && key.is_ascii() && key.as_bytes()[0].is_ascii_alphanumeric())
            || named.contains(&key)
            || key
                .strip_prefix('F')
                .and_then(|n| n.parse::<u8>().ok())
                .is_some_and(|n| (1..=24).contains(&n)))
}

pub fn update_autostart(enabled: bool, executable: &std::path::Path) -> io::Result<()> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."));
    let dir = base.join("autostart");
    let path = dir.join("tinyhand-rs.desktop");
    if enabled {
        fs::create_dir_all(dir)?;
        let exe = executable.to_string_lossy();
        if exe.contains(['\n', '\r']) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid executable path",
            ));
        }
        fs::write(path, format!("[Desktop Entry]\nType=Application\nName=TinyHand for Linux\nExec=\"{}\" --background\nIcon=tinyhand-rs\nCategories=Utility;Accessibility;\nX-GNOME-Autostart-enabled=true\n", exe.replace('"', "\\\"")))
    } else if path.exists() {
        fs::remove_file(path)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_colours_migrate_without_losing_explicit_overrides() {
        let path =
            std::env::temp_dir().join(format!("tinyhand-colour-migration-{}", std::process::id()));
        fs::write(&path, "color=0\n").unwrap();
        assert!(
            Config::load(&path).match_color,
            "old default purple becomes pointer matching"
        );
        fs::write(&path, "color=1\n").unwrap();
        assert!(!Config::load(&path).match_color, "explicit red stays fixed");
        fs::write(&path, "color=5\n").unwrap();
        let old = Config::load(&path);
        assert_eq!(old.color, 0);
        assert!(!old.match_color);
        fs::write(&path, "color=5\nmatch_color=false\n").unwrap();
        let yellow = Config::load(&path);
        assert_eq!(yellow.color, 5);
        assert!(!yellow.match_color);
        fs::remove_file(path).unwrap();
    }
    #[test]
    fn settings_roundtrip_and_validation() {
        let path = std::env::temp_dir().join(format!("tinyhand-test-{}-{}", std::process::id(), 1));
        let c = Config {
            style: 36,
            fade: 2.25,
            color: 4,
            ..Default::default()
        };
        c.save(&path).unwrap();
        assert_eq!(Config::load(&path), c);
        fs::write(&path, "style=unknown\nsize=nan\nfade=900\ntoggle=Bad\n").unwrap();
        let loaded = Config::load(&path);
        assert_eq!(loaded.style, 0);
        assert_eq!(loaded.size, 0.13);
        assert_eq!(loaded.fade, 60.0);
        assert_eq!(loaded.toggle, c.toggle);
        fs::remove_file(path).unwrap();
    }
}
