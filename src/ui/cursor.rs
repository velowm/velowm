use anyhow::Result;
use x11::xlib;

pub struct Cursor {
    raw: xlib::Cursor,
    display: *mut xlib::Display,
}

impl Cursor {
    /// Creates a new cursor for the given X display.
    ///
    /// # Safety
    /// The display pointer must be valid and point to an active X display connection.
    /// The caller must ensure the display connection remains valid for the lifetime of the cursor.
    pub unsafe fn new(display: *mut xlib::Display) -> Result<Self> {
        let raw = xlib::XCreateFontCursor(display, 68);

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
