use anyhow::{anyhow, Result};
use std::{env, ffi::CString};
use x11::xlib;

pub struct Display {
    raw: *mut xlib::Display,
}

impl Display {
    pub fn new() -> Result<Self> {
        unsafe {
            xlib::XSetErrorHandler(Some(Self::error_handler));
        }

        let display_name = env::var("DISPLAY").unwrap_or_else(|_| String::from(":0"));
        let c_display_name =
            CString::new(display_name).map_err(|_| anyhow!("Invalid DISPLAY variable"))?;
        let raw = unsafe { xlib::XOpenDisplay(c_display_name.as_ptr()) };

        if raw.is_null() {
            return Err(anyhow!("Failed to open X display"));
        }

        unsafe {
            xlib::XSynchronize(raw, 1);
            xlib::XGrabServer(raw);
            xlib::XSync(raw, false as i32);
            xlib::XUngrabServer(raw);
        }

        Ok(Self { raw })
    }

    pub fn raw(&self) -> *mut xlib::Display {
        self.raw
    }

    unsafe extern "C" fn error_handler(
        display: *mut xlib::Display,
        e: *mut xlib::XErrorEvent,
    ) -> i32 {
        let mut error_text = [0i8; 1024];
        xlib::XGetErrorText(
            display,
            (*e).error_code as i32,
            error_text.as_mut_ptr(),
            error_text.len() as i32,
        );

        let error_msg = std::ffi::CStr::from_ptr(error_text.as_ptr())
            .to_string_lossy()
            .into_owned();

        log::error!(
            "X11 Error: {} (code: {}, resource id: {}, request code: {})",
            error_msg,
            (*e).error_code,
            (*e).resourceid,
            (*e).request_code
        );

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
