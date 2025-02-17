use anyhow::Result;
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::{
    process::Command as ProcessCommand,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::channel,
    },
    thread,
};
use x11::xlib;

use crate::{
    config::{command::Command, loader::Config},
    ui::{cursor::Cursor, layout::MasterStackLayout, notification::NotificationWindow},
    utils::x11::Display,
};

static SHOULD_RELOAD_CONFIG: AtomicBool = AtomicBool::new(false);

pub struct WindowManager {
    display: Display,
    running: bool,
    #[allow(dead_code)]
    cursor: Cursor,
    config: Config,
    layout: MasterStackLayout,
    _watcher: RecommendedWatcher,
    notification: NotificationWindow,
}

impl WindowManager {
    pub fn new() -> Result<Self> {
        unsafe {
            libc::signal(libc::SIGUSR1, Self::handle_sigusr1 as libc::sighandler_t);
        }

        let display = Display::new()?;
        let root = unsafe { xlib::XDefaultRootWindow(display.raw()) };
        let cursor = unsafe { Cursor::new(display.raw())? };

        let config = Config::load().unwrap_or_else(|_| Config::default());

        let layout = unsafe { MasterStackLayout::new(display.raw(), root, config.clone()) };
        let notification = unsafe { NotificationWindow::new(display.raw(), root) };

        if let Err(e) = Config::load() {
            unsafe {
                notification.show_error(&format!("Failed to load config: {}", e));
            }
        }

        let (tx, rx) = channel();
        let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                if event.kind.is_modify() {
                    let _ = tx.send(());
                }
            }
        })?;

        let config_path = Config::get_config_path()?;
        watcher.watch(&config_path, RecursiveMode::NonRecursive)?;

        let pid = std::process::id();
        thread::spawn(move || {
            while rx.recv().is_ok() {
                thread::sleep(std::time::Duration::from_millis(100));
                if let Ok(mut cmd) = ProcessCommand::new("kill")
                    .arg("-SIGUSR1")
                    .arg(pid.to_string())
                    .spawn()
                {
                    let _ = cmd.wait();
                }
            }
        });

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

        Ok(Self {
            display,
            running: true,
            cursor,
            config,
            layout,
            _watcher: watcher,
            notification,
        })
    }

    unsafe extern "C" fn handle_sigusr1(_: i32) {
        SHOULD_RELOAD_CONFIG.store(true, Ordering::SeqCst);
    }

    fn reload_config(&mut self) -> Result<()> {
        let new_config = match Config::load() {
            Ok(config) => config,
            Err(e) => {
                unsafe {
                    self.notification
                        .show_error(&format!("Failed to load config: {}", e));
                }
                return Ok(());
            }
        };
        let root = unsafe { xlib::XDefaultRootWindow(self.display.raw()) };

        unsafe {
            xlib::XUngrabKey(self.display.raw(), xlib::AnyKey, xlib::AnyModifier, root);

            Self::setup_key_bindings(self.display.raw(), root, &new_config);

            xlib::XSync(self.display.raw(), 0);
        }

        self.layout.update_config(new_config.clone());

        self.config = new_config;

        unsafe {
            xlib::XSync(self.display.raw(), 0);
        }

        Ok(())
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
            if SHOULD_RELOAD_CONFIG.load(Ordering::SeqCst) {
                if let Err(e) = self.reload_config() {
                    unsafe {
                        self.notification
                            .show_error(&format!("Failed to reload config: {}", e));
                    }
                }
                SHOULD_RELOAD_CONFIG.store(false, Ordering::SeqCst);
                self.layout.relayout();
                unsafe {
                    xlib::XSync(self.display.raw(), 0);
                }
            }

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
                xlib::EnterNotify => self.handle_enter_notify(event),
                xlib::LeaveNotify => self.handle_leave_notify(event),
                xlib::Expose => {
                    let expose_event: xlib::XExposeEvent = From::from(event);
                    if expose_event.window == self.notification.window {
                        // TODO: Store last error message and redraw it here
                    }
                }
                _ => (),
            }
        }

        Ok(())
    }

    fn handle_motion_notify(&mut self, event: xlib::XEvent) {
        let motion_event: xlib::XMotionEvent = From::from(event);
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
                        if let Err(e) = ProcessCommand::new(cmd).spawn() {
                            unsafe {
                                self.notification
                                    .show_error(&format!("Failed to spawn {}: {}", cmd, e));
                            }
                        }
                    }
                    Command::Close => self.close_focused_window(),
                }
            }
        }
    }

    fn close_focused_window(&mut self) {
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

        unsafe {
            xlib::XMapWindow(self.display.raw(), map_event.window);

            self.layout.add_window(map_event.window);
        }
    }

    fn handle_unmap_notify(&mut self, event: xlib::XEvent) {
        let unmap_event: xlib::XUnmapEvent = From::from(event);
        self.layout.remove_window(unmap_event.window);
    }

    fn handle_destroy_notify(&mut self, event: xlib::XEvent) {
        let destroy_event: xlib::XDestroyWindowEvent = From::from(event);
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
}
