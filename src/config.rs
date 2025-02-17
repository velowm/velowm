use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;
use x11::keysym;

#[derive(Deserialize, Clone)]
pub enum Command {
    Exit,
    Spawn(String),
    Close,
}

impl Command {
    fn from_str(s: &str) -> Self {
        match s {
            "exit" => Command::Exit,
            "close" => Command::Close,
            cmd => Command::Spawn(cmd.to_string()),
        }
    }
}

#[derive(Deserialize, Clone)]
pub struct Bind {
    pub key: String,
    #[serde(deserialize_with = "deserialize_command")]
    pub command: Command,
}

fn deserialize_command<'de, D>(deserializer: D) -> Result<Command, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    Ok(Command::from_str(&s))
}

#[derive(Deserialize, Default, Clone)]
pub struct Appearance {
    #[serde(default = "default_border_width")]
    pub border_width: u32,
    #[serde(default = "default_border_color")]
    pub border_color: String,
}

fn default_border_width() -> u32 {
    2
}

fn default_border_color() -> String {
    String::from("#7A8478")
}

#[derive(Deserialize, Clone)]
pub struct Config {
    pub modifier: String,
    pub binds: Vec<Bind>,
    #[serde(default)]
    pub appearance: Appearance,
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_path = Self::get_config_path()?;

        if !config_path.exists() {
            Self::create_default_config(&config_path)?;
        }

        let content = fs::read_to_string(&config_path).context("Failed to read config file")?;

        toml::from_str(&content).context("Failed to parse config file")
    }

    pub fn get_config_path() -> Result<PathBuf> {
        let home = std::env::var("HOME").context("Failed to get HOME directory")?;

        Ok(PathBuf::from(home).join(".config/velocitty/config.toml"))
    }

    fn create_default_config(path: &PathBuf) -> Result<()> {
        let default_config = r###"# Global modifier key for all shortcuts
# You can combine multiple modifiers with + like:
# modifier = "alt+shift"
# modifier = "super+alt"
# Available modifiers: alt, ctrl, shift, super (or win)
modifier = "alt"

# Window appearance
[appearance]
# Border width in pixels
border_width = 2
# Border color in hex format (supports transparency)
border_color = "#7A8478"

# Keybindings
# Format: bind = key,command
# Commands:
#   - exit: Exit the window manager
#   - close: Close focused window
#   - Any other string will be executed as a command
[[binds]]
key = "w"
command = "exit"

[[binds]]
key = "q"
command = "alacritty"

[[binds]]
key = "c"
command = "close""###;

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).context("Failed to create config directory")?;
        }

        fs::write(path, default_config).context("Failed to write default config")
    }

    pub fn get_keysym_for_key(&self, key: &str) -> u64 {
        match key.to_lowercase().as_str() {
            "a" => keysym::XK_a,
            "b" => keysym::XK_b,
            "c" => keysym::XK_c,
            "d" => keysym::XK_d,
            "e" => keysym::XK_e,
            "f" => keysym::XK_f,
            "g" => keysym::XK_g,
            "h" => keysym::XK_h,
            "i" => keysym::XK_i,
            "j" => keysym::XK_j,
            "k" => keysym::XK_k,
            "l" => keysym::XK_l,
            "m" => keysym::XK_m,
            "n" => keysym::XK_n,
            "o" => keysym::XK_o,
            "p" => keysym::XK_p,
            "q" => keysym::XK_q,
            "r" => keysym::XK_r,
            "s" => keysym::XK_s,
            "t" => keysym::XK_t,
            "u" => keysym::XK_u,
            "v" => keysym::XK_v,
            "w" => keysym::XK_w,
            "x" => keysym::XK_x,
            "y" => keysym::XK_y,
            "z" => keysym::XK_z,
            _ => keysym::XK_w,
        }
        .into()
    }

    pub fn get_modifier(&self) -> u32 {
        self.modifier
            .split('+')
            .map(|m| match m.trim().to_lowercase().as_str() {
                "alt" => x11::xlib::Mod1Mask as u32,
                "ctrl" => x11::xlib::ControlMask as u32,
                "shift" => x11::xlib::ShiftMask as u32,
                "super" | "win" => x11::xlib::Mod4Mask as u32,
                _ => x11::xlib::Mod1Mask as u32,
            })
            .fold(0, |acc, mask| acc | mask)
    }

    pub fn get_border_color(&self) -> u64 {
        let color = self.appearance.border_color.trim_start_matches('#');
        u64::from_str_radix(color, 16).unwrap_or(0x7A8478)
    }
}
