use anyhow::Result;
use x11::xlib;

pub struct Cursor {
    normal: xlib::Cursor,
    grabbing: xlib::Cursor,
    display: *mut xlib::Display,
}

impl Cursor {
    /// Creates a new cursor for the given X display.
    ///
    /// # Safety
    /// The display pointer must be valid and point to an active X display connection.
    /// The caller must ensure the display connection remains valid for the lifetime of the cursor.
    pub unsafe fn new(display: *mut xlib::Display) -> Result<Self> {
        let normal = xlib::XCreateFontCursor(display, 68);
        let grabbing = xlib::XCreateFontCursor(display, 90); // XC_hand2

        Ok(Self {
            normal,
            grabbing,
            display,
        })
    }

    pub fn normal(&self) -> xlib::Cursor {
        self.normal
    }

    pub fn grabbing(&self) -> xlib::Cursor {
        self.grabbing
    }
}

impl Drop for Cursor {
    fn drop(&mut self) {
        unsafe {
            xlib::XFreeCursor(self.display, self.normal);
            xlib::XFreeCursor(self.display, self.grabbing);
        }
    }
}
