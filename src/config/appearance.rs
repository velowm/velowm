use serde::Deserialize;

#[derive(Deserialize, Default, Clone)]
pub struct Bar {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_bar_background_color")]
    pub background_color: String,
    #[serde(default = "default_bar_text_color")]
    pub text_color: String,
    #[serde(default = "default_bar_highlight_color")]
    pub highlight_color: String,
    #[serde(default)]
    pub show_underline: bool,
    #[serde(default = "default_bar_underline_color")]
    pub underline_color: String,
}

impl Bar {
    pub fn get_background_color(&self) -> u64 {
        let color = self.background_color.trim_start_matches('#');
        u64::from_str_radix(color, 16).unwrap_or(0x282C34)
    }

    pub fn get_text_color(&self) -> u64 {
        let color = self.text_color.trim_start_matches('#');
        u64::from_str_radix(color, 16).unwrap_or(0xABB2BF)
    }

    pub fn get_highlight_color(&self) -> u64 {
        let color = self.highlight_color.trim_start_matches('#');
        u64::from_str_radix(color, 16).unwrap_or(0x3E4451)
    }

    pub fn get_underline_color(&self) -> u64 {
        let color = self.underline_color.trim_start_matches('#');
        u64::from_str_radix(color, 16).unwrap_or(0x3E4451)
    }
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
    pub bar: Bar,
}

fn default_border_width() -> u32 {
    2
}

fn default_border_color() -> String {
    String::from("#7A8478")
}

fn default_focused_border_color() -> String {
    String::from("#A7C080")
}

fn default_gaps() -> u32 {
    8
}

fn default_bar_background_color() -> String {
    String::from("#0F0F0F")
}

fn default_bar_text_color() -> String {
    String::from("#ABB2BF")
}

fn default_bar_highlight_color() -> String {
    String::from("#3E4451")
}

fn default_bar_underline_color() -> String {
    String::from("#3E4451")
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
