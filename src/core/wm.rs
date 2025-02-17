use anyhow::Result;
use log::{debug, error, info, warn};
use std::process::Command as ProcessCommand;
use x11::xlib;

use crate::{
    config::{command::Command, loader::Config},
    ui::{
        bar::StatusBar, cursor::Cursor, layout::MasterStackLayout, notification::NotificationWindow,
    },
    utils::x11::Display,
};

use super::{window::Window, workspace::Workspace};

pub struct WindowManager {
    display: Display,
    running: bool,
    #[allow(dead_code)]
    cursor: Cursor,
    config: Config,
    layout: MasterStackLayout,
    notification: NotificationWindow,
    workspaces: Vec<Workspace>,
    current_workspace: usize,
    status_bar: StatusBar,
}

impl WindowManager {
    pub fn new() -> Result<Self> {
        info!("Initializing window manager");

        let display = Display::new()?;
        let root = unsafe { xlib::XDefaultRootWindow(display.raw()) };
        let cursor = unsafe { Cursor::new(display.raw())? };

        let config = Config::load().unwrap_or_else(|_| {
            warn!("Failed to load config, using default configuration");
            Config::default()
        });

        let layout = unsafe { MasterStackLayout::new(display.raw(), root, config.clone()) };
        let notification = unsafe { NotificationWindow::new(display.raw(), root) };

        if let Err(e) = Config::load() {
            error!("Failed to load config: {}", e);
            unsafe {
                notification.show_error(&format!("Failed to load config: {}", e));
            }
        }

        let screen_width = unsafe {
            xlib::XDisplayWidth(display.raw(), xlib::XDefaultScreen(display.raw())) as u32
        };
        let status_bar = unsafe {
            StatusBar::new(
                display.raw(),
                root,
                screen_width,
                config.appearance.bar.clone(),
            )
        };

        unsafe {
            xlib::XDefineCursor(display.raw(), root, cursor.raw());

            Self::setup_key_bindings(display.raw(), root, &config);

            xlib::XSelectInput(
                display.raw(),
                root,
                xlib::SubstructureRedirectMask
                    | xlib::SubstructureNotifyMask
                    | xlib::PointerMotionMask,
            );

            xlib::XSync(display.raw(), 0);
        }

        let mut workspaces = Vec::with_capacity(10);
        for i in 0..10 {
            workspaces.push(Workspace::new(i));
        }

        Ok(Self {
            display,
            running: true,
            cursor,
            config,
            layout,
            notification,
            workspaces,
            current_workspace: 0,
            status_bar,
        })
    }

    unsafe fn setup_key_bindings(display: *mut xlib::Display, root: xlib::Window, config: &Config) {
        for bind in &config.binds {
            let keycode = xlib::XKeysymToKeycode(display, config.get_keysym_for_key(&bind.key));
            xlib::XGrabKey(
                display,
                keycode as i32,
                config.get_modifier(),
                root,
                1,
                xlib::GrabModeAsync,
                xlib::GrabModeAsync,
            );
        }
        xlib::XSync(display, 0);
    }

    pub fn run(&mut self) -> Result<()> {
        unsafe {
            self.status_bar.draw(self.current_workspace);
        }

        while self.running {
            let mut event: xlib::XEvent = unsafe { std::mem::zeroed() };
            unsafe {
                xlib::XNextEvent(self.display.raw(), &mut event);
            }

            match event.get_type() {
                xlib::KeyPress => self.handle_keypress(event),
                xlib::MapRequest => self.handle_map_request(event),
                xlib::UnmapNotify => self.handle_unmap_notify(event),
                xlib::DestroyNotify => self.handle_destroy_notify(event),
                xlib::MotionNotify => self.handle_motion_notify(event),
                xlib::ButtonPress => {
                    let button_event: xlib::XButtonEvent = From::from(event);
                    if button_event.window == self.status_bar.window {
                        if let Some(workspace) = self
                            .status_bar
                            .get_clicked_workspace(button_event.x, button_event.y)
                        {
                            self.switch_to_workspace(workspace);
                        }
                    }
                }
                xlib::EnterNotify => self.handle_enter_notify(event),
                xlib::LeaveNotify => self.handle_leave_notify(event),
                xlib::Expose => {
                    let expose_event: xlib::XExposeEvent = From::from(event);
                    if expose_event.window == self.notification.window {
                        // TODO: Store last error message and redraw it here
                    } else if expose_event.window == self.status_bar.window {
                        unsafe {
                            self.status_bar.draw(self.current_workspace);
                        }
                    }
                }
                _ => (),
            }
        }

        Ok(())
    }

    fn handle_motion_notify(&mut self, event: xlib::XEvent) {
        let _motion_event: xlib::XMotionEvent = From::from(event);
        unsafe {
            let mut root_return: xlib::Window = 0;
            let mut child_return: xlib::Window = 0;
            let mut root_x: i32 = 0;
            let mut root_y: i32 = 0;
            let mut win_x: i32 = 0;
            let mut win_y: i32 = 0;
            let mut mask_return: u32 = 0;

            xlib::XQueryPointer(
                self.display.raw(),
                self.layout.get_root(),
                &mut root_return,
                &mut child_return,
                &mut root_x,
                &mut root_y,
                &mut win_x,
                &mut win_y,
                &mut mask_return,
            );

            if child_return != 0 && child_return != self.layout.get_root() {
                self.layout.focus_window(child_return);
            }
        }
    }

    fn handle_keypress(&mut self, event: xlib::XEvent) {
        let key_event: xlib::XKeyEvent = From::from(event);

        let binds = self.config.binds.clone();
        for bind in &binds {
            let keycode = unsafe {
                xlib::XKeysymToKeycode(
                    self.display.raw(),
                    self.config.get_keysym_for_key(&bind.key),
                )
            };

            if key_event.state & self.config.get_modifier() != 0
                && key_event.keycode as u8 == keycode
            {
                match &bind.command {
                    Command::Exit => self.running = false,
                    Command::Spawn(cmd) => {
                        if let Err(e) = ProcessCommand::new(cmd)
                            .stdout(std::process::Stdio::null())
                            .stderr(std::process::Stdio::null())
                            .spawn()
                        {
                            unsafe {
                                self.notification
                                    .show_error(&format!("Failed to spawn {}: {}", cmd, e));
                            }
                        }
                    }
                    Command::Close => self.close_focused_window(),
                    Command::Workspace(idx) => self.switch_to_workspace(*idx),
                }
            }
        }
    }

    fn close_focused_window(&mut self) {
        debug!("Attempting to close focused window");
        unsafe {
            let mut root_return: xlib::Window = 0;
            let mut child_return: xlib::Window = 0;
            let mut root_x: i32 = 0;
            let mut root_y: i32 = 0;
            let mut win_x: i32 = 0;
            let mut win_y: i32 = 0;
            let mut mask_return: u32 = 0;

            xlib::XQueryPointer(
                self.display.raw(),
                self.layout.get_root(),
                &mut root_return,
                &mut child_return,
                &mut root_x,
                &mut root_y,
                &mut win_x,
                &mut win_y,
                &mut mask_return,
            );

            if child_return != 0 && child_return != self.layout.get_root() {
                let wm_protocols =
                    xlib::XInternAtom(self.display.raw(), c"WM_PROTOCOLS".as_ptr(), 0);
                let wm_delete_window =
                    xlib::XInternAtom(self.display.raw(), c"WM_DELETE_WINDOW".as_ptr(), 0);

                let mut protocols: *mut xlib::Atom = std::ptr::null_mut();
                let mut num_protocols: i32 = 0;

                if xlib::XGetWMProtocols(
                    self.display.raw(),
                    child_return,
                    &mut protocols,
                    &mut num_protocols,
                ) != 0
                {
                    let protocols_slice =
                        std::slice::from_raw_parts(protocols, num_protocols as usize);
                    if protocols_slice.contains(&wm_delete_window) {
                        let mut data: xlib::ClientMessageData = std::mem::zeroed();
                        data.set_long(0, wm_delete_window as i64);

                        let mut event = xlib::XEvent {
                            client_message: xlib::XClientMessageEvent {
                                type_: xlib::ClientMessage,
                                serial: 0,
                                send_event: 1,
                                display: self.display.raw(),
                                window: child_return,
                                message_type: wm_protocols,
                                format: 32,
                                data,
                            },
                        };
                        xlib::XSendEvent(self.display.raw(), child_return, 0, 0, &mut event);
                    } else {
                        xlib::XDestroyWindow(self.display.raw(), child_return);
                    }
                    xlib::XFree(protocols as *mut _);
                } else {
                    xlib::XDestroyWindow(self.display.raw(), child_return);
                }
                xlib::XSync(self.display.raw(), 0);
            }
        }
    }

    fn handle_map_request(&mut self, event: xlib::XEvent) {
        let map_event: xlib::XMapRequestEvent = From::from(event);
        let window_id = map_event.window;
        debug!("Handling map request for window {}", window_id);

        let mut attrs: xlib::XWindowAttributes = unsafe { std::mem::zeroed() };
        unsafe {
            xlib::XGetWindowAttributes(self.display.raw(), window_id, &mut attrs);
        }

        let window = Window {
            id: window_id,
            x: attrs.x,
            y: attrs.y,
            width: attrs.width as u32,
            height: attrs.height as u32,
        };

        if let Some(workspace) = self.workspaces.get_mut(self.current_workspace) {
            workspace.add_window(window);
            unsafe {
                xlib::XMapWindow(self.display.raw(), window_id);
                xlib::XSetWindowBorderWidth(
                    self.display.raw(),
                    window_id,
                    self.config.appearance.border_width,
                );
            }
            self.layout.add_window(window_id);
            unsafe {
                xlib::XSync(self.display.raw(), 0);
            }
        }
    }

    fn handle_unmap_notify(&mut self, event: xlib::XEvent) {
        let unmap_event: xlib::XUnmapEvent = From::from(event);
        if let Some(workspace) = self.workspaces.get_mut(self.current_workspace) {
            workspace.remove_window(unmap_event.window);
        }
        self.layout.remove_window(unmap_event.window);
    }

    fn handle_destroy_notify(&mut self, event: xlib::XEvent) {
        let destroy_event: xlib::XDestroyWindowEvent = From::from(event);
        if let Some(workspace) = self.workspaces.get_mut(self.current_workspace) {
            workspace.remove_window(destroy_event.window);
        }
        self.layout.remove_window(destroy_event.window);
    }

    fn handle_enter_notify(&mut self, event: xlib::XEvent) {
        let enter_event: xlib::XCrossingEvent = From::from(event);
        if enter_event.window != 0 && enter_event.window != self.layout.get_root() {
            self.layout.focus_window(enter_event.window);
        }
    }

    fn handle_leave_notify(&mut self, _event: xlib::XEvent) {
        // we don't need to do anything on leave, as entering a new window will handle focus. in the future we might want
        // a config option to disable automatic focus changing, for now leave this empty. If automatic focus changing is
        // disabled then allow clicking on windows to change focus.
    }

    fn switch_to_workspace(&mut self, index: usize) {
        if index >= self.workspaces.len() || index == self.current_workspace {
            debug!("Invalid workspace switch request to {}", index);
            return;
        }

        info!("Switching to workspace {}", index);
        if let Some(current) = self.workspaces.get(self.current_workspace) {
            for window in &current.windows {
                unsafe {
                    xlib::XUnmapWindow(self.display.raw(), window.id);
                }
            }
        }

        self.current_workspace = index;
        self.layout.clear_windows();

        if let Some(new) = self.workspaces.get(self.current_workspace) {
            for window in &new.windows {
                unsafe {
                    xlib::XMapWindow(self.display.raw(), window.id);
                    xlib::XSetWindowBorderWidth(
                        self.display.raw(),
                        window.id,
                        self.config.appearance.border_width,
                    );
                }
                self.layout.add_window(window.id);
            }
            if let Some(focused) = new.get_focused_window() {
                self.layout.focus_window(focused.id);
            }
        }

        self.layout.relayout();
        unsafe {
            self.status_bar.draw(self.current_workspace);
            xlib::XSync(self.display.raw(), 0);
        }
    }
}
