use serde::Deserialize;

#[derive(Deserialize, Default, Clone)]
pub struct Appearance {
    #[serde(default = "default_border_width")]
    pub border_width: u32,
    #[serde(default = "default_border_color")]
    pub border_color: String,
    #[serde(default = "default_gaps")]
    pub gaps: u32,
}

fn default_border_width() -> u32 {
    2
}

fn default_border_color() -> String {
    String::from("#7A8478")
}

fn default_gaps() -> u32 {
    8
}

impl Appearance {
    pub fn get_border_color(&self) -> u64 {
        let color = self.border_color.trim_start_matches('#');
        u64::from_str_radix(color, 16).unwrap_or(0x7A8478)
    }
}
