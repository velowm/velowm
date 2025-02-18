use anyhow::Result;
use log::error;
use rand::random;
use std::{
    env, fs,
    io::{self, Write},
    path::PathBuf,
    process,
};
use velowm::{velowm_core::wm::WindowManager, Config};

fn get_log_file_path() -> Result<PathBuf> {
    let cache_dir = PathBuf::from(env::var("HOME")?).join(".cache/velowm");
    fs::create_dir_all(&cache_dir)?;
    Ok(cache_dir.join(format!("log{}.log", random::<u32>())))
}

struct DualWriter {
    file: fs::File,
}

impl Write for DualWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        io::stdout().write_all(buf)?;
        self.file.write_all(buf)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        io::stdout().flush()?;
        self.file.flush()
    }
}

fn main() -> Result<()> {
    let config = Config::load().unwrap_or_default();

    if config.logging_enabled {
        if env::var("RUST_LOG").is_err() {
            env::set_var("RUST_LOG", "debug");
        }

        let log_file = fs::File::create(get_log_file_path()?)?;
        let dual_writer = DualWriter { file: log_file };

        env_logger::Builder::from_default_env()
            .format(|buf, record| {
                writeln!(
                    buf,
                    "{} [{}] {}",
                    chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                    record.level(),
                    record.args()
                )
            })
            .target(env_logger::Target::Pipe(Box::new(dual_writer)))
            .init();
    }

    if env::var("WAYLAND_DISPLAY").is_ok()
        || env::var("XDG_SESSION_TYPE").is_ok_and(|v| v == "wayland")
    {
        error!("Wayland session detected. velowm is an X11 window manager and cannot run under Wayland.");
        process::exit(1);
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
