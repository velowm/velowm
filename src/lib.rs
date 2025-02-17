pub mod core {
    pub mod window;
    pub mod wm;
    pub mod workspace;
}

pub mod utils {
    pub mod command;
    pub mod keybind;
    pub mod x11;
}

pub mod input {
    pub mod event;
    pub mod keyboard;
    pub mod mouse;
}

pub mod ui {
    pub mod appearance;
    pub mod bar;
    pub mod cursor;
    pub mod layout;
    pub mod notification;
}

pub mod config {
    pub mod appearance;
    pub mod command;
    pub mod keybind;
    pub mod loader;
}

pub use config::loader::Config;
pub use core::{window::Window, wm::WindowManager};
