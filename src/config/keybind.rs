use super::command::deserialize_command;
use serde::Deserialize;
use x11::keysym;

pub use super::command::Command;

#[derive(Clone, Deserialize)]
pub struct Bind {
    pub key: String,
    #[serde(deserialize_with = "deserialize_command")]
    pub command: Command,
}

pub fn get_keysym_for_key(key: &str) -> u64 {
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
        "0" => keysym::XK_0,
        "1" => keysym::XK_1,
        "2" => keysym::XK_2,
        "3" => keysym::XK_3,
        "4" => keysym::XK_4,
        "5" => keysym::XK_5,
        "6" => keysym::XK_6,
        "7" => keysym::XK_7,
        "8" => keysym::XK_8,
        "9" => keysym::XK_9,
        "space" => keysym::XK_space,
        _ => keysym::XK_w,
    }
    .into()
}

pub fn get_modifier(modifier: &str) -> u32 {
    modifier
        .split('+')
        .map(|m| match m.trim().to_lowercase().as_str() {
            "alt" => x11::xlib::Mod1Mask,
            "ctrl" => x11::xlib::ControlMask,
            "shift" => x11::xlib::ShiftMask,
            "super" | "win" => x11::xlib::Mod4Mask,
            _ => x11::xlib::Mod1Mask,
        })
        .fold(0, |acc, mask| acc | mask)
}

pub fn get_modifier_for_key(key: &str) -> u32 {
    match key.to_lowercase().as_str() {
        "alt" => x11::xlib::Mod1Mask,
        "ctrl" => x11::xlib::ControlMask,
        "shift" => x11::xlib::ShiftMask,
        "super" | "win" => x11::xlib::Mod4Mask,
        _ => x11::xlib::Mod1Mask,
    }
}
