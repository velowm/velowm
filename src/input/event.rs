use x11::xlib;

pub enum Event {
    KeyPress(xlib::XKeyEvent),
    KeyRelease(xlib::XKeyEvent),
    ButtonPress(xlib::XButtonEvent),
    ButtonRelease(xlib::XButtonEvent),
    MotionNotify(xlib::XMotionEvent),
}
