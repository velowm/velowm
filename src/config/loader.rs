use anyhow::{Context, Result};
use serde::Deserialize;
use std::{fs, path::PathBuf};

use super::{
    appearance::Appearance,
    keybind::{self, Bind},
};

#[derive(Deserialize, Clone)]
pub struct Config {
    pub modifier: String,
    pub binds: Vec<Bind>,
    #[serde(default)]
    pub appearance: Appearance,
}

impl Config {
    pub fn get_keysym_for_key(&self, key: &str) -> u64 {
        keybind::get_keysym_for_key(key)
    }

    pub fn get_modifier(&self) -> u32 {
        keybind::get_modifier(&self.modifier)
    }

    pub fn get_border_color(&self) -> u64 {
        self.appearance.get_border_color()
    }

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

        Ok(PathBuf::from(home).join(".config/velowm/config.toml"))
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
}
