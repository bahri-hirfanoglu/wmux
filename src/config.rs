use std::path::Path;

use anyhow::Result;
use serde::Deserialize;

/// Top-level wmux configuration, loaded from %APPDATA%\wmux\config.toml.
#[derive(Debug, Deserialize, Default)]
pub struct WmuxConfig {
    /// Override the default shell (e.g., "pwsh.exe", "cmd.exe",
    /// "C:\\Program Files\\Git\\bin\\bash.exe").
    pub default_shell: Option<String>,
}

/// Load config from the given path. Returns default config if file does not exist.
/// Errors only on parse failures (malformed TOML), NOT on missing file.
pub fn load_config(path: &Path) -> Result<WmuxConfig> {
    if !path.exists() {
        return Ok(WmuxConfig::default());
    }
    let contents = std::fs::read_to_string(path)?;
    let config: WmuxConfig = toml::from_str(&contents)?;
    Ok(config)
}
