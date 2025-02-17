use std::ffi::CString;
use x11::xlib;

pub struct NotificationWindow {
    display: *mut xlib::Display,
    pub window: xlib::Window,
    gc: xlib::GC,
    height: u32,
    font: *mut xlib::XFontStruct,
}

impl NotificationWindow {
    /// Creates a new notification window for displaying error messages
    ///
    /// # Safety
    /// - The display pointer must be valid and point to an active X display connection
    /// - The root window must be a valid window ID for the given display
    /// - The caller must ensure the display connection remains valid for the lifetime of this window
    pub unsafe fn new(display: *mut xlib::Display, root: xlib::Window) -> Self {
        let screen = xlib::XDefaultScreen(display);
        let white = xlib::XWhitePixel(display, screen);
        let red = 0xFF0000;
        let dark_gray = 0x0F0F0F;

        let width = 600;
        let height = 50;
        let x = (xlib::XDisplayWidth(display, screen) - width as i32) / 2;
        let y = 50;

        let window =
            xlib::XCreateSimpleWindow(display, root, x, y, width, height, 2, red, dark_gray);

        let gc = xlib::XCreateGC(display, window, 0, std::ptr::null_mut());
        xlib::XSetForeground(display, gc, white);

        let font_name = CString::new("-*-*-medium-r-*-*-14-*-*-*-*-*-*-*").unwrap();
        let font = xlib::XLoadQueryFont(display, font_name.as_ptr());

        if !font.is_null() {
            xlib::XSetFont(display, gc, (*font).fid);
        }

        xlib::XSelectInput(display, window, xlib::ExposureMask | xlib::ButtonPressMask);

        Self {
            display,
            window,
            gc,
            height,
            font,
        }
    }

    /// Shows an error message in the notification window
    ///
    /// # Safety
    /// - The display connection must still be valid
    /// - The window must not have been destroyed
    pub unsafe fn show_error(&self, message: &str) {
        xlib::XMapWindow(self.display, self.window);
        xlib::XRaiseWindow(self.display, self.window);

        let message = CString::new(message).unwrap();
        let x = 10;
        let y = self.height as i32 / 2 + 5;

        xlib::XClearWindow(self.display, self.window);
        xlib::XDrawString(
            self.display,
            self.window,
            self.gc,
            x,
            y,
            message.as_ptr(),
            message.as_bytes().len() as i32,
        );
        xlib::XFlush(self.display);
    }

    /// Hides the notification window
    ///
    /// # Safety
    /// - The display connection must still be valid
    /// - The window must not have been destroyed
    pub unsafe fn hide(&self) {
        xlib::XUnmapWindow(self.display, self.window);
        xlib::XFlush(self.display);
    }

    /// Handles a button press event on the notification window
    ///
    /// # Safety
    /// - The display connection must still be valid
    /// - The window must not have been destroyed
    pub unsafe fn handle_button_press(&self) {
        self.hide();
    }
}

impl Drop for NotificationWindow {
    fn drop(&mut self) {
        unsafe {
            if !self.font.is_null() {
                xlib::XFreeFont(self.display, self.font);
            }
            xlib::XFreeGC(self.display, self.gc);
            xlib::XDestroyWindow(self.display, self.window);
        }
    }
}
