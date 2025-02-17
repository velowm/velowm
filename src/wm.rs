use anyhow::Result;
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::process::Command as ProcessCommand;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::channel;
use std::thread;
use x11::xlib;

use crate::config::{Command, Config};
use crate::cursor::Cursor;
use crate::layout::MasterStackLayout;
use crate::x::Display;

static SHOULD_RELOAD_CONFIG: AtomicBool = AtomicBool::new(false);

pub struct WindowManager {
    display: Display,
    running: bool,
    cursor: Cursor,
    config: Config,
    layout: MasterStackLayout,
    _watcher: RecommendedWatcher,
}

impl WindowManager {
    pub fn new() -> Result<Self> {
        unsafe {
            libc::signal(libc::SIGUSR1, Self::handle_sigusr1 as libc::sighandler_t);
        }

        let display = Display::new()?;
        let cursor = Cursor::new(display.raw())?;
        let config = Config::load()?;
        let root = unsafe { xlib::XDefaultRootWindow(display.raw()) };
        let layout = MasterStackLayout::new(display.raw(), root, config.clone());

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
            while let Ok(_) = rx.recv() {
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
        })
    }

    unsafe extern "C" fn handle_sigusr1(_: i32) {
        SHOULD_RELOAD_CONFIG.store(true, Ordering::SeqCst);
    }

    fn reload_config(&mut self) -> Result<()> {
        let new_config = Config::load()?;
        let root = unsafe { xlib::XDefaultRootWindow(self.display.raw()) };

        unsafe {
            xlib::XUngrabKey(
                self.display.raw(),
                xlib::AnyKey as i32,
                xlib::AnyModifier,
                root,
            );

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
                    eprintln!("Failed to reload config: {}", e);
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
                _ => (),
            }
        }

        Ok(())
    }

    fn handle_motion_notify(&mut self, event: xlib::XEvent) {
        let motion_event: xlib::XMotionEvent = From::from(event);
        unsafe {
            xlib::XSetInputFocus(
                self.display.raw(),
                motion_event.window,
                xlib::RevertToPointerRoot,
                xlib::CurrentTime,
            );
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
                            eprintln!("Failed to spawn {}: {}", cmd, e);
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
            let mut window_under_pointer: xlib::Window = 0;

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

            window_under_pointer = child_return;

            if window_under_pointer != 0 && window_under_pointer != self.layout.get_root() {
                let wm_protocols = xlib::XInternAtom(
                    self.display.raw(),
                    "WM_PROTOCOLS\0".as_ptr() as *const i8,
                    0,
                );
                let wm_delete_window = xlib::XInternAtom(
                    self.display.raw(),
                    "WM_DELETE_WINDOW\0".as_ptr() as *const i8,
                    0,
                );

                let mut protocols: *mut xlib::Atom = std::ptr::null_mut();
                let mut num_protocols: i32 = 0;

                if xlib::XGetWMProtocols(
                    self.display.raw(),
                    window_under_pointer,
                    &mut protocols,
                    &mut num_protocols,
                ) != 0
                {
                    let protocols_slice =
                        std::slice::from_raw_parts(protocols, num_protocols as usize);
                    if protocols_slice.contains(&wm_delete_window) {
                        let mut data: xlib::ClientMessageData = unsafe { std::mem::zeroed() };
                        data.set_long(0, wm_delete_window as i64);

                        let mut event = xlib::XEvent {
                            client_message: xlib::XClientMessageEvent {
                                type_: xlib::ClientMessage,
                                serial: 0,
                                send_event: 1,
                                display: self.display.raw(),
                                window: window_under_pointer,
                                message_type: wm_protocols,
                                format: 32,
                                data,
                            },
                        };
                        xlib::XSendEvent(
                            self.display.raw(),
                            window_under_pointer,
                            0,
                            0,
                            &mut event,
                        );
                    } else {
                        xlib::XDestroyWindow(self.display.raw(), window_under_pointer);
                    }
                    xlib::XFree(protocols as *mut _);
                } else {
                    xlib::XDestroyWindow(self.display.raw(), window_under_pointer);
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
}
