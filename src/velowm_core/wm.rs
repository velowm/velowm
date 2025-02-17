use anyhow::Result;
use log::{debug, error, info, warn};
use std::process::Command as ProcessCommand;
use x11::{xinerama, xlib};

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
    dragging: bool,
    drag_start_x: i32,
    drag_start_y: i32,
    dragged_window: Option<xlib::Window>,
    resizing: bool,
    resize_start_width: u32,
    resize_start_height: u32,
    resized_window: Option<xlib::Window>,
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
            xlib::XDefineCursor(display.raw(), root, cursor.normal());

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
            dragging: false,
            drag_start_x: 0,
            drag_start_y: 0,
            dragged_window: None,
            resizing: false,
            resize_start_width: 0,
            resize_start_height: 0,
            resized_window: None,
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
                    debug!(
                        "Button press: window={}, button={}, state={}",
                        button_event.window, button_event.button, button_event.state
                    );
                    if button_event.window == self.status_bar.window {
                        if let Some(workspace) = self
                            .status_bar
                            .get_clicked_workspace(button_event.x, button_event.y)
                        {
                            self.switch_to_workspace(workspace);
                        }
                    } else if button_event.state & self.config.get_modifier() != 0 {
                        match button_event.button {
                            1 => self.start_window_drag(button_event),
                            3 => self.start_window_resize(button_event),
                            _ => (),
                        }
                    }
                }
                xlib::ButtonRelease => {
                    if self.dragging {
                        self.end_window_drag();
                    } else if self.resizing {
                        self.end_window_resize();
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

    fn raise_floating_windows(&mut self) {
        if let Some(workspace) = self.workspaces.get(self.current_workspace) {
            for window in &workspace.windows {
                if window.is_floating {
                    unsafe {
                        xlib::XRaiseWindow(self.display.raw(), window.id);
                    }
                }
            }
        }
    }

    fn handle_motion_notify(&mut self, _event: xlib::XEvent) {
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

            if self.dragging {
                if let Some(dragged) = self.dragged_window {
                    let dx = root_x - self.drag_start_x;
                    let dy = root_y - self.drag_start_y;

                    if let Some(workspace) = self.workspaces.get_mut(self.current_workspace) {
                        if let Some(window) = workspace.windows.iter_mut().find(|w| w.id == dragged)
                        {
                            if window.is_floating {
                                let new_x = window.pre_float_x + dx;
                                let new_y = window.pre_float_y + dy;
                                window.x = new_x;
                                window.y = new_y;
                                xlib::XMoveWindow(self.display.raw(), window.id, new_x, new_y);
                                xlib::XRaiseWindow(self.display.raw(), window.id);
                                return;
                            }
                        }
                    }

                    debug!("Dragging window {} over window {}", dragged, child_return);
                    if let Some(target) = child_return.checked_sub(0).filter(|_| {
                        child_return != dragged
                            && child_return != 0
                            && child_return != self.layout.get_root()
                    }) {
                        debug!("Swapping windows {} and {}", dragged, target);
                        self.layout.swap_windows(dragged, target);
                        self.layout.relayout();
                        xlib::XSync(self.display.raw(), 0);
                        self.raise_floating_windows();
                    }
                }
            } else if self.resizing {
                if let Some(resized) = self.resized_window {
                    let dx = root_x - self.drag_start_x;
                    let dy = root_y - self.drag_start_y;

                    if let Some(workspace) = self.workspaces.get_mut(self.current_workspace) {
                        if let Some(window) = workspace.windows.iter_mut().find(|w| w.id == resized)
                        {
                            if window.is_floating {
                                let new_width =
                                    ((self.resize_start_width as i32 + dx) as u32).max(100);
                                let new_height =
                                    ((self.resize_start_height as i32 + dy) as u32).max(100);
                                window.width = new_width;
                                window.height = new_height;
                                xlib::XResizeWindow(
                                    self.display.raw(),
                                    window.id,
                                    new_width,
                                    new_height,
                                );
                                xlib::XRaiseWindow(self.display.raw(), window.id);
                            }
                        }
                    }
                }
            } else if child_return != 0 && child_return != self.layout.get_root() {
                self.layout.focus_window(child_return);
                self.raise_floating_windows();
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
                    Command::Close => self.close_focused_window(),
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
                    Command::Workspace(idx) => self.switch_to_workspace(*idx),
                    Command::ToggleFloat => self.toggle_float(),
                }
            }
        }
    }

    fn toggle_float(&mut self) {
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
                if let Some(workspace) = self.workspaces.get_mut(self.current_workspace) {
                    if let Some(window) =
                        workspace.windows.iter_mut().find(|w| w.id == child_return)
                    {
                        if window.is_floating {
                            window.is_floating = false;
                            window.x = window.pre_float_x;
                            window.y = window.pre_float_y;
                            window.width = window.pre_float_width;
                            window.height = window.pre_float_height;
                            self.layout.add_window(window.id);
                            self.layout.relayout();
                        } else {
                            let mut win_attrs: xlib::XWindowAttributes = std::mem::zeroed();
                            xlib::XGetWindowAttributes(
                                self.display.raw(),
                                window.id,
                                &mut win_attrs,
                            );

                            let mut child_x: i32 = 0;
                            let mut child_y: i32 = 0;
                            let mut child: xlib::Window = 0;
                            xlib::XTranslateCoordinates(
                                self.display.raw(),
                                window.id,
                                self.layout.get_root(),
                                0,
                                0,
                                &mut child_x,
                                &mut child_y,
                                &mut child,
                            );

                            window.is_floating = true;
                            window.pre_float_x = child_x;
                            window.pre_float_y = child_y;
                            window.pre_float_width = window.width;
                            window.pre_float_height = window.height;

                            if self.config.appearance.floating.center_on_float {
                                let float_width = self.config.appearance.floating.width;
                                let float_height = self.config.appearance.floating.height;
                                let bar_height = if self.config.appearance.bar.enabled {
                                    self.config.appearance.bar.height
                                } else {
                                    0
                                };

                                let mut num_monitors = 0;
                                let monitors = xinerama::XineramaQueryScreens(
                                    self.display.raw(),
                                    &mut num_monitors,
                                );

                                if !monitors.is_null() && num_monitors > 0 {
                                    let monitors_slice =
                                        std::slice::from_raw_parts(monitors, num_monitors as usize);
                                    let current_monitor = monitors_slice
                                        .iter()
                                        .find(|monitor| {
                                            root_x >= monitor.x_org as i32
                                                && root_x
                                                    < monitor.x_org as i32 + monitor.width as i32
                                                && root_y >= monitor.y_org as i32
                                                && root_y
                                                    < monitor.y_org as i32 + monitor.height as i32
                                        })
                                        .unwrap_or(&monitors_slice[0]);

                                    let new_x = current_monitor.x_org as i32
                                        + ((current_monitor.width as u32 - float_width) / 2) as i32;
                                    let new_y = current_monitor.y_org as i32
                                        + ((current_monitor.height as u32 - float_height) / 2)
                                            as i32
                                        + bar_height as i32;

                                    window.width = float_width;
                                    window.height = float_height;
                                    window.x = new_x;
                                    window.y = new_y;

                                    window.pre_float_x = new_x;
                                    window.pre_float_y = new_y;

                                    xlib::XFree(monitors as *mut _);
                                } else {
                                    let screen_width = xlib::XDisplayWidth(
                                        self.display.raw(),
                                        xlib::XDefaultScreen(self.display.raw()),
                                    ) as u32;
                                    let screen_height = xlib::XDisplayHeight(
                                        self.display.raw(),
                                        xlib::XDefaultScreen(self.display.raw()),
                                    )
                                        as u32;

                                    let new_x = ((screen_width - float_width) / 2) as i32;
                                    let new_y =
                                        ((screen_height - float_height) / 2 + bar_height) as i32;

                                    window.width = float_width;
                                    window.height = float_height;
                                    window.x = new_x;
                                    window.y = new_y;

                                    window.pre_float_x = new_x;
                                    window.pre_float_y = new_y;
                                }

                                xlib::XMoveResizeWindow(
                                    self.display.raw(),
                                    window.id,
                                    window.x,
                                    window.y,
                                    window.width,
                                    window.height,
                                );
                            }

                            self.layout.remove_window(window.id);
                            self.layout.relayout();
                            xlib::XRaiseWindow(self.display.raw(), window.id);
                        }
                    }
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

            debug!("Grabbing buttons for window {}", window_id);
            xlib::XGrabButton(
                self.display.raw(),
                1,
                self.config.get_modifier(),
                window_id,
                1,
                (xlib::ButtonPressMask | xlib::ButtonReleaseMask | xlib::PointerMotionMask) as u32,
                xlib::GrabModeAsync,
                xlib::GrabModeAsync,
                0,
                0,
            );
            xlib::XGrabButton(
                self.display.raw(),
                3,
                self.config.get_modifier(),
                window_id,
                1,
                (xlib::ButtonPressMask | xlib::ButtonReleaseMask | xlib::PointerMotionMask) as u32,
                xlib::GrabModeAsync,
                xlib::GrabModeAsync,
                0,
                0,
            );
        }

        let window = Window::new(
            window_id,
            attrs.x,
            attrs.y,
            attrs.width as u32,
            attrs.height as u32,
        );

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
        if !self.dragging && enter_event.window != 0 && enter_event.window != self.layout.get_root()
        {
            self.layout.focus_window(enter_event.window);
            self.raise_floating_windows();
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
                    xlib::XGrabButton(
                        self.display.raw(),
                        1,
                        self.config.get_modifier(),
                        window.id,
                        1,
                        (xlib::ButtonPressMask | xlib::ButtonReleaseMask | xlib::PointerMotionMask)
                            as u32,
                        xlib::GrabModeAsync,
                        xlib::GrabModeAsync,
                        0,
                        0,
                    );
                }
                self.layout.add_window(window.id);
            }
            if let Some(focused) = new.get_focused_window() {
                self.layout.focus_window(focused.id);
            }
            self.raise_floating_windows();
        }

        self.layout.relayout();
        unsafe {
            self.status_bar.draw(self.current_workspace);
            xlib::XSync(self.display.raw(), 0);
        }
    }

    fn start_window_drag(&mut self, event: xlib::XButtonEvent) {
        debug!("Starting window drag for window {}", event.window);
        self.dragging = true;
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

            self.drag_start_x = root_x;
            self.drag_start_y = root_y;
            self.dragged_window = Some(event.window);

            debug!("Setting grabbing cursor for window {}", event.window);
            xlib::XDefineCursor(self.display.raw(), event.window, self.cursor.grabbing());
            xlib::XSync(self.display.raw(), 0);
        }
    }

    fn end_window_drag(&mut self) {
        if let Some(window) = self.dragged_window {
            debug!("Ending window drag for window {}", window);
            unsafe {
                debug!("Resetting cursor for window {}", window);
                xlib::XDefineCursor(self.display.raw(), window, self.cursor.normal());
                if let Some(workspace) = self.workspaces.get_mut(self.current_workspace) {
                    if let Some(win) = workspace.windows.iter_mut().find(|w| w.id == window) {
                        if win.is_floating {
                            self.drag_start_x = 0;
                            self.drag_start_y = 0;
                            win.pre_float_x = win.x;
                            win.pre_float_y = win.y;
                        }
                    }
                }
                xlib::XSync(self.display.raw(), 0);
            }
        }
        self.dragging = false;
        self.dragged_window = None;
    }

    fn start_window_resize(&mut self, event: xlib::XButtonEvent) {
        debug!("Starting window resize for window {}", event.window);
        self.resizing = true;
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

            if let Some(workspace) = self.workspaces.get(self.current_workspace) {
                if let Some(window) = workspace.windows.iter().find(|w| w.id == event.window) {
                    self.resize_start_width = window.width;
                    self.resize_start_height = window.height;
                    self.drag_start_x = root_x;
                    self.drag_start_y = root_y;
                    self.resized_window = Some(event.window);

                    debug!("Setting grabbing cursor for window {}", event.window);
                    xlib::XDefineCursor(self.display.raw(), event.window, self.cursor.grabbing());
                    xlib::XSync(self.display.raw(), 0);
                }
            }
        }
    }

    fn end_window_resize(&mut self) {
        if let Some(window) = self.resized_window {
            debug!("Ending window resize for window {}", window);
            unsafe {
                debug!("Resetting cursor for window {}", window);
                xlib::XDefineCursor(self.display.raw(), window, self.cursor.normal());
                if let Some(workspace) = self.workspaces.get_mut(self.current_workspace) {
                    if let Some(win) = workspace.windows.iter_mut().find(|w| w.id == window) {
                        if win.is_floating {
                            win.pre_float_width = win.width;
                            win.pre_float_height = win.height;
                        }
                    }
                }
                xlib::XSync(self.display.raw(), 0);
            }
        }
        self.resizing = false;
        self.resized_window = None;
    }
}
