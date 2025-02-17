use x11::{xinerama, xlib};

use crate::config::loader::Config;

pub struct Window {
    id: xlib::Window,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

pub struct Monitor {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

pub struct MasterStackLayout {
    windows: Vec<Window>,
    display: *mut xlib::Display,
    root: xlib::Window,
    master_width_ratio: f32,
    #[allow(dead_code)]
    screen: i32,
    current_monitor: Monitor,
    config: Config,
    focused_window: Option<xlib::Window>,
}

impl MasterStackLayout {
    /// Creates a new master stack layout for managing window layouts.
    ///
    /// # Safety
    /// - The display pointer must be valid and point to an active X display connection.
    /// - The root window must be a valid window ID for the given display.
    /// - The caller must ensure the display connection remains valid for the lifetime of the layout.
    pub unsafe fn new(display: *mut xlib::Display, root: xlib::Window, config: Config) -> Self {
        let screen = xlib::XDefaultScreen(display);
        let current_monitor = {
            let mut num_monitors = 0;
            let monitors = xinerama::XineramaQueryScreens(display, &mut num_monitors);

            if !monitors.is_null() && num_monitors > 0 {
                let monitor = *monitors;
                let mon = Monitor {
                    x: monitor.x_org as i32,
                    y: monitor.y_org as i32,
                    width: monitor.width as u32,
                    height: monitor.height as u32,
                };
                xlib::XFree(monitors as *mut _);
                mon
            } else {
                Monitor {
                    x: 0,
                    y: 0,
                    width: xlib::XDisplayWidth(display, screen) as u32,
                    height: xlib::XDisplayHeight(display, screen) as u32,
                }
            }
        };

        Self {
            windows: Vec::new(),
            display,
            root,
            master_width_ratio: 0.5,
            screen,
            current_monitor,
            config,
            focused_window: None,
        }
    }

    pub fn get_root(&self) -> xlib::Window {
        self.root
    }

    pub fn focus_window(&mut self, window: xlib::Window) {
        if window == self.root {
            return;
        }

        unsafe {
            if let Some(old_focused) = self.focused_window {
                xlib::XSetWindowBorder(self.display, old_focused, self.config.get_border_color());
            }

            xlib::XSetWindowBorder(self.display, window, self.config.get_focused_border_color());
            xlib::XSetInputFocus(
                self.display,
                window,
                xlib::RevertToPointerRoot,
                xlib::CurrentTime,
            );
            xlib::XRaiseWindow(self.display, window);
            xlib::XSync(self.display, 0);
        }

        self.focused_window = Some(window);
    }

    pub fn add_window(&mut self, window: xlib::Window) {
        unsafe {
            xlib::XSetWindowBorderWidth(self.display, window, self.config.appearance.border_width);
            xlib::XSetWindowBorder(self.display, window, self.config.get_border_color());

            xlib::XSelectInput(
                self.display,
                window,
                xlib::EnterWindowMask | xlib::LeaveWindowMask | xlib::FocusChangeMask,
            );
        }

        let mut attrs: xlib::XWindowAttributes = unsafe { std::mem::zeroed() };
        unsafe {
            xlib::XGetWindowAttributes(self.display, window, &mut attrs);
        }

        let new_window = Window {
            id: window,
            x: attrs.x,
            y: attrs.y,
            width: attrs.width as u32,
            height: attrs.height as u32,
        };

        self.windows.push(new_window);
        self.relayout();
        self.focus_window(window);
    }

    pub fn clear_windows(&mut self) {
        self.windows.clear();
        self.focused_window = None;
    }

    pub fn remove_window(&mut self, window: xlib::Window) {
        if self.focused_window == Some(window) {
            self.focused_window = None;
            if let Some(last_window) = self.windows.iter().find(|w| w.id != window) {
                self.focus_window(last_window.id);
            }
        }
        self.windows.retain(|w| w.id != window);
        self.relayout();
    }

    fn get_screen_dimensions(&self) -> (u32, u32) {
        (self.current_monitor.width, self.current_monitor.height)
    }

    pub fn update_config(&mut self, config: Config) {
        self.config = config;

        unsafe {
            for window in &self.windows {
                xlib::XSetWindowBorderWidth(self.display, window.id, 0);

                xlib::XSetWindowBorderWidth(
                    self.display,
                    window.id,
                    self.config.appearance.border_width,
                );
                xlib::XSetWindowBorder(self.display, window.id, self.config.get_border_color());

                xlib::XClearWindow(self.display, window.id);
            }
            xlib::XSync(self.display, 0);
        }

        self.relayout();
    }

    pub fn relayout(&mut self) {
        if self.windows.is_empty() {
            return;
        }

        let (screen_width, screen_height) = self.get_screen_dimensions();
        let master_width = (screen_width as f32 * self.master_width_ratio) as u32;
        let stack_width = screen_width - master_width;

        let border_offset = self.config.appearance.border_width * 2;
        let gaps = self.config.appearance.gaps;
        let bar_height = if self.config.appearance.bar.enabled {
            self.config.appearance.bar.height
        } else {
            0
        };

        match self.windows.len() {
            0 => (),
            1 => {
                self.apply_window_geometry(
                    0,
                    self.current_monitor.x as u32 + gaps,
                    self.current_monitor.y as u32 + gaps + bar_height,
                    screen_width - border_offset - (gaps * 2),
                    screen_height - border_offset - (gaps * 2) - bar_height,
                );
            }
            n => {
                self.apply_window_geometry(
                    0,
                    self.current_monitor.x as u32 + gaps,
                    self.current_monitor.y as u32 + gaps + bar_height,
                    master_width - border_offset - gaps - (gaps / 2),
                    screen_height - border_offset - (gaps * 2) - bar_height,
                );

                let stack_count = n - 1;
                let height_per_window =
                    ((screen_height - bar_height - (gaps * (stack_count + 1) as u32))
                        / stack_count as u32)
                        .saturating_sub(border_offset);

                for i in 1..n {
                    let stack_index = i - 1;
                    self.apply_window_geometry(
                        i,
                        self.current_monitor.x as u32 + master_width + (gaps / 2),
                        self.current_monitor.y as u32
                            + gaps
                            + bar_height
                            + stack_index as u32 * (height_per_window + border_offset + gaps),
                        stack_width - border_offset - gaps - (gaps / 2),
                        height_per_window,
                    );
                }
            }
        }
    }

    fn apply_window_geometry(&mut self, index: usize, x: u32, y: u32, width: u32, height: u32) {
        if let Some(window) = self.windows.get_mut(index) {
            window.x = x as i32;
            window.y = y as i32;
            window.width = width;
            window.height = height;

            unsafe {
                xlib::XMoveResizeWindow(
                    self.display,
                    window.id,
                    window.x,
                    window.y,
                    window.width,
                    window.height,
                );
            }
        }
    }

    pub fn swap_windows(&mut self, window1: xlib::Window, window2: xlib::Window) {
        if let (Some(idx1), Some(idx2)) = (
            self.windows.iter().position(|w| w.id == window1),
            self.windows.iter().position(|w| w.id == window2),
        ) {
            self.windows.swap(idx1, idx2);
            self.relayout();
        }
    }
}
