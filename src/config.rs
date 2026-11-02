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
}

impl Default for Config {
    fn default() -> Self {
        Self {
            style: 0,
            size: 0.13,
            thickness: 5.0,
            color: 0,
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
                    "fade" => c.fade = num(v, c.fade, 0.3, 20.0),
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
                    _ => {}
                }
            }
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
        let temp = path.with_extension("tmp");
        fs::write(&temp, s)?;
        fs::rename(temp, path)
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
    let mut parts = s.split('+');
    let mut mods = 0;
    let mut key = "";
    for p in parts.by_ref() {
        match p.to_ascii_lowercase().as_str() {
            "control" => mods |= 1,
            "alt" => mods |= 2,
            "shift" => mods |= 4,
            _ => {
                key = p;
                break;
            }
        }
    }
    mods != 0 && key.len() == 1 && key.is_ascii() && parts.next().is_none()
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
        fs::write(path, format!("[Desktop Entry]\nType=Application\nName=TinyHand for Linux\nExec=\"{}\"\nIcon=tinyhand-rs\nCategories=Utility;Accessibility;\nX-GNOME-Autostart-enabled=true\n", exe.replace('"', "\\\"")))
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
        assert_eq!(loaded.fade, 20.0);
        assert_eq!(loaded.toggle, c.toggle);
        fs::remove_file(path).unwrap();
    }
}
