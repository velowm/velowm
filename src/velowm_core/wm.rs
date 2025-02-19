use anyhow::Result;
use log::{debug, error, info, warn};
use std::process::Command as ProcessCommand;
use x11::{xinerama, xlib};

use crate::{
    config::loader::Config,
    ui::{cursor::Cursor, layout::MasterStackLayout, notification::NotificationManager},
    utils::{command::Command, x11::Display},
};

use super::{window::Window, workspace::Workspace};

pub struct WindowManager {
    display: Display,
    running: bool,
    #[allow(dead_code)]
    cursor: Cursor,
    config: Config,
    layout: MasterStackLayout,
    notification_manager: NotificationManager,
    workspaces: Vec<Workspace>,
    current_workspace: usize,
    dragging: bool,
    drag_start_x: i32,
    drag_start_y: i32,
    dragged_window: Option<xlib::Window>,
    resizing: bool,
    resize_start_width: u32,
    resize_start_height: u32,
    resized_window: Option<xlib::Window>,
    net_active_window: xlib::Atom,
    net_current_desktop: xlib::Atom,
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
        let mut notification_manager = unsafe { NotificationManager::new(display.raw(), root) };

        if let Err(e) = Config::load() {
            error!("Failed to load config: {}", e);
            if config.notifications_enabled {
                unsafe {
                    notification_manager.show_error(&format!("Failed to load config: {}", e));
                }
            }
        }

        if config.auto_generated && config.notifications_enabled {
            unsafe {
                notification_manager.show_error(
                    "You are using an auto generated config\n\
                    \n\
                    Press Alt+Q to open alacritty\n\
                    Press Alt+W to exit\n\
                    \n\
                    Press on this to dismiss this message",
                );
            }
        }

        let (net_active_window, net_current_desktop) = unsafe {
            let net_active_window =
                xlib::XInternAtom(display.raw(), c"_NET_ACTIVE_WINDOW".as_ptr(), 0);
            let net_current_desktop =
                xlib::XInternAtom(display.raw(), c"_NET_CURRENT_DESKTOP".as_ptr(), 0);
            let net_number_of_desktops =
                xlib::XInternAtom(display.raw(), c"_NET_NUMBER_OF_DESKTOPS".as_ptr(), 0);
            let net_desktop_names =
                xlib::XInternAtom(display.raw(), c"_NET_DESKTOP_NAMES".as_ptr(), 0);
            let net_supported = xlib::XInternAtom(display.raw(), c"_NET_SUPPORTED".as_ptr(), 0);

            let supported_atoms = [
                net_active_window,
                net_current_desktop,
                net_number_of_desktops,
                net_desktop_names,
            ];

            xlib::XChangeProperty(
                display.raw(),
                root,
                net_supported,
                xlib::XA_ATOM,
                32,
                xlib::PropModeReplace,
                supported_atoms.as_ptr() as *const u8,
                supported_atoms.len() as i32,
            );

            let num_desktops: u32 = 10;
            xlib::XChangeProperty(
                display.raw(),
                root,
                net_number_of_desktops,
                xlib::XA_CARDINAL,
                32,
                xlib::PropModeReplace,
                &num_desktops as *const u32 as *const u8,
                1,
            );

            let current_desktop: u32 = 0;
            xlib::XChangeProperty(
                display.raw(),
                root,
                net_current_desktop,
                xlib::XA_CARDINAL,
                32,
                xlib::PropModeReplace,
                &current_desktop as *const u32 as *const u8,
                1,
            );

            let names = (0..10)
                .map(|i| format!("Workspace {}", i + 1))
                .collect::<Vec<_>>();
            let names_str = names.join("\0") + "\0";
            xlib::XChangeProperty(
                display.raw(),
                root,
                net_desktop_names,
                xlib::XInternAtom(display.raw(), c"UTF8_STRING".as_ptr(), 0),
                8,
                xlib::PropModeReplace,
                names_str.as_bytes().as_ptr(),
                names_str.len() as i32,
            );

            (net_active_window, net_current_desktop)
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
            notification_manager,
            workspaces,
            current_workspace: 0,
            dragging: false,
            drag_start_x: 0,
            drag_start_y: 0,
            dragged_window: None,
            resizing: false,
            resize_start_width: 0,
            resize_start_height: 0,
            resized_window: None,
            net_active_window,
            net_current_desktop,
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
                    self.handle_button_press(button_event);
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
                    self.handle_expose(expose_event);
                }
                xlib::ClientMessage => self.handle_client_message(event),
                _ => (),
            }
        }

        Ok(())
    }

    fn raise_floating_windows(&mut self) {
        if let Some(workspace) = self.workspaces.get(self.current_workspace) {
            for window in &workspace.windows {
                if window.is_floating && !window.is_dock {
                    unsafe {
                        xlib::XRaiseWindow(self.display.raw(), window.id);
                    }
                }
            }

            for window in &workspace.windows {
                if window.is_dock {
                    unsafe {
                        xlib::XRaiseWindow(self.display.raw(), window.id);
                    }
                }
            }
        }

        unsafe {
            self.notification_manager.raise_all();
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
                            if self.config.notifications_enabled {
                                unsafe {
                                    self.notification_manager
                                        .show_error(&format!("Failed to spawn {}: {}", cmd, e));
                                }
                            }
                        }
                    }
                    Command::Workspace(idx) => self.switch_to_workspace(*idx),
                    Command::ToggleFloat => self.toggle_float(),
                    Command::ToggleFullscreen => self.toggle_fullscreen(),
                }
            }
        }
    }

    fn toggle_float(&mut self) {
        unsafe {
            let mut focused_win: xlib::Window = 0;
            let mut revert_to: i32 = 0;
            xlib::XGetInputFocus(self.display.raw(), &mut focused_win, &mut revert_to);

            let mut actual_type: xlib::Atom = 0;
            let mut actual_format: i32 = 0;
            let mut nitems: u64 = 0;
            let mut bytes_after: u64 = 0;
            let mut data: *mut xlib::Window = std::ptr::null_mut();

            let root = xlib::XDefaultRootWindow(self.display.raw());
            xlib::XGetWindowProperty(
                self.display.raw(),
                root,
                self.net_active_window,
                0,
                1,
                0,
                xlib::XA_WINDOW,
                &mut actual_type,
                &mut actual_format,
                &mut nitems,
                &mut bytes_after,
                &mut data as *mut *mut xlib::Window as *mut *mut u8,
            );

            let net_active_win = if !data.is_null() && nitems > 0 {
                let win = *data;
                xlib::XFree(data as *mut _);
                win
            } else {
                0
            };

            let window_id = if focused_win != 0 && focused_win != self.layout.get_root() {
                focused_win
            } else if net_active_win != 0 && net_active_win != self.layout.get_root() {
                net_active_win
            } else if let Some(workspace) = self.workspaces.get(self.current_workspace) {
                workspace.get_focused_window().map(|w| w.id).unwrap_or(0)
            } else {
                0
            };

            if window_id != 0 {
                let (is_floating, should_update) = if let Some(workspace) =
                    self.workspaces.get_mut(self.current_workspace)
                {
                    let is_floating = workspace
                        .windows
                        .iter()
                        .find(|w| w.id == window_id)
                        .map(|w| w.is_floating)
                        .unwrap_or(false);

                    if is_floating {
                        if let Some(window) =
                            workspace.windows.iter_mut().find(|w| w.id == window_id)
                        {
                            window.is_floating = false;
                            window.x = window.pre_float_x;
                            window.y = window.pre_float_y;
                            window.width = window.pre_float_width;
                            window.height = window.pre_float_height;
                        }
                        (false, true)
                    } else {
                        if let Some(window) =
                            workspace.windows.iter_mut().find(|w| w.id == window_id)
                        {
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
                                let mut num_monitors = 0;
                                let monitors = xinerama::XineramaQueryScreens(
                                    self.display.raw(),
                                    &mut num_monitors,
                                );

                                if !monitors.is_null() && num_monitors > 0 {
                                    let monitors_slice =
                                        std::slice::from_raw_parts(monitors, num_monitors as usize);

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
                                            as i32;

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
                                    let new_y = ((screen_height - float_height) / 2) as i32;

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
                        }
                        (true, true)
                    }
                } else {
                    (false, false)
                };

                if should_update {
                    if !is_floating {
                        self.layout.add_window(window_id);
                        self.layout.relayout();
                    } else {
                        self.layout.remove_window(window_id);
                        self.layout.relayout();
                    }

                    xlib::XSetInputFocus(
                        self.display.raw(),
                        window_id,
                        xlib::RevertToPointerRoot,
                        xlib::CurrentTime,
                    );
                    self.set_active_window(window_id);

                    if let Some(workspace) = self.workspaces.get(self.current_workspace) {
                        for window in &workspace.windows {
                            let border_color = if window.id == window_id {
                                self.config.get_focused_border_color()
                            } else {
                                self.config.get_border_color()
                            };
                            xlib::XSetWindowBorder(self.display.raw(), window.id, border_color);
                        }
                    }

                    if is_floating {
                        xlib::XRaiseWindow(self.display.raw(), window_id);
                    }

                    self.raise_floating_windows();
                    xlib::XSync(self.display.raw(), 0);
                }
            }
        }
    }

    fn toggle_fullscreen(&mut self) {
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
                        let mut num_monitors = 0;
                        let monitors =
                            xinerama::XineramaQueryScreens(self.display.raw(), &mut num_monitors);

                        if !monitors.is_null() && num_monitors > 0 {
                            let monitors_slice =
                                std::slice::from_raw_parts(monitors, num_monitors as usize);
                            let current_monitor = monitors_slice
                                .iter()
                                .find(|monitor| {
                                    root_x >= monitor.x_org as i32
                                        && root_x < monitor.x_org as i32 + monitor.width as i32
                                        && root_y >= monitor.y_org as i32
                                        && root_y < monitor.y_org as i32 + monitor.height as i32
                                })
                                .unwrap_or(&monitors_slice[0]);

                            if window.is_fullscreen {
                                window.is_fullscreen = false;
                                window.x = window.pre_fullscreen_x;
                                window.y = window.pre_fullscreen_y;
                                window.width = window.pre_fullscreen_width;
                                window.height = window.pre_fullscreen_height;
                                xlib::XSetWindowBorderWidth(
                                    self.display.raw(),
                                    window.id,
                                    window.pre_fullscreen_border_width,
                                );
                                if window.is_floating {
                                    xlib::XMoveResizeWindow(
                                        self.display.raw(),
                                        window.id,
                                        window.x,
                                        window.y,
                                        window.width,
                                        window.height,
                                    );
                                } else {
                                    self.layout.relayout();
                                }
                            } else {
                                let mut attrs: xlib::XWindowAttributes = std::mem::zeroed();
                                xlib::XGetWindowAttributes(
                                    self.display.raw(),
                                    window.id,
                                    &mut attrs,
                                );

                                window.is_fullscreen = true;
                                window.pre_fullscreen_x = attrs.x;
                                window.pre_fullscreen_y = attrs.y;
                                window.pre_fullscreen_width = attrs.width as u32;
                                window.pre_fullscreen_height = attrs.height as u32;
                                window.pre_fullscreen_border_width = attrs.border_width as u32;

                                window.x = current_monitor.x_org as i32;
                                window.y = current_monitor.y_org as i32;
                                window.width = current_monitor.width as u32;
                                window.height = current_monitor.height as u32;

                                xlib::XSetWindowBorderWidth(self.display.raw(), window.id, 0);
                                xlib::XMoveResizeWindow(
                                    self.display.raw(),
                                    window.id,
                                    window.x,
                                    window.y,
                                    window.width,
                                    window.height,
                                );
                                xlib::XRaiseWindow(self.display.raw(), window.id);
                            }

                            xlib::XFree(monitors as *mut _);
                        }
                    }
                }
            }
        }
    }

    fn close_focused_window(&mut self) {
        debug!("Attempting to close focused window");
        unsafe {
            let (focused_window, _was_floating, next_window) = {
                let workspace = self.workspaces.get(self.current_workspace);

                let mut focused_win: xlib::Window = 0;
                let mut revert_to: i32 = 0;
                xlib::XGetInputFocus(self.display.raw(), &mut focused_win, &mut revert_to);

                let mut actual_type: xlib::Atom = 0;
                let mut actual_format: i32 = 0;
                let mut nitems: u64 = 0;
                let mut bytes_after: u64 = 0;
                let mut data: *mut xlib::Window = std::ptr::null_mut();

                let root = xlib::XDefaultRootWindow(self.display.raw());
                xlib::XGetWindowProperty(
                    self.display.raw(),
                    root,
                    self.net_active_window,
                    0,
                    1,
                    0,
                    xlib::XA_WINDOW,
                    &mut actual_type,
                    &mut actual_format,
                    &mut nitems,
                    &mut bytes_after,
                    &mut data as *mut *mut xlib::Window as *mut *mut u8,
                );

                let net_active_win = if !data.is_null() && nitems > 0 {
                    let win = *data;
                    xlib::XFree(data as *mut _);
                    win
                } else {
                    0
                };

                let (focused_id, is_floating) =
                    if focused_win != 0 && focused_win != self.layout.get_root() {
                        workspace.and_then(|ws| {
                            ws.windows
                                .iter()
                                .find(|w| w.id == focused_win)
                                .map(|w| (w.id, w.is_floating))
                        })
                    } else if net_active_win != 0 && net_active_win != self.layout.get_root() {
                        workspace.and_then(|ws| {
                            ws.windows
                                .iter()
                                .find(|w| w.id == net_active_win)
                                .map(|w| (w.id, w.is_floating))
                        })
                    } else {
                        None
                    }
                    .unwrap_or_else(|| {
                        workspace
                            .and_then(|ws| ws.get_focused_window().map(|w| (w.id, w.is_floating)))
                            .or_else(|| self.layout.get_focused_window().map(|id| (id, false)))
                            .unwrap_or((0, false))
                    });

                let next = if focused_id != 0 {
                    workspace.and_then(|ws| {
                        if is_floating {
                            let next_floating = ws
                                .windows
                                .iter()
                                .filter(|w| w.is_floating && !w.is_dock && w.id != focused_id)
                                .last();

                            next_floating
                                .or_else(|| {
                                    ws.windows
                                        .iter()
                                        .filter(|w| !w.is_floating && !w.is_dock)
                                        .last()
                                })
                                .map(|w| (w.id, w.is_floating))
                        } else {
                            ws.windows
                                .iter()
                                .filter(|w| !w.is_dock)
                                .last()
                                .map(|w| (w.id, w.is_floating))
                        }
                    })
                } else {
                    None
                };

                (focused_id, is_floating, next)
            };

            if focused_window == 0 {
                return;
            }

            if let Some(workspace) = self.workspaces.get(self.current_workspace) {
                if let Some(window) = workspace.windows.iter().find(|w| w.id == focused_window) {
                    if window.is_dock {
                        debug!("Ignoring close request for dock window");
                        return;
                    }
                }
            }

            let wm_protocols = xlib::XInternAtom(self.display.raw(), c"WM_PROTOCOLS".as_ptr(), 0);
            let wm_delete_window =
                xlib::XInternAtom(self.display.raw(), c"WM_DELETE_WINDOW".as_ptr(), 0);

            let mut protocols: *mut xlib::Atom = std::ptr::null_mut();
            let mut num_protocols: i32 = 0;

            if xlib::XGetWMProtocols(
                self.display.raw(),
                focused_window,
                &mut protocols,
                &mut num_protocols,
            ) != 0
            {
                let protocols_slice = std::slice::from_raw_parts(protocols, num_protocols as usize);
                if protocols_slice.contains(&wm_delete_window) {
                    let mut data: xlib::ClientMessageData = std::mem::zeroed();
                    data.set_long(0, wm_delete_window as i64);

                    let mut event = xlib::XEvent {
                        client_message: xlib::XClientMessageEvent {
                            type_: xlib::ClientMessage,
                            serial: 0,
                            send_event: 1,
                            display: self.display.raw(),
                            window: focused_window,
                            message_type: wm_protocols,
                            format: 32,
                            data,
                        },
                    };
                    xlib::XSendEvent(self.display.raw(), focused_window, 0, 0, &mut event);
                } else {
                    xlib::XDestroyWindow(self.display.raw(), focused_window);
                }
                xlib::XFree(protocols as *mut _);
            } else {
                xlib::XDestroyWindow(self.display.raw(), focused_window);
            }

            xlib::XSync(self.display.raw(), 0);

            if let Some((next_id, is_floating)) = next_window {
                if is_floating {
                    xlib::XRaiseWindow(self.display.raw(), next_id);
                }
                self.layout.focus_window(next_id);
                self.set_active_window(next_id);

                if let Some(workspace) = self.workspaces.get(self.current_workspace) {
                    for w in &workspace.windows {
                        let border_color = if w.id == next_id {
                            self.config.get_focused_border_color()
                        } else {
                            self.config.get_border_color()
                        };
                        xlib::XSetWindowBorder(self.display.raw(), w.id, border_color);
                    }
                }
            }
        }
    }

    fn handle_map_request(&mut self, event: xlib::XEvent) {
        let map_event: xlib::XMapRequestEvent = From::from(event);
        let window_id = map_event.window;
        debug!("Handling map request for window {}", window_id);

        let mut attrs: xlib::XWindowAttributes = unsafe { std::mem::zeroed() };
        let is_dock = unsafe {
            xlib::XGetWindowAttributes(self.display.raw(), window_id, &mut attrs);

            let net_wm_window_type =
                xlib::XInternAtom(self.display.raw(), c"_NET_WM_WINDOW_TYPE".as_ptr(), 0);
            let net_wm_window_type_dock =
                xlib::XInternAtom(self.display.raw(), c"_NET_WM_WINDOW_TYPE_DOCK".as_ptr(), 0);

            let mut actual_type: xlib::Atom = 0;
            let mut actual_format: i32 = 0;
            let mut nitems: u64 = 0;
            let mut bytes_after: u64 = 0;
            let mut prop: *mut u8 = std::ptr::null_mut();

            let is_dock = if xlib::XGetWindowProperty(
                self.display.raw(),
                window_id,
                net_wm_window_type,
                0,
                1,
                0,
                xlib::XA_ATOM,
                &mut actual_type,
                &mut actual_format,
                &mut nitems,
                &mut bytes_after,
                &mut prop,
            ) == 0
                && !prop.is_null()
                && nitems > 0
            {
                let atom = *(prop as *const xlib::Atom);
                xlib::XFree(prop as *mut _);
                atom == net_wm_window_type_dock
            } else {
                false
            };

            debug!("Grabbing buttons for window {}", window_id);
            if !is_dock {
                xlib::XGrabButton(
                    self.display.raw(),
                    1,
                    self.config.get_modifier(),
                    window_id,
                    1,
                    (xlib::ButtonPressMask | xlib::ButtonReleaseMask | xlib::PointerMotionMask)
                        as u32,
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
                    (xlib::ButtonPressMask | xlib::ButtonReleaseMask | xlib::PointerMotionMask)
                        as u32,
                    xlib::GrabModeAsync,
                    xlib::GrabModeAsync,
                    0,
                    0,
                );

                if !self.config.appearance.focus_follows_mouse {
                    xlib::XGrabButton(
                        self.display.raw(),
                        xlib::AnyButton as u32,
                        0,
                        window_id,
                        1,
                        (xlib::ButtonPressMask | xlib::ButtonReleaseMask) as u32,
                        xlib::GrabModeSync,
                        xlib::GrabModeAsync,
                        0,
                        0,
                    );
                }
            }
            is_dock
        };

        let mut window = Window::new(
            window_id,
            attrs.x,
            attrs.y,
            attrs.width as u32,
            attrs.height as u32,
        );

        unsafe {
            if is_dock {
                window.is_floating = true;
                window.is_dock = true;

                xlib::XSetWindowBorderWidth(self.display.raw(), window_id, 0);

                for workspace in &mut self.workspaces {
                    workspace.add_window(window.clone());
                }

                xlib::XMapWindow(self.display.raw(), window_id);
                xlib::XRaiseWindow(self.display.raw(), window_id);

                self.layout.update_dock_space(window.y, window.height);
            } else if let Some(workspace) = self.workspaces.get_mut(self.current_workspace) {
                xlib::XMapWindow(self.display.raw(), window_id);
                xlib::XSetWindowBorderWidth(
                    self.display.raw(),
                    window_id,
                    self.config.appearance.border_width,
                );

                workspace.add_window(window);
                self.layout.add_window(window_id);

                for window in &workspace.windows {
                    let border_color = if window.id == window_id {
                        self.config.get_focused_border_color()
                    } else {
                        self.config.get_border_color()
                    };
                    xlib::XSetWindowBorder(self.display.raw(), window.id, border_color);
                }

                self.set_active_window(window_id);
                xlib::XSync(self.display.raw(), 0);
            }
        }

        self.raise_floating_windows();
        unsafe {
            self.notification_manager.raise_all();
            xlib::XSync(self.display.raw(), 0);
        }
    }

    fn handle_unmap_notify(&mut self, event: xlib::XEvent) {
        let unmap_event: xlib::XUnmapEvent = From::from(event);
        if let Some(workspace) = self.workspaces.get_mut(self.current_workspace) {
            workspace.remove_window(unmap_event.window);
        }
        self.layout.remove_window(unmap_event.window);
        self.raise_floating_windows();
        unsafe {
            self.notification_manager.raise_all();
            xlib::XSync(self.display.raw(), 0);
        }
    }

    fn handle_destroy_notify(&mut self, event: xlib::XEvent) {
        let destroy_event: xlib::XDestroyWindowEvent = From::from(event);
        if let Some(workspace) = self.workspaces.get_mut(self.current_workspace) {
            workspace.remove_window(destroy_event.window);
        }
        self.layout.remove_window(destroy_event.window);
        self.raise_floating_windows();
        unsafe {
            self.notification_manager.raise_all();
            xlib::XSync(self.display.raw(), 0);
        }
    }

    fn handle_enter_notify(&mut self, event: xlib::XEvent) {
        let enter_event: xlib::XCrossingEvent = From::from(event);
        if !self.dragging
            && !self.resizing
            && enter_event.window != 0
            && enter_event.window != self.layout.get_root()
            && !self
                .notification_manager
                .contains_window(enter_event.window)
            && self.config.appearance.focus_follows_mouse
        {
            let window_id = enter_event.window;
            let is_floating = if let Some(workspace) = self.workspaces.get(self.current_workspace) {
                for window in &workspace.windows {
                    unsafe {
                        let border_color = if window.id == window_id {
                            self.config.get_focused_border_color()
                        } else {
                            self.config.get_border_color()
                        };
                        xlib::XSetWindowBorder(self.display.raw(), window.id, border_color);
                    }
                }

                workspace
                    .windows
                    .iter()
                    .find(|w| w.id == window_id)
                    .map(|w| w.is_floating)
                    .unwrap_or(false)
            } else {
                false
            };

            self.layout.focus_window(window_id);
            self.set_active_window(window_id);

            if is_floating {
                unsafe {
                    xlib::XRaiseWindow(self.display.raw(), window_id);
                    self.notification_manager.raise_all();
                }
            } else {
                self.raise_floating_windows();
                unsafe {
                    self.notification_manager.raise_all();
                }
            }
        }
    }

    fn handle_leave_notify(&mut self, _event: xlib::XEvent) {
        // no-op
    }

    fn switch_to_workspace(&mut self, index: usize) {
        if index >= self.workspaces.len() || index == self.current_workspace {
            debug!("Invalid workspace switch request to {}", index);
            return;
        }

        info!("Switching to workspace {}", index);
        if let Some(current) = self.workspaces.get(self.current_workspace) {
            for window in &current.windows {
                if !window.is_dock {
                    unsafe {
                        xlib::XUnmapWindow(self.display.raw(), window.id);
                    }
                }
            }
        }

        self.current_workspace = index;
        self.update_current_desktop();
        self.layout.clear_windows();

        if let Some(new) = self.workspaces.get(self.current_workspace) {
            for window in &new.windows {
                unsafe {
                    if !window.is_dock {
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
                            (xlib::ButtonPressMask
                                | xlib::ButtonReleaseMask
                                | xlib::PointerMotionMask) as u32,
                            xlib::GrabModeAsync,
                            xlib::GrabModeAsync,
                            0,
                            0,
                        );
                        xlib::XGrabButton(
                            self.display.raw(),
                            3,
                            self.config.get_modifier(),
                            window.id,
                            1,
                            (xlib::ButtonPressMask
                                | xlib::ButtonReleaseMask
                                | xlib::PointerMotionMask) as u32,
                            xlib::GrabModeAsync,
                            xlib::GrabModeAsync,
                            0,
                            0,
                        );

                        if !self.config.appearance.focus_follows_mouse {
                            xlib::XGrabButton(
                                self.display.raw(),
                                xlib::AnyButton as u32,
                                0,
                                window.id,
                                1,
                                (xlib::ButtonPressMask | xlib::ButtonReleaseMask) as u32,
                                xlib::GrabModeSync,
                                xlib::GrabModeAsync,
                                0,
                                0,
                            );
                        }

                        if window.is_floating {
                            xlib::XMoveResizeWindow(
                                self.display.raw(),
                                window.id,
                                window.x,
                                window.y,
                                window.width,
                                window.height,
                            );
                        }
                    }
                }
                if !window.is_dock && !window.is_floating {
                    self.layout.add_window(window.id);
                }
            }
            if let Some(focused) = new.get_focused_window() {
                if !focused.is_dock {
                    self.layout.focus_window(focused.id);
                    self.set_active_window(focused.id);
                }
            }
            self.raise_floating_windows();
        }

        self.layout.relayout();
        unsafe {
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
            self.layout.focus_window(event.window);
            self.set_active_window(event.window);
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
                    self.layout.focus_window(event.window);
                    self.set_active_window(event.window);
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

    fn set_active_window(&mut self, window: xlib::Window) {
        unsafe {
            let root = xlib::XDefaultRootWindow(self.display.raw());
            xlib::XChangeProperty(
                self.display.raw(),
                root,
                self.net_active_window,
                xlib::XA_WINDOW,
                32,
                xlib::PropModeReplace,
                &window as *const xlib::Window as *const u8,
                1,
            );
            xlib::XSync(self.display.raw(), 0);
        }
    }

    fn update_current_desktop(&mut self) {
        unsafe {
            let root = xlib::XDefaultRootWindow(self.display.raw());
            let current_desktop = self.current_workspace as u32;
            xlib::XChangeProperty(
                self.display.raw(),
                root,
                self.net_current_desktop,
                xlib::XA_CARDINAL,
                32,
                xlib::PropModeReplace,
                &current_desktop as *const u32 as *const u8,
                1,
            );
            xlib::XSync(self.display.raw(), 0);
        }
    }

    fn handle_button_press(&mut self, event: xlib::XButtonEvent) {
        let button_event: xlib::XButtonEvent = event;
        debug!(
            "Button press: window={}, button={}, state={}",
            button_event.window, button_event.button, button_event.state
        );

        unsafe {
            self.notification_manager
                .handle_button_press(button_event.window);
        }

        if button_event.state & self.config.get_modifier() != 0 {
            match button_event.button {
                1 => self.start_window_drag(button_event),
                3 => self.start_window_resize(button_event),
                _ => (),
            }
        } else if !self.config.appearance.focus_follows_mouse
            && button_event.window != 0
            && button_event.window != self.layout.get_root()
            && !self
                .notification_manager
                .contains_window(button_event.window)
        {
            let window_id = button_event.window;
            let is_floating = if let Some(workspace) = self.workspaces.get(self.current_workspace) {
                for window in &workspace.windows {
                    unsafe {
                        let border_color = if window.id == window_id {
                            self.config.get_focused_border_color()
                        } else {
                            self.config.get_border_color()
                        };
                        xlib::XSetWindowBorder(self.display.raw(), window.id, border_color);
                    }
                }

                workspace
                    .windows
                    .iter()
                    .find(|w| w.id == window_id)
                    .map(|w| w.is_floating)
                    .unwrap_or(false)
            } else {
                false
            };

            self.layout.focus_window(window_id);
            self.set_active_window(window_id);

            if is_floating {
                unsafe {
                    xlib::XRaiseWindow(self.display.raw(), window_id);
                    self.notification_manager.raise_all();
                }
            } else {
                self.raise_floating_windows();
                unsafe {
                    self.notification_manager.raise_all();
                }
            }

            unsafe {
                xlib::XAllowEvents(self.display.raw(), xlib::ReplayPointer, 0);
                xlib::XSync(self.display.raw(), 0);
            }
        }
    }

    fn handle_expose(&mut self, event: xlib::XExposeEvent) {
        unsafe {
            self.notification_manager.handle_expose(event.window);
        }
    }

    fn handle_client_message(&mut self, event: xlib::XEvent) {
        let client_event: xlib::XClientMessageEvent = From::from(event);
        if client_event.message_type == self.net_current_desktop {
            let workspace_index = client_event.data.get_long(0) as usize;
            if workspace_index < self.workspaces.len() {
                self.switch_to_workspace(workspace_index);
            }
        }
    }
}
