use serde::Deserialize;
use x11::keysym;

use super::command::Command;

#[derive(Deserialize, Clone)]
pub struct Bind {
    pub key: String,
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
        _ => keysym::XK_w,
    }
    .into()
}
