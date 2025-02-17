use x11::xlib;

pub struct KeyboardState {
    pub modifiers: u32,
    pub last_key: Option<xlib::KeyCode>,
}
