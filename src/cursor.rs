use anyhow::Result;
use x11::xlib;

pub struct Cursor {
    raw: xlib::Cursor,
    display: *mut xlib::Display,
}

impl Cursor {
    pub fn new(display: *mut xlib::Display) -> Result<Self> {
        let raw = unsafe { xlib::XCreateFontCursor(display, 68) };

        Ok(Self { raw, display })
    }

    pub fn raw(&self) -> xlib::Cursor {
        self.raw
    }
}

impl Drop for Cursor {
    fn drop(&mut self) {
        unsafe {
            xlib::XFreeCursor(self.display, self.raw);
        }
    }
}
