use x11::xlib;

pub struct Window {
    pub id: xlib::Window,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}
