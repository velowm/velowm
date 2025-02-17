use x11::xlib;

use crate::config::appearance::Bar;

pub struct StatusBar {
    display: *mut xlib::Display,
    pub window: xlib::Window,
    width: u32,
    height: u32,
    gc: xlib::GC,
    font: *mut xlib::XFontStruct,
    config: Bar,
    workspace_width: i32,
}

impl StatusBar {
    pub unsafe fn new(
        display: *mut xlib::Display,
        root: xlib::Window,
        screen_width: u32,
        config: Bar,
    ) -> Self {
        let height = config.height;
        let black = xlib::XBlackPixel(display, xlib::XDefaultScreen(display));

        let window = xlib::XCreateSimpleWindow(
            display,
            root,
            0,
            0,
            screen_width,
            height,
            0,
            black,
            config.get_background_color(),
        );

        let mut wa = xlib::XSetWindowAttributes {
            background_pixel: 0,
            border_pixel: 0,
            colormap: 0,
            event_mask: xlib::ExposureMask | xlib::ButtonPressMask,
            override_redirect: 1,
            backing_store: 0,
            backing_planes: 0,
            backing_pixel: 0,
            save_under: 0,
            do_not_propagate_mask: 0,
            win_gravity: 0,
            bit_gravity: 0,
            cursor: 0,
            background_pixmap: 0,
            border_pixmap: 0,
        };

        xlib::XChangeWindowAttributes(
            display,
            window,
            xlib::CWOverrideRedirect | xlib::CWBackPixel | xlib::CWEventMask,
            &mut wa,
        );

        let gc = xlib::XCreateGC(display, window, 0, std::ptr::null_mut());
        xlib::XSetForeground(display, gc, config.get_text_color());

        let font = xlib::XLoadQueryFont(display, b"fixed\0".as_ptr() as *const _);
        if !font.is_null() {
            xlib::XSetFont(display, gc, (*font).fid);
        }

        xlib::XMapWindow(display, window);
        xlib::XRaiseWindow(display, window);

        Self {
            display,
            window,
            width: screen_width,
            height,
            gc,
            font,
            config,
            workspace_width: 25,
        }
    }

    pub fn get_clicked_workspace(&self, x: i32, y: i32) -> Option<usize> {
        let workspace_area_start = 10;
        let workspace_area_width = (self.workspace_width + 5) * 10;

        if x >= workspace_area_start
            && x < workspace_area_start + workspace_area_width
            && y >= 0
            && y < self.height as i32
        {
            let workspace_idx = ((x - workspace_area_start) / (self.workspace_width + 5)) as usize;
            if workspace_idx < 10 {
                return Some(workspace_idx);
            }
        }
        None
    }

    pub unsafe fn draw(&self, current_workspace: usize) {
        if !self.config.enabled {
            xlib::XUnmapWindow(self.display, self.window);
            return;
        }

        xlib::XMapWindow(self.display, self.window);
        xlib::XRaiseWindow(self.display, self.window);

        xlib::XClearWindow(self.display, self.window);

        let mut x: i32 = 10;
        let workspace_height: i32 = self.height as i32 - 4;

        for i in 0..10 {
            if i == current_workspace {
                xlib::XSetForeground(self.display, self.gc, self.config.get_highlight_color());
            } else {
                xlib::XSetForeground(self.display, self.gc, self.config.get_background_color());
            }

            xlib::XFillRectangle(
                self.display,
                self.window,
                self.gc,
                x,
                2,
                self.workspace_width as u32,
                workspace_height as u32,
            );

            xlib::XSetForeground(self.display, self.gc, self.config.get_text_color());
            let text = format!(" {} ", i + 1);
            let text_width = if !self.font.is_null() {
                xlib::XTextWidth(self.font, text.as_ptr() as *const _, text.len() as i32)
            } else {
                10
            };

            xlib::XDrawString(
                self.display,
                self.window,
                self.gc,
                x + (self.workspace_width - text_width) / 2,
                15,
                text.as_ptr() as *const _,
                text.len() as i32,
            );

            x += self.workspace_width + 5;
        }

        if self.config.show_underline {
            xlib::XSetForeground(self.display, self.gc, self.config.get_underline_color());
            xlib::XFillRectangle(
                self.display,
                self.window,
                self.gc,
                0,
                self.height as i32 - 2,
                self.width,
                2,
            );
        }
    }
}

impl Drop for StatusBar {
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
