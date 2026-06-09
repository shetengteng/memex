use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};

const LABEL: &str = "com.memex.daemon";

pub fn plist_path() -> PathBuf {
    // INVARIANT: launchd integration only runs on macOS where $HOME is
    // guaranteed by the OS. Panicking on a missing HOME is correct — there is
    // no reasonable fallback for a per-user LaunchAgent plist.
    dirs::home_dir()
        .expect("INVARIANT: home directory must be resolvable on macOS")
        .join("Library/LaunchAgents")
        .join(format!("{}.plist", LABEL))
}

pub fn generate_plist(daemon_bin: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{label}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{bin}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>{home}/.memex/daemon.stdout.log</string>
    <key>StandardErrorPath</key>
    <string>{home}/.memex/daemon.stderr.log</string>
</dict>
</plist>"#,
        label = LABEL,
        bin = daemon_bin,
        home = dirs::home_dir().unwrap_or_default().display(),
    )
}

pub fn install_plist(daemon_bin: &str) -> Result<()> {
    let path = plist_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let content = generate_plist(daemon_bin);
    fs::write(&path, content).with_context(|| format!("failed to write {}", path.display()))?;
    println!("launchd plist written to {}", path.display());
    println!("To load:   launchctl load {}", path.display());
    println!("To unload: launchctl unload {}", path.display());
    Ok(())
}

pub fn uninstall_plist() -> Result<()> {
    let path = plist_path();
    if path.exists() {
        fs::remove_file(&path).with_context(|| format!("failed to remove {}", path.display()))?;
        println!("launchd plist removed");
    } else {
        println!("no plist found at {}", path.display());
    }
    Ok(())
}
