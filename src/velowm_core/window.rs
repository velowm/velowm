use x11::xlib;

#[derive(Clone)]
pub struct Window {
    pub id: xlib::Window,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub is_floating: bool,
    pub pre_float_x: i32,
    pub pre_float_y: i32,
    pub pre_float_width: u32,
    pub pre_float_height: u32,
    pub is_fullscreen: bool,
    pub pre_fullscreen_x: i32,
    pub pre_fullscreen_y: i32,
    pub pre_fullscreen_width: u32,
    pub pre_fullscreen_height: u32,
    pub pre_fullscreen_border_width: u32,
    pub is_dock: bool,
}

impl Window {
    pub fn new(id: xlib::Window, x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            id,
            x,
            y,
            width,
            height,
            is_floating: false,
            pre_float_x: 0,
            pre_float_y: 0,
            pre_float_width: 0,
            pre_float_height: 0,
            is_fullscreen: false,
            pre_fullscreen_x: 0,
            pre_fullscreen_y: 0,
            pre_fullscreen_width: 0,
            pre_fullscreen_height: 0,
            pre_fullscreen_border_width: 0,
            is_dock: false,
        }
    }
}
