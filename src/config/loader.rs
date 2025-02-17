use anyhow::{Context, Result};
use serde::Deserialize;
use std::{fs, path::PathBuf};

use crate::{
    ui::appearance::Appearance,
    utils::{
        command::Command,
        keybind::{self, Bind},
    },
};

#[derive(Deserialize, Clone)]
pub struct Config {
    pub modifier: String,
    pub binds: Vec<Bind>,
    #[serde(default)]
    pub appearance: Appearance,
    #[serde(default = "default_logging_enabled")]
    pub logging_enabled: bool,
    #[serde(default = "default_auto_generated")]
    pub auto_generated: bool,
}

fn default_logging_enabled() -> bool {
    true
}

fn default_auto_generated() -> bool {
    false
}

impl Default for Config {
    fn default() -> Self {
        Self {
            modifier: "alt".to_string(),
            binds: vec![
                Bind {
                    key: "w".to_string(),
                    command: Command::Exit,
                },
                Bind {
                    key: "q".to_string(),
                    command: Command::Spawn("alacritty".to_string()),
                },
                Bind {
                    key: "c".to_string(),
                    command: Command::Close,
                },
                Bind {
                    key: "space".to_string(),
                    command: Command::ToggleFloat,
                },
                Bind {
                    key: "f".to_string(),
                    command: Command::ToggleFullscreen,
                },
            ],
            appearance: Appearance::default(),
            logging_enabled: true,
            auto_generated: true,
        }
    }
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

    pub fn get_focused_border_color(&self) -> u64 {
        self.appearance.get_focused_border_color()
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

# Enable or disable logging
logging_enabled = true

# Set to false to disable the popup notification
auto_generated = true

# Window appearance
[appearance]
# Border width in pixels
border_width = 2
# Border color in hex format (supports transparency)
border_color = "#2B0000"
# Border color for focused windows
focused_border_color = "#FF0000"
# Gap between windows in pixels
gaps = 8

# Floating window settings
[appearance.floating]
# Center windows when they become floating
center_on_float = true
# Default width for floating windows
width = 800
# Default height for floating windows
height = 600

# Keybindings
# Format: bind = key,command
# Commands:
#   - exit: Exit the window manager
#   - close: Close focused window
#   - workspace<N>: Switch to workspace N (1-10)
#   - toggle_float: Toggle floating mode for focused window
#   - toggle_fullscreen: Toggle fullscreen mode for focused window
#   - Any other string will be executed as a command
[[binds]]
key = "w"
command = "exit"

[[binds]]
key = "q"
command = "spawn alacritty"

[[binds]]
key = "c"
command = "close"

[[binds]]
key = "space"
command = "toggle_float"

[[binds]]
key = "f"
command = "toggle_fullscreen"

# Workspace bindings
[[binds]]
key = "1"
command = "workspace1"

[[binds]]
key = "2"
command = "workspace2"

[[binds]]
key = "3"
command = "workspace3"

[[binds]]
key = "4"
command = "workspace4"

[[binds]]
key = "5"
command = "workspace5"

[[binds]]
key = "6"
command = "workspace6"

[[binds]]
key = "7"
command = "workspace7"

[[binds]]
key = "8"
command = "workspace8"

[[binds]]
key = "9"
command = "workspace9"

[[binds]]
key = "0"
command = "workspace10""###;

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).context("Failed to create config directory")?;
        }

        fs::write(path, default_config).context("Failed to write default config")
    }
}
