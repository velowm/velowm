use anyhow::Result;
use log::error;
use std::{env, process};
use velowm::{core::wm::WindowManager, Config};

fn main() -> Result<()> {
    let config = Config::load().unwrap_or_default();

    if config.logging_enabled {
        if env::var("RUST_LOG").is_err() {
            env::set_var("RUST_LOG", "debug");
        }
        env_logger::init();
    }

    if env::var("DISPLAY").is_err() {
        error!("DISPLAY environment variable not set. Are you running inside X11?");
        process::exit(1);
    }

    match WindowManager::new() {
        Ok(mut wm) => wm.run()?,
        Err(e) => {
            error!("Failed to initialize window manager: {}", e);
            error!("Make sure X11 is running and you have the correct permissions");
            process::exit(1);
        }
    }

    Ok(())
}
