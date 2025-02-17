use std::ffi::CString;
use x11::xlib;

pub struct NotificationWindow {
    display: *mut xlib::Display,
    pub window: xlib::Window,
    gc: xlib::GC,
    font: *mut xlib::XFontStruct,
    current_message: Option<String>,
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
        let line_height = 20;
        let padding = 10;
        let height = line_height * 5 + padding * 2;
        let x = (xlib::XDisplayWidth(display, screen) - width as i32) / 2;
        let y = 50;

        let window =
            xlib::XCreateSimpleWindow(display, root, x, y, width, height, 2, red, dark_gray);

        let mut attrs: xlib::XSetWindowAttributes = std::mem::zeroed();
        attrs.override_redirect = 1;
        attrs.save_under = 1;
        attrs.do_not_propagate_mask =
            xlib::ButtonPressMask | xlib::ButtonReleaseMask | xlib::PointerMotionMask;
        xlib::XChangeWindowAttributes(
            display,
            window,
            xlib::CWOverrideRedirect as u64
                | xlib::CWSaveUnder as u64
                | xlib::CWDontPropagate as u64,
            &mut attrs,
        );

        let net_wm_window_type =
            xlib::XInternAtom(display, b"_NET_WM_WINDOW_TYPE\0".as_ptr() as *const i8, 0);
        let net_wm_window_type_dock = xlib::XInternAtom(
            display,
            b"_NET_WM_WINDOW_TYPE_DOCK\0".as_ptr() as *const i8,
            0,
        );
        xlib::XChangeProperty(
            display,
            window,
            net_wm_window_type,
            xlib::XA_ATOM,
            32,
            xlib::PropModeReplace,
            &net_wm_window_type_dock as *const u64 as *const u8,
            1,
        );

        let net_wm_state = xlib::XInternAtom(display, b"_NET_WM_STATE\0".as_ptr() as *const i8, 0);
        let net_wm_state_above =
            xlib::XInternAtom(display, b"_NET_WM_STATE_ABOVE\0".as_ptr() as *const i8, 0);
        xlib::XChangeProperty(
            display,
            window,
            net_wm_state,
            xlib::XA_ATOM,
            32,
            xlib::PropModeReplace,
            &net_wm_state_above as *const u64 as *const u8,
            1,
        );

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
            font,
            current_message: None,
        }
    }

    /// Shows an error message in the notification window
    ///
    /// # Safety
    /// - The display connection must still be valid
    /// - The window must not have been destroyed
    pub unsafe fn show_error(&mut self, message: &str) {
        self.current_message = Some(message.to_string());
        xlib::XMapWindow(self.display, self.window);
        xlib::XRaiseWindow(self.display, self.window);

        let mut above: xlib::XWindowChanges = std::mem::zeroed();
        above.stack_mode = xlib::Above;
        xlib::XConfigureWindow(
            self.display,
            self.window,
            xlib::CWStackMode as u32,
            &mut above,
        );

        self.redraw();
    }

    /// Redraws the current message
    ///
    /// # Safety
    /// - The display connection must still be valid
    /// - The window must not have been destroyed
    pub unsafe fn redraw(&self) {
        if let Some(message) = &self.current_message {
            xlib::XClearWindow(self.display, self.window);

            let lines: Vec<&str> = message.split('\n').collect();
            let line_height = 20;
            let x = 10;
            let mut y = 25;

            for line in lines {
                let line = CString::new(line).unwrap();
                xlib::XDrawString(
                    self.display,
                    self.window,
                    self.gc,
                    x,
                    y,
                    line.as_ptr(),
                    line.as_bytes().len() as i32,
                );
                y += line_height;
            }

            xlib::XFlush(self.display);
        }
    }

    /// Hides the notification window
    ///
    /// # Safety
    /// - The display connection must still be valid
    /// - The window must not have been destroyed
    pub unsafe fn hide(&mut self) {
        self.current_message = None;
        xlib::XUnmapWindow(self.display, self.window);
        xlib::XFlush(self.display);
    }

    /// Handles a button press event on the notification window
    ///
    /// # Safety
    /// - The display connection must still be valid
    /// - The window must not have been destroyed
    pub unsafe fn handle_button_press(&mut self) {
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
