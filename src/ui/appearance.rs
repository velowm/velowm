use serde::Deserialize;

#[derive(Deserialize, Default, Clone)]
pub struct FloatingWindow {
    #[serde(default)]
    pub center_on_float: bool,
    #[serde(default = "default_float_width")]
    pub width: u32,
    #[serde(default = "default_float_height")]
    pub height: u32,
}

#[derive(Deserialize, Default, Clone)]
pub struct Appearance {
    #[serde(default = "default_border_width")]
    pub border_width: u32,
    #[serde(default = "default_border_color")]
    pub border_color: String,
    #[serde(default = "default_focused_border_color")]
    pub focused_border_color: String,
    #[serde(default = "default_gaps")]
    pub gaps: u32,
    #[serde(default)]
    pub floating: FloatingWindow,
    #[serde(default = "default_focus_follows_mouse")]
    pub focus_follows_mouse: bool,
}

fn default_border_width() -> u32 {
    2
}
fn default_border_color() -> String {
    String::from("#2B0000")
}
fn default_focused_border_color() -> String {
    String::from("#FF0000")
}
fn default_gaps() -> u32 {
    8
}
fn default_float_width() -> u32 {
    800
}
fn default_float_height() -> u32 {
    600
}
fn default_focus_follows_mouse() -> bool {
    true
}

impl Appearance {
    pub fn get_border_color(&self) -> u64 {
        let color = self.border_color.trim_start_matches('#');
        u64::from_str_radix(color, 16).unwrap_or(0x7A8478)
    }

    pub fn get_focused_border_color(&self) -> u64 {
        let color = self.focused_border_color.trim_start_matches('#');
        u64::from_str_radix(color, 16).unwrap_or(0xA7C080)
    }
}
