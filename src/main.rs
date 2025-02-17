mod config;
mod cursor;
mod layout;
mod wm;
mod x;

use anyhow::Result;

fn main() -> Result<()> {
    env_logger::init();

    let mut wm = wm::WindowManager::new()?;
    wm.run()?;

    Ok(())
}
