use anyhow::Result;
use velowm::core::wm::WindowManager;

fn main() -> Result<()> {
    env_logger::init();

    let mut wm = WindowManager::new()?;
    wm.run()?;

    Ok(())
}
