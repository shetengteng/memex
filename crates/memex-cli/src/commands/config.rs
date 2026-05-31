use anyhow::Result;
use memex_core::config::MemexConfig;
use memex_core::memex_dir;

pub fn show(json: bool) -> Result<()> {
    let config = MemexConfig::load(&memex_dir())?;

    if json {
        println!("{}", serde_json::to_string_pretty(&config)?);
    } else {
        println!("{}", toml::to_string_pretty(&config)?);
    }

    Ok(())
}
