use anyhow::{anyhow, Result};
use std::env;
use x11::xlib;

pub struct Display {
    raw: *mut xlib::Display,
}

impl Display {
    pub fn new() -> Result<Self> {
        let display_name = env::var("DISPLAY").unwrap_or(String::from(":0"));
        let raw = unsafe { xlib::XOpenDisplay(display_name.as_bytes().as_ptr() as *const i8) };

        if raw.is_null() {
            return Err(anyhow!("Failed to open X display"));
        }

        unsafe {
            xlib::XSetErrorHandler(Some(Self::error_handler));
            xlib::XSynchronize(raw, 1);

            let root = xlib::XDefaultRootWindow(raw);
            xlib::XSelectInput(
                raw,
                root,
                xlib::SubstructureRedirectMask | xlib::SubstructureNotifyMask,
            );
        }

        Ok(Self { raw })
    }

    pub fn raw(&self) -> *mut xlib::Display {
        self.raw
    }

    unsafe extern "C" fn error_handler(_: *mut xlib::Display, e: *mut xlib::XErrorEvent) -> i32 {
        log::error!("X11 Error: {}", (*e).error_code);
        0
    }
}

impl Drop for Display {
    fn drop(&mut self) {
        unsafe {
            xlib::XCloseDisplay(self.raw);
        }
    }
}
