use std::ffi::CString;
use x11::xlib;

pub struct NotificationWindow {
    display: *mut xlib::Display,
    pub window: xlib::Window,
    gc: xlib::GC,
    font: *mut xlib::XFontStruct,
    current_message: Option<String>,
    line_height: i32,
    padding: i32,
    width: i32,
    height: i32,
    x: i32,
    y: i32,
}

pub struct NotificationManager {
    display: *mut xlib::Display,
    root: xlib::Window,
    notifications: Vec<NotificationWindow>,
    width: i32,
    padding: i32,
    initial_y: i32,
}

impl NotificationManager {
    /// Creates a new notification manager.
    ///
    /// # Safety
    ///
    /// The display pointer must be valid and point to an active X display connection.
    /// The root window must be a valid window ID for the given display.
    pub unsafe fn new(display: *mut xlib::Display, root: xlib::Window) -> Self {
        Self {
            display,
            root,
            notifications: Vec::new(),
            width: 600,
            padding: 10,
            initial_y: 50,
        }
    }

    /// Shows an error notification with the given message.
    ///
    /// # Safety
    ///
    /// The display pointer stored in self must still be valid and point to an active X display connection.
    pub unsafe fn show_error(&mut self, message: &str) {
        let mut notification = NotificationWindow::new(self.display, self.root, self.width);
        notification.show_error(message);
        self.notifications.push(notification);
        self.relayout();
    }

    /// Handles button press events for notification windows.
    ///
    /// # Safety
    ///
    /// The display pointer stored in self must still be valid and point to an active X display connection.
    /// The window ID must be valid for the given display.
    pub unsafe fn handle_button_press(&mut self, window: xlib::Window) {
        if let Some(index) = self.notifications.iter().position(|n| n.window == window) {
            self.notifications.remove(index);
            self.relayout();
        }
    }

    /// Handles expose events for notification windows.
    ///
    /// # Safety
    ///
    /// The display pointer stored in self must still be valid and point to an active X display connection.
    /// The window ID must be valid for the given display.
    pub unsafe fn handle_expose(&self, window: xlib::Window) {
        if let Some(notification) = self.notifications.iter().find(|n| n.window == window) {
            notification.redraw();
        }
    }

    /// Raises all notification windows to the top of the window stack.
    ///
    /// # Safety
    ///
    /// The display pointer stored in self must still be valid and point to an active X display connection.
    pub unsafe fn raise_all(&self) {
        for notification in &self.notifications {
            xlib::XRaiseWindow(self.display, notification.window);
        }
    }

    pub fn contains_window(&self, window: xlib::Window) -> bool {
        self.notifications.iter().any(|n| n.window == window)
    }

    unsafe fn relayout(&mut self) {
        let mut current_y = self.initial_y;
        for notification in &mut self.notifications {
            notification.move_to(current_y);
            current_y += notification.height + self.padding;
        }
    }
}

impl NotificationWindow {
    /// Creates a new notification window for displaying error messages
    ///
    /// # Safety
    /// - The display pointer must be valid and point to an active X display connection
    /// - The root window must be a valid window ID for the given display
    /// - The caller must ensure the display connection remains valid for the lifetime of this window
    pub unsafe fn new(display: *mut xlib::Display, root: xlib::Window, width: i32) -> Self {
        let screen = xlib::XDefaultScreen(display);
        let white = xlib::XWhitePixel(display, screen);

        let line_height = 20i32;
        let padding = 10i32;
        let initial_height = line_height + padding * 2;
        let x = (xlib::XDisplayWidth(display, screen) - width) / 2;
        let y = 50;

        let config = crate::config::loader::Config::load().unwrap_or_default();
        let background_color = config.appearance.get_notification_background_color();
        let border_color = config.appearance.get_notification_border_color();

        let window = xlib::XCreateSimpleWindow(
            display,
            root,
            x,
            y,
            width as u32,
            initial_height as u32,
            2,
            border_color,
            background_color,
        );

        let mut attrs: xlib::XSetWindowAttributes = std::mem::zeroed();
        attrs.override_redirect = 1;
        attrs.save_under = 1;
        attrs.do_not_propagate_mask =
            xlib::ButtonPressMask | xlib::ButtonReleaseMask | xlib::PointerMotionMask;
        xlib::XChangeWindowAttributes(
            display,
            window,
            xlib::CWOverrideRedirect | xlib::CWSaveUnder | xlib::CWDontPropagate,
            &mut attrs,
        );

        let net_wm_window_type = xlib::XInternAtom(display, c"_NET_WM_WINDOW_TYPE".as_ptr(), 0);
        let net_wm_window_type_dock =
            xlib::XInternAtom(display, c"_NET_WM_WINDOW_TYPE_DOCK".as_ptr(), 0);

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

        let net_wm_state = xlib::XInternAtom(display, c"_NET_WM_STATE".as_ptr(), 0);
        let net_wm_state_above = xlib::XInternAtom(display, c"_NET_WM_STATE_ABOVE".as_ptr(), 0);

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
            line_height,
            padding,
            width,
            height: initial_height,
            x,
            y,
        }
    }

    /// Shows an error message in the notification window
    ///
    /// # Safety
    /// - The display connection must still be valid
    /// - The window must not have been destroyed
    pub unsafe fn show_error(&mut self, message: &str) {
        self.current_message = Some(message.to_string());

        let lines: Vec<&str> = message.split('\n').collect();
        self.height = self.line_height * lines.len() as i32 + self.padding * 2;

        xlib::XResizeWindow(
            self.display,
            self.window,
            self.width as u32,
            self.height as u32,
        );

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

    unsafe fn move_to(&mut self, y: i32) {
        self.y = y;
        let screen = xlib::XDefaultScreen(self.display);
        self.x = (xlib::XDisplayWidth(self.display, screen) - self.width) / 2;
        xlib::XMoveWindow(self.display, self.window, self.x, self.y);
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
            let mut y = self.padding + self.line_height - 5;

            for line in lines {
                let line = CString::new(line.trim()).unwrap();
                xlib::XDrawString(
                    self.display,
                    self.window,
                    self.gc,
                    self.padding,
                    y,
                    line.as_ptr(),
                    line.as_bytes().len() as i32,
                );
                y += self.line_height;
            }

            xlib::XFlush(self.display);
        }
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
